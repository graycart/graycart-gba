//! Seiko S-3511A real-time clock (SIIRTC) behind cart GPIO.
//!
//! Protocol matches pret `siirtc.c` / mGBA `gpio.c`: commands are shifted MSB-first
//! on the wire (assembled LSB-first in the shift register), data bytes LSB-first.
//! Status bit 7 (`STATUS_POWER`) is the Pokemon "internal battery has run dry" flag.
//!
//! Cited: GBATEK Cart RTC; pret pokeemerald `src/siirtc.c`; mGBA `src/gba/cart/gpio.c`.

use std::time::{SystemTime, UNIX_EPOCH};

/// Sidecar trailer length: 8 BCD/status bytes + 8-byte LE unix timestamp.
pub const RTC_SAVE_LEN: usize = 16;

/// Force-reset / status / datetime / time / alarm register ids (bits 1–3 of command).
const REG_RESET: u8 = 0;
const REG_STATUS: u8 = 1;
const REG_DATETIME: u8 = 2;
const REG_TIME: u8 = 3;
const REG_ALARM: u8 = 4;

/// Byte counts for each register (mGBA `RTC_BYTES` after command decode).
fn reg_len(reg: u8) -> u8 {
    match reg {
        REG_STATUS => 1,
        REG_DATETIME => 7,
        REG_TIME => 3,
        REG_ALARM => 2,
        _ => 0,
    }
}

fn to_bcd(v: u8) -> u8 {
    let v = v.min(99);
    ((v / 10) << 4) | (v % 10)
}

fn bit_reverse(b: u8) -> u8 {
    let mut out = 0u8;
    for i in 0..8 {
        if b & (1 << i) != 0 {
            out |= 1 << (7 - i);
        }
    }
    out
}

/// S-3511A chip state + 3-wire serial engine (SCK / SIO / CS).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SiRtc {
    year: u8,
    month: u8,
    day: u8,
    weekday: u8,
    hour: u8,
    minute: u8,
    second: u8,
    /// Control/status. Bit6 = 24h, bit7 = power failure (battery dry).
    status: u8,
    alarm_hour: u8,
    alarm_minute: u8,

    // --- serial engine (not part of the 16-byte sidecar) ---
    #[serde(skip)]
    cs: bool,
    #[serde(skip)]
    sck: bool,
    #[serde(skip)]
    /// Last SIO bit driven by the cart (read path).
    sio_out: bool,
    #[serde(skip)]
    sck_was_high: bool,
    #[serde(skip)]
    command_active: bool,
    #[serde(skip)]
    reading: bool,
    #[serde(skip)]
    reg: u8,
    #[serde(skip)]
    bits: u8,
    #[serde(skip)]
    bits_read: u8,
    #[serde(skip)]
    bytes_remaining: u8,
    #[serde(skip)]
    data_index: u8,
    /// Latched datetime/time payload for the active transfer.
    #[serde(skip)]
    transfer: [u8; 7],

    /// Host unix seconds when `year..second` were last synced (sidecar).
    host_unix: i64,
}

impl Default for SiRtc {
    fn default() -> Self {
        Self::new_from_host()
    }
}

impl SiRtc {
    /// Fresh chip with good battery (status `0x40` = 24h, no POWER bit) and host time.
    pub fn new_from_host() -> Self {
        let mut rtc = Self {
            year: 0,
            month: 1,
            day: 1,
            weekday: 0,
            hour: 0,
            minute: 0,
            second: 0,
            status: 0x40,
            alarm_hour: 0,
            alarm_minute: 0x80,
            cs: false,
            sck: false,
            sio_out: true,
            sck_was_high: true,
            command_active: false,
            reading: false,
            reg: 0,
            bits: 0,
            bits_read: 0,
            bytes_remaining: 0,
            data_index: 0,
            transfer: [0; 7],
            host_unix: 0,
        };
        rtc.sync_from_host();
        rtc
    }

    /// Encode 16-byte sidecar trailer (ares-compatible layout).
    pub fn save_bytes(&mut self) -> [u8; RTC_SAVE_LEN] {
        self.sync_from_host();
        let mut out = [0u8; RTC_SAVE_LEN];
        out[0] = self.year;
        out[1] = self.month;
        out[2] = self.day;
        out[3] = self.weekday;
        out[4] = self.hour;
        out[5] = self.minute;
        out[6] = self.second;
        out[7] = self.status;
        let ts = now_unix();
        self.host_unix = ts;
        out[8..16].copy_from_slice(&ts.to_le_bytes());
        out
    }

