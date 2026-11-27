//! Cartridge save, GPIO, and the mGBA debug port.
//!
//! Cited: GBATEK, GBA Cart Backup. https://problemkaputt.de/gbatek.htm

use super::{Bus, SaveChip};
use crate::cart::{Gpio, RTC_SAVE_LEN, SaveKind, gpio_reject_line};

impl Bus {
    /// Detected save kind (from ROM ID strings).
    pub fn save_kind(&self) -> SaveKind {
        self.save_kind
    }

    /// Enable cart GPIO at 0x080000C4/C6/C8 without an RTC (tests / reject path).
    pub fn set_has_gpio(&mut self, yes: bool) {
        if yes {
            if self.gpio.is_none() {
                self.gpio = Some(Gpio::unsupported());
            }
        } else {
            self.gpio = None;
        }
        self.gpio_warned = false;
    }

    /// True when this cart has GPIO (RTC or test-forced).
    pub fn has_gpio(&self) -> bool {
        self.gpio.is_some()
    }

    /// True when an S-3511A RTC is attached.
    pub fn has_rtc(&self) -> bool {
        self.gpio.as_ref().is_some_and(|g| g.has_rtc())
    }

    /// Load sidecar bytes into the save chip (and RTC trailer when present).
    pub fn load_save(&mut self, bytes: &[u8]) {
        let chip_len = self.save_chip_len();
        let (chip, rtc_trailer) = if chip_len > 0 && bytes.len() >= chip_len + RTC_SAVE_LEN {
            (
                &bytes[..chip_len],
                Some(&bytes[chip_len..chip_len + RTC_SAVE_LEN]),
            )
        } else if chip_len > 0 && bytes.len() > chip_len {
            (&bytes[..chip_len], None)
        } else {
            (bytes, None)
        };
        match &mut self.save {
            SaveChip::None => {}
            SaveChip::Sram(data) => {
                data.fill(0xFF);
                let n = chip.len().min(data.len());
                data[..n].copy_from_slice(&chip[..n]);
            }
            SaveChip::Flash(flash) => flash.load_bytes(chip),
            SaveChip::Eeprom(eeprom) => eeprom.load_bytes(chip),
        }
        if let Some(gpio) = self.gpio.as_mut()
            && let Some(rtc) = gpio.rtc_mut()
        {
            if let Some(trailer) = rtc_trailer {
                rtc.load_bytes(trailer);
            } else {
                // No trailer: sync from host with a good battery.
                *rtc = crate::cart::SiRtc::new_from_host();
            }
        }
        self.save_dirty = false;
    }

    /// True when the save chip differs from the last successful sidecar flush.
    pub fn save_dirty(&self) -> bool {
        self.save_dirty
    }

    /// Clear the dirty flag after a successful flush.
    pub fn clear_save_dirty(&mut self) {
        self.save_dirty = false;
    }

    /// Mark save contents dirty (e.g. after restoring a slot snapshot).
    pub fn mark_save_dirty(&mut self) {
        if self.save_kind != SaveKind::None {
            self.save_dirty = true;
        }
    }

    /// Current save-chip bytes (no RTC trailer). Prefer [`Self::sidecar_bytes`] for flush.
    pub fn save_bytes(&self) -> Option<&[u8]> {
        match &self.save {
            SaveChip::None => None,
            SaveChip::Sram(data) => Some(data.as_slice()),
            SaveChip::Flash(flash) => Some(flash.bytes()),
            SaveChip::Eeprom(eeprom) => {
                let b = eeprom.bytes();
                if b.is_empty() { None } else { Some(b) }
            }
        }
    }

    /// Sidecar payload: save chip bytes plus a 16-byte RTC trailer when present.
    pub fn sidecar_bytes(&mut self) -> Option<Vec<u8>> {
        let chip = self.save_bytes()?.to_vec();
        let mut out = chip;
        if let Some(gpio) = self.gpio.as_mut()
            && let Some(rtc) = gpio.rtc_mut()
        {
            out.extend_from_slice(&rtc.save_bytes());
        }
        Some(out)
    }

    fn save_chip_len(&self) -> usize {
        match &self.save {
            SaveChip::None => 0,
            SaveChip::Sram(data) => data.len(),
            SaveChip::Flash(flash) => flash.bytes().len(),
            SaveChip::Eeprom(eeprom) => eeprom.bytes().len(),
        }
    }

    /// SRAM / flash are an 8-bit bus: multi-byte reads duplicate the byte;
    /// multi-byte writes program only the lane that hits `addr`.
    pub(super) fn access_save(&mut self, addr: u32, size: u32) -> u32 {
        let wide_ok = matches!(
            self.save_kind,
            SaveKind::Sram | SaveKind::Flash64 | SaveKind::Flash128
        );
        match size {
            1 => {
                let value = self.read_save8(addr);
                self.latch(value);
                value
            }
            2 | 4 if wide_ok => {
                let b = self.read_save8(addr) & 0xFF;
                let value = if size == 2 {
                    b | (b << 8)
                } else {
                    b | (b << 8) | (b << 16) | (b << 24)
                };
                self.latch(value);
                value
            }
            _ => self.openbus(addr, size, "sram"),
        }
    }

    pub(super) fn store_save(&mut self, addr: u32, value: u32, size: u32) {
        let wide_ok = matches!(
            self.save_kind,
            SaveKind::Sram | SaveKind::Flash64 | SaveKind::Flash128
        );
        match size {
            1 => self.write_save8(addr, value as u8),
            2 | 4 if wide_ok => {
                let shift = (addr & (size - 1)) * 8;
                let byte = ((value >> shift) & 0xFF) as u8;
                self.write_save8(addr, byte);
            }
            _ => self.log_openbus(addr, "sram"),
        }
    }