    /// Restore from a 16-byte trailer, advancing by wall-clock delta since save.
    pub fn load_bytes(&mut self, bytes: &[u8]) {
        if bytes.len() < RTC_SAVE_LEN {
            self.sync_from_host();
            return;
        }
        self.year = bytes[0];
        self.month = bytes[1].max(1);
        self.day = bytes[2].max(1);
        self.weekday = bytes[3] % 7;
        self.hour = bytes[4];
        self.minute = bytes[5];
        self.second = bytes[6];
        self.status = bytes[7];
        // Clear POWER: emulated battery is always good after a successful load.
        self.status &= !0x80;
        if self.status & 0x40 == 0 {
            self.status |= 0x40;
        }
        let mut ts_bytes = [0u8; 8];
        ts_bytes.copy_from_slice(&bytes[8..16]);
        let saved = i64::from_le_bytes(ts_bytes);
        self.host_unix = saved;
        let now = now_unix();
        if saved > 0 && now > saved {
            let mut delta = (now - saved).min(365 * 24 * 3600);
            while delta > 0 {
                self.tick_second();
                delta -= 1;
            }
        }
        self.host_unix = now;
    }

    /// Current status byte (bit7 = battery dry). Used by tests.
    pub fn status(&self) -> u8 {
        self.status
    }

    /// Drive pins from GPIO data register. `pins` bits: 0=SCK, 1=SIO(in), 2=CS.
    /// Returns the SIO bit the chip drives when SIO is an input.
    pub fn update_pins(&mut self, pins: u8) -> bool {
        let sck = pins & 1 != 0;
        let sio_in = pins & 2 != 0;
        let cs = pins & 4 != 0;

        if !cs {
            self.cs = false;
            self.sck = sck;
            self.sck_was_high = true;
            self.command_active = false;
            self.bits_read = 0;
            self.bits = 0;
            self.bytes_remaining = 0;
            self.data_index = 0;
            self.sio_out = true;
            return true;
        }

        let cs_rose = !self.cs && cs;
        self.cs = true;
        if cs_rose {
            self.command_active = false;
            self.bits_read = 0;
            self.bits = 0;
            self.bytes_remaining = 0;
            self.data_index = 0;
            self.sck_was_high = sck;
            self.sio_out = true;
        }

        if !self.command_active {
            // Collect command byte: sample SIO while SCK low; count on rising edge.
            if !sck {
                self.bits &= !(1 << self.bits_read);
                if sio_in {
                    self.bits |= 1 << self.bits_read;
                }
            }
            if !self.sck_was_high && sck {
                self.bits_read = self.bits_read.wrapping_add(1);
                if self.bits_read == 8 {
                    self.begin_command();
                }
            }
            self.sio_out = true;
        } else if !self.reading {
            // Write payload
            if !sck {
                self.bits &= !(1 << self.bits_read);
                if sio_in {
                    self.bits |= 1 << self.bits_read;
                }
            }
            if !self.sck_was_high && sck {
                self.bits_read = self.bits_read.wrapping_add(1);
                if self.bits_read == 8 {
                    self.process_write_byte();
                }
            }
            self.sio_out = true;
        } else {
            // Read payload: present bit on falling SCK (first falling after command = bit 0).
            if self.sck_was_high && !sck {
                self.sio_out = self.output_bit();
                self.bits_read = self.bits_read.wrapping_add(1);
                if self.bits_read == 8 {
                    self.bytes_remaining = self.bytes_remaining.saturating_sub(1);
                    self.data_index = self.data_index.wrapping_add(1);
                    self.bits_read = 0;
                    if self.bytes_remaining == 0 {
                        self.bytes_remaining = reg_len(self.reg);
                        self.data_index = 0;
                    }
                }
            }
        }

        self.sck = sck;
        self.sck_was_high = sck;
        self.sio_out
    }

    fn begin_command(&mut self) {
        // Wire was MSB-first; shift register is LSB-first → bit-reversed vs pret CMD().
        let mut cmd = self.bits;
        if cmd >> 4 != 0b0110 {
            cmd = bit_reverse(cmd);
        }
        if cmd >> 4 != 0b0110 {
            self.bits = 0;
            self.bits_read = 0;
            return;
        }
        self.reading = cmd & 1 != 0;
        self.reg = (cmd >> 1) & 7;
        self.command_active = true;
        self.bits = 0;
        self.bits_read = 0;
        self.data_index = 0;
        self.bytes_remaining = reg_len(self.reg);

        match self.reg {
            REG_RESET if !self.reading => {
                self.reset_regs();
            }
            REG_DATETIME | REG_TIME if self.reading => {
                self.sync_from_host();
                self.fill_transfer();
            }
            REG_STATUS if self.reading => {
                self.transfer[0] = self.status;
            }
            REG_ALARM if self.reading => {
                self.transfer[0] = self.alarm_hour;
                self.transfer[1] = self.alarm_minute;
            }
            _ => {}
        }
    }