    fn read_save8(&mut self, addr: u32) -> u32 {
        match &mut self.save {
            SaveChip::None | SaveChip::Eeprom(_) => 0xFF,
            SaveChip::Sram(data) => u32::from(data[(addr & 0x7FFF) as usize]),
            SaveChip::Flash(flash) => u32::from(flash.read(addr)),
        }
    }

    fn write_save8(&mut self, addr: u32, value: u8) {
        match &mut self.save {
            SaveChip::None | SaveChip::Eeprom(_) => {}
            SaveChip::Sram(data) => {
                data[(addr & 0x7FFF) as usize] = value;
                self.save_dirty = true;
            }
            SaveChip::Flash(flash) => {
                flash.write(addr, value);
                self.save_dirty = true;
            }
        }
    }

    fn gpio_offset(addr: u32) -> Option<u32> {
        // Only the cartridge-header GPIO hole (mirrored across WS0–WS2).
        match addr & 0x1_FFFF {
            0xC4 => Some(0xC4),
            0xC6 => Some(0xC6),
            0xC8 => Some(0xC8),
            _ => None,
        }
    }

    pub(super) fn load_mgba_debug(&self, off: u32, size: u32) -> u32 {
        match off {
            // mGBA returns `0x1A00` when the debug port is live.
            0x00FF_F780 => u32::from(if self.mgba_enabled { 0x1A00u16 } else { 0 }),
            0x00FF_F600..=0x00FF_F6FF => {
                let i = (off - 0x00FF_F600) as usize;
                match size {
                    1 => u32::from(self.mgba_buf.get(i).copied().unwrap_or(0)),
                    2 => {
                        let lo = u32::from(self.mgba_buf.get(i).copied().unwrap_or(0));
                        let hi = u32::from(self.mgba_buf.get(i + 1).copied().unwrap_or(0));
                        lo | (hi << 8)
                    }
                    _ => {
                        let mut w = 0u32;
                        for n in 0..4 {
                            w |= u32::from(self.mgba_buf.get(i + n).copied().unwrap_or(0))
                                << (8 * n);
                        }
                        w
                    }
                }
            }
            _ => 0,
        }
    }

    /// mGBA emulator printf protocol (enable `0xC0DE`, string at `0xFFF600`, flush via flags).
    pub(super) fn store_mgba_debug(&mut self, off: u32, value: u32, size: u32) {
        match off {
            0x00FF_F780 if size >= 2 => {
                self.mgba_enabled = (value as u16) == 0xC0DE;
            }
            0x00FF_F700 if size >= 2 && self.mgba_enabled => {
                // Bit 8 set = print the string buffer as one line.
                if value & 0x100 != 0 {
                    let end = self
                        .mgba_buf
                        .iter()
                        .position(|&b| b == 0)
                        .unwrap_or(self.mgba_buf.len());
                    let line = String::from_utf8_lossy(&self.mgba_buf[..end])
                        .trim_end_matches(['\r', '\n'])
                        .to_string();
                    if !line.is_empty() {
                        self.mgba_log.push(line);
                    }
                    self.mgba_buf.fill(0);
                }
            }
            0x00FF_F600..=0x00FF_F6FF if self.mgba_enabled => {
                let i = (off - 0x00FF_F600) as usize;
                match size {
                    1 if i < self.mgba_buf.len() => self.mgba_buf[i] = value as u8,
                    2 if i + 1 < self.mgba_buf.len() => {
                        self.mgba_buf[i] = value as u8;
                        self.mgba_buf[i + 1] = (value >> 8) as u8;
                    }
                    4 if i + 3 < self.mgba_buf.len() => {
                        self.mgba_buf[i] = value as u8;
                        self.mgba_buf[i + 1] = (value >> 8) as u8;
                        self.mgba_buf[i + 2] = (value >> 16) as u8;
                        self.mgba_buf[i + 3] = (value >> 24) as u8;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Read cart GPIO when present. Returns `None` so the caller falls through to ROM.
    pub(super) fn read_gpio(&mut self, addr: u32, size: u32) -> Option<u32> {
        let offset = Self::gpio_offset(addr)?;
        let gpio = self.gpio.as_mut()?;
        if !gpio.has_rtc() {
            if !self.gpio_warned {
                self.gpio_warned = true;
                self.warn_lines.push(gpio_reject_line().to_string());
            }
            return Some(0);
        }
        let half = u32::from(gpio.read(offset));
        let value = match size {
            1 => half & 0xFF,
            2 => half,
            _ => half,
        };
        Some(value)
    }

    /// Write cart GPIO when present.
    pub(super) fn write_gpio(&mut self, addr: u32, value: u32) {
        let Some(offset) = Self::gpio_offset(addr) else {
            return;
        };
        let Some(gpio) = self.gpio.as_mut() else {
            return;
        };
        if !gpio.has_rtc() {
            if !self.gpio_warned {
                self.gpio_warned = true;
                self.warn_lines.push(gpio_reject_line().to_string());
            }
            return;
        }
        gpio.write(offset, value as u16);
        self.save_dirty = true;
    }

    pub(super) fn eeprom_region(addr: u32) -> bool {
        (0x0D00_0000..=0x0DFF_FFFF).contains(&addr)
    }
}