    fn process_write_byte(&mut self) {
        let v = self.bits;
        self.bits = 0;
        self.bits_read = 0;
        match self.reg {
            REG_STATUS => {
                // POWER is sticky-clear only (hardware AND).
                let power = self.status & 0x80;
                self.status = (v & 0x6A) | (power & v);
                self.status &= !0x80; // emulated battery stays charged
            }
            REG_DATETIME => {
                self.write_datetime_byte(self.data_index as usize, v);
                self.data_index = self.data_index.wrapping_add(1);
            }
            REG_TIME => {
                self.write_time_byte(self.data_index as usize, v);
                self.data_index = self.data_index.wrapping_add(1);
            }
            REG_ALARM => {
                if self.data_index == 0 {
                    self.alarm_hour = v;
                } else {
                    self.alarm_minute = v;
                }
                self.data_index = self.data_index.wrapping_add(1);
            }
            _ => {}
        }
        self.bytes_remaining = self.bytes_remaining.saturating_sub(1);
        if self.bytes_remaining == 0 {
            self.bytes_remaining = reg_len(self.reg);
            self.data_index = 0;
            self.host_unix = now_unix();
        }
    }

    fn output_bit(&self) -> bool {
        let byte = match self.reg {
            REG_STATUS => self.status,
            REG_DATETIME | REG_TIME | REG_ALARM => self
                .transfer
                .get(self.data_index as usize)
                .copied()
                .unwrap_or(0xFF),
            _ => 0xFF,
        };
        (byte >> self.bits_read) & 1 != 0
    }

    fn fill_transfer(&mut self) {
        self.transfer = [
            self.year,
            self.month,
            self.day,
            self.weekday,
            self.hour,
            self.minute,
            self.second,
        ];
        if self.reg == REG_TIME {
            self.transfer = [self.hour, self.minute, self.second, 0, 0, 0, 0];
        }
    }

    fn write_datetime_byte(&mut self, index: usize, v: u8) {
        match index {
            0 => self.year = v,
            1 => self.month = v,
            2 => self.day = v,
            3 => self.weekday = v & 7,
            4 => self.hour = v,
            5 => self.minute = v,
            _ => self.second = v & 0x7F,
        }
    }

    fn write_time_byte(&mut self, index: usize, v: u8) {
        match index {
            0 => self.hour = v,
            1 => self.minute = v,
            _ => self.second = v & 0x7F,
        }
    }

    fn reset_regs(&mut self) {
        self.year = 0;
        self.month = 1;
        self.day = 1;
        self.weekday = 0;
        self.hour = 0;
        self.minute = 0;
        self.second = 0;
        self.status = 0x00;
        self.alarm_hour = 0;
        self.alarm_minute = 0;
    }

    fn sync_from_host(&mut self) {
        let now = SystemTime::now();
        let secs = now
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        // localtime via offset from UTC: use `chrono`-free civil conversion via
        // the same epoch math as `time` crate would — prefer libc localtime.
        let tm = local_tm(secs);
        self.year = to_bcd((tm.year % 100) as u8);
        self.month = to_bcd(tm.month);
        self.day = to_bcd(tm.day);
        self.weekday = tm.weekday;
        if self.status & 0x40 != 0 {
            self.hour = to_bcd(tm.hour);
        } else {
            let h12 = tm.hour % 12;
            self.hour = to_bcd(h12);
        }
        self.minute = to_bcd(tm.minute);
        self.second = to_bcd(tm.second);
        self.host_unix = secs;
        // Keep battery good while syncing from a live host clock.
        self.status &= !0x80;
        if self.status & 0x40 == 0 && self.status & 0x80 == 0 {
            // leave 12h mode alone if game set it; default path sets 24h in new_from_host
        }
    }

    fn tick_second(&mut self) {
        fn bcd_inc(v: &mut u8) -> u8 {
            if (*v & 0x0F) >= 0x09 {
                *v = (*v & 0xF0).wrapping_add(0x10);
            } else {
                *v = v.wrapping_add(1);
            }
            *v
        }

        if bcd_inc(&mut self.second) < 0x60 {
            return;
        }
        self.second = 0;
        if bcd_inc(&mut self.minute) < 0x60 {
            return;
        }
        self.minute = 0;
        if self.status & 0x40 != 0 {
            if (bcd_inc(&mut self.hour) & 0x7F) < 0x24 {
                return;
            }
            self.hour &= 0x80;
        } else if (bcd_inc(&mut self.hour) & 0x7F) < 0x12 {
            return;
        } else {
            self.hour &= 0x80;
            self.hour ^= 0x80;
        }
        self.weekday = (self.weekday + 1) % 7;
        let mut days = [
            0x31u8, 0x28, 0x31, 0x30, 0x31, 0x30, 0x31, 0x31, 0x30, 0x31, 0x30, 0x31,
        ];
        if self.year != 0 && self.year & 3 == 0 {
            days[1] = 0x29;
        }
        let mon = bcd_to_bin(self.month).saturating_sub(1).min(11) as usize;
        if bcd_inc(&mut self.day) <= days[mon] {
            return;
        }
        self.day = 1;
        if bcd_inc(&mut self.month) <= 0x12 {
            return;
        }
        self.month = 1;
        let _ = bcd_inc(&mut self.year);
    }
}

fn bcd_to_bin(v: u8) -> u8 {
    (v >> 4) * 10 + (v & 0x0F)
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

struct Civil {
    year: i32,
    month: u8,
    day: u8,
    weekday: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

fn local_tm(unix: i64) -> Civil {
    // UTC is enough for berry growth / tides (relative deltas dominate). Avoids
    // a libc dependency for localtime_r.
    civil_utc(unix)
}

fn civil_utc(unix: i64) -> Civil {
    let secs = unix.max(0) as u64;
    let second = (secs % 60) as u8;
    let mins = secs / 60;
    let minute = (mins % 60) as u8;
    let hours = mins / 60;
    let hour = (hours % 24) as u8;
    let days = (hours / 24) as i64;
    // 1970-01-01 was Thursday (4)
    let weekday = ((days + 4) % 7) as u8;
    let (year, month, day) = civil_from_days(days);
    Civil {
        year,
        month,
        day,
        weekday,
        hour,
        minute,
        second,
    }
}

fn civil_from_days(days: i64) -> (i32, u8, u8) {
    // Howard Hinnant civil_from_days (UTC).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64 + era * 400) as i32;
    let doy = (doe - (365 * yoe + yoe / 4 - yoe / 100)) as u32;
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drive one pret-style WriteCommand / WriteData / ReadData transaction.
    fn gpio_write_byte_msb_first(rtc: &mut SiRtc, value: u8, pins_base: &mut u8) {
        for i in 0..8 {
            let bit = (value >> (7 - i)) & 1;
            *pins_base = (bit << 1) | 4; // CS high, SCK low
            rtc.update_pins(*pins_base);
            *pins_base = (bit << 1) | 1 | 4; // SCK high
            rtc.update_pins(*pins_base);
        }
    }

    fn gpio_write_byte_lsb_first(rtc: &mut SiRtc, value: u8, pins_base: &mut u8) {
        for i in 0..8 {
            let bit = (value >> i) & 1;
            *pins_base = (bit << 1) | 4;
            rtc.update_pins(*pins_base);
            *pins_base = (bit << 1) | 1 | 4;
            rtc.update_pins(*pins_base);
        }
    }

    fn gpio_read_byte(rtc: &mut SiRtc, pins_base: &mut u8) -> u8 {
        let mut value = 0u8;
        for _ in 0..8 {
            *pins_base = 4; // CS, SCK low → chip presents bit
            let sio = rtc.update_pins(*pins_base);
            value = (value >> 1) | (u8::from(sio) << 7);
            *pins_base = 1 | 4;
            let _ = rtc.update_pins(*pins_base);
        }
        value
    }

    #[test]
    fn status_read_is_24h_no_power() {
        let mut rtc = SiRtc::new_from_host();
        assert_eq!(rtc.status() & 0x80, 0, "battery must be good");
        assert_ne!(rtc.status() & 0x40, 0, "24h mode");

        // pret: CS rise with SCK high, then CMD_STATUS|RD = 0x63 MSB-first
        let mut pins = 1u8; // SCK high
        rtc.update_pins(pins);
        pins = 1 | 4; // CS rise
        rtc.update_pins(pins);

        gpio_write_byte_msb_first(&mut rtc, 0x63, &mut pins);
        let status = gpio_read_byte(&mut rtc, &mut pins);
        assert_eq!(status & 0x80, 0, "POWER clear, got {status:#x}");
        assert_ne!(status & 0x40, 0, "24HOUR set, got {status:#x}");
    }

    #[test]
    fn status_write_preserves_24h() {
        let mut rtc = SiRtc::new_from_host();
        let mut pins = 1u8;
        rtc.update_pins(pins);
        pins = 1 | 4;
        rtc.update_pins(pins);
        // CMD_STATUS|WR = 0x62
        gpio_write_byte_msb_first(&mut rtc, 0x62, &mut pins);
        gpio_write_byte_lsb_first(&mut rtc, 0x40, &mut pins);
        assert_eq!(rtc.status() & 0xC0, 0x40);
    }

    #[test]
    fn sidecar_round_trip_clears_power() {
        let mut rtc = SiRtc::new_from_host();
        rtc.status |= 0x80;
        let bytes = rtc.save_bytes();
        let mut loaded = SiRtc::new_from_host();
        loaded.load_bytes(&bytes);
        assert_eq!(loaded.status() & 0x80, 0);
    }
}
