//! Four GBA PSG channels (square 1/2, wave, noise).
//!
//! Cited: GBATEK Sound Controller.
//! <https://problemkaputt.de/gbatek.htm>
//!
//! Sound clock is CPU / 4. One sound tick advances per 4 CPU cycles; leftovers carry.

/// Duty patterns: 12.5%, 25%, 50%, 75% (one high phase out of eight, then two, four, six).
const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 1, 1, 1, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 1, 1, 1],
];

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
struct Envelope {
    initial: u8,
    direction_up: bool,
    period: u8,
    volume: u8,
    timer: u8,
}

impl Envelope {
    fn from_nr2_high(reg: u16) -> Self {
        let initial = ((reg >> 12) & 0xF) as u8;
        let direction_up = (reg & (1 << 11)) != 0;
        let period = ((reg >> 8) & 0x7) as u8;
        Self {
            initial,
            direction_up,
            period,
            volume: initial,
            timer: period,
        }
    }

    fn trigger(&mut self) {
        self.volume = self.initial;
        self.timer = self.period;
    }

    /// Envelope clock (64 Hz frame step). Period 0 keeps volume fixed.
    fn tick(&mut self) {
        if self.period == 0 {
            return;
        }
        if self.timer > 0 {
            self.timer -= 1;
        }
        if self.timer == 0 {
            self.timer = self.period;
            if self.direction_up {
                if self.volume < 15 {
                    self.volume += 1;
                }
            } else if self.volume > 0 {
                self.volume -= 1;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
struct Sweep {
    shift: u8,
    negate: bool,
    time: u8,
    timer: u8,
    enabled: bool,
    shadow: u16,
}

impl Sweep {
    fn from_nr10(reg: u16) -> Self {
        Self {
            shift: (reg & 0x7) as u8,
            negate: (reg & 0x8) != 0,
            time: ((reg >> 4) & 0x7) as u8,
            timer: 0,
            enabled: false,
            shadow: 0,
        }
    }

    fn trigger(&mut self, freq: u16) {
        self.shadow = freq & 0x7FF;
        self.timer = if self.time == 0 { 8 } else { self.time };
        self.enabled = self.time != 0 || self.shift != 0;
        // Sweep time 0 is a no-op for ongoing updates (see tick).
    }

    /// Sweep clock (128 Hz). Time 0 does not change frequency.
    fn tick(&mut self, freq: &mut u16) -> bool {
        if self.time == 0 {
            return true;
        }
        if self.timer > 0 {
            self.timer -= 1;
        }
        if self.timer != 0 {
            return true;
        }
        self.timer = if self.time == 0 { 8 } else { self.time };
        if !self.enabled || self.shift == 0 {
            return true;
        }
        let delta = self.shadow >> self.shift;
        let new_freq = if self.negate {
            self.shadow.wrapping_sub(delta)
        } else {
            self.shadow.wrapping_add(delta)
        };
        if new_freq > 0x7FF {
            return false;
        }
        self.shadow = new_freq;
        *freq = new_freq;
        true
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct Square {
    sweep: Sweep,
    has_sweep: bool,
    duty: u8,
    length: u8,
    envelope: Envelope,
    freq: u16,
    length_enable: bool,
    enabled: bool,
    timer: u32,
    duty_step: u8,
    length_counter: u8,
    sample: i16,
}

impl Square {
    fn new(has_sweep: bool) -> Self {
        Self {
            has_sweep,
            ..Default::default()
        }
    }

    fn period(&self) -> u32 {
        (2048u32.saturating_sub(u32::from(self.freq & 0x7FF))) * 4
    }

    fn write_sweep(&mut self, value: u16) {
        self.sweep = Sweep::from_nr10(value);
    }

    fn write_duty_env(&mut self, value: u16) {
        self.duty = ((value >> 6) & 0x3) as u8;
        self.length = (value & 0x3F) as u8;
        self.envelope = Envelope::from_nr2_high(value);
    }

    fn write_freq(&mut self, value: u16) {
        self.freq = value & 0x7FF;
        self.length_enable = (value & (1 << 14)) != 0;
        if value & (1 << 15) != 0 {
            self.trigger();
        }
    }

    fn trigger(&mut self) {
        self.enabled = true;
        self.envelope.trigger();
        self.timer = self.period();
        self.duty_step = 0;
        self.length_counter = if self.length == 0 {
            64
        } else {
            64 - self.length
        };
        if self.has_sweep {
            self.sweep.trigger(self.freq);
        }
    }

    fn silence(&mut self) {
        self.enabled = false;
        self.sample = 0;
    }

    fn tick_sound(&mut self) {
        if !self.enabled {
            self.sample = 0;
            return;
        }
        if self.timer > 0 {
            self.timer -= 1;
        }
        if self.timer == 0 {
            self.timer = self.period().max(1);
            self.duty_step = (self.duty_step + 1) & 7;
        }
        let high = DUTY_TABLE[self.duty as usize][self.duty_step as usize] != 0;
        let vol = i16::from(self.envelope.volume);
        self.sample = if high { vol } else { -vol };
    }

    fn tick_length(&mut self) {
        if self.length_enable && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.silence();
            }
        }
    }

    fn tick_envelope(&mut self) {
        if self.enabled {
            self.envelope.tick();
        }
    }

    fn tick_sweep(&mut self) {
        if self.has_sweep && self.enabled && !self.sweep.tick(&mut self.freq) {
            self.silence();
        }
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct Wave {
    dac_on: bool,
    length: u8,
    volume_code: u8,
    freq: u16,
    length_enable: bool,
    enabled: bool,
    timer: u32,
    position: u8,
    length_counter: u16,
    ram: [u8; 16],
    sample: i16,
}

impl Wave {
    fn period(&self) -> u32 {
        (2048u32.saturating_sub(u32::from(self.freq & 0x7FF))) * 2
    }

    fn write_dac(&mut self, value: u16) {
        self.dac_on = (value & 0x80) != 0;
        if !self.dac_on {
            self.silence();
        }
    }

    fn write_length_vol(&mut self, value: u16) {
        self.length = (value & 0xFF) as u8;
        self.volume_code = ((value >> 13) & 0x3) as u8;
    }

    fn write_freq(&mut self, value: u16) {
        self.freq = value & 0x7FF;
        self.length_enable = (value & (1 << 14)) != 0;
        if value & (1 << 15) != 0 {
            self.trigger();
        }
    }

    fn write_ram(&mut self, offset: u32, value: u16) {
        let i = (offset - 0x30) as usize;
        if i < 15 {
            self.ram[i] = (value & 0xFF) as u8;
            self.ram[i + 1] = (value >> 8) as u8;
        } else if i == 15 {
            self.ram[15] = (value & 0xFF) as u8;
        }
    }

    fn trigger(&mut self) {
        if !self.dac_on {
            self.enabled = false;
            self.sample = 0;
            return;
        }
        self.enabled = true;
        self.timer = self.period();
        self.position = 0;
        self.length_counter = if self.length == 0 {
            256
        } else {
            256 - u16::from(self.length)
        };
    }

    fn silence(&mut self) {
        self.enabled = false;
        self.sample = 0;
    }

    fn nibble(&self) -> u8 {
        let byte = self.ram[(self.position / 2) as usize];
        // High nibble first, then low (GBATEK wave RAM).
        if self.position & 1 == 0 {
            byte >> 4
        } else {
            byte & 0xF
        }
    }

    fn tick_sound(&mut self) {
        if !self.enabled || !self.dac_on {
            self.sample = 0;
            return;
        }
        if self.timer > 0 {
            self.timer -= 1;
        }
        if self.timer == 0 {
            self.timer = self.period().max(1);
            self.position = (self.position + 1) & 31;
        }
        let mut s = self.nibble();
        match self.volume_code {
            0 => s = 0,
            1 => {}
            2 => s >>= 1,
            3 => s >>= 2,
            _ => s = 0,
        }
        // Center 4-bit sample into roughly -15..=15.
        self.sample = i16::from(s) - 8;
        if self.volume_code == 0 {
            self.sample = 0;
        }
    }

    fn tick_length(&mut self) {
        if self.length_enable && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.silence();
            }
        }
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct Noise {
    length: u8,
    envelope: Envelope,
    divisor: u8,
    width7: bool,
    shift: u8,
    length_enable: bool,
    enabled: bool,
    timer: u32,
    lfsr: u16,
    length_counter: u8,
    sample: i16,
}

impl Noise {
    fn period(&self) -> u32 {
        let r = u32::from(self.divisor);
        let s = u32::from(self.shift);
        let base = if r == 0 { 8 } else { r * 16 };
        base << s
    }

    fn write_env(&mut self, value: u16) {
        self.length = (value & 0x3F) as u8;
        self.envelope = Envelope::from_nr2_high(value);
    }

    fn write_poly(&mut self, value: u16) {
        self.divisor = (value & 0x7) as u8;
        self.width7 = (value & 0x8) != 0;
        self.shift = ((value >> 4) & 0xF) as u8;
        self.length_enable = (value & (1 << 14)) != 0;
        if value & (1 << 15) != 0 {
            self.trigger();
        }
    }

    fn trigger(&mut self) {
        self.enabled = true;
        self.envelope.trigger();
        self.timer = self.period().max(1);
        self.lfsr = 0x7FFF;
        self.length_counter = if self.length == 0 {
            64
        } else {
            64 - self.length
        };
    }

    fn silence(&mut self) {
        self.enabled = false;
        self.sample = 0;
    }

    fn tick_sound(&mut self) {
        if !self.enabled {
            self.sample = 0;
            return;
        }
        if self.timer > 0 {
            self.timer -= 1;
        }
        if self.timer == 0 {
            self.timer = self.period().max(1);
            let bit = (self.lfsr ^ (self.lfsr >> 1)) & 1;
            self.lfsr = (self.lfsr >> 1) | (bit << 14);
            if self.width7 {
                self.lfsr = (self.lfsr & !0x40) | (bit << 6);
            }
        }
        let vol = i16::from(self.envelope.volume);
        // LFSR bit 0 clear => output volume (Game Boy convention).
        self.sample = if self.lfsr & 1 == 0 { vol } else { -vol };
    }

    fn tick_length(&mut self) {
        if self.length_enable && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.silence();
            }
        }
    }

    fn tick_envelope(&mut self) {
        if self.enabled {
            self.envelope.tick();
        }
    }
}

/// GBA PSG: two squares, wave, and noise.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Psg {
    master: bool,
    square1: Square,
    square2: Square,
    wave: Wave,
    noise: Noise,
    cycle_accum: u32,
    /// Frame sequencer step 0..7; clocks every 8192 sound ticks (~512 Hz).
    frame_timer: u32,
    frame_step: u8,
}

impl Psg {
    pub fn new() -> Self {
        Self {
            master: false,
            square1: Square::new(true),
            square2: Square::new(false),
            wave: Wave::default(),
            noise: Noise::default(),
            cycle_accum: 0,
            frame_timer: 8192,
            frame_step: 0,
        }
    }

    pub fn write(&mut self, offset: u32, value: u16) {
        if offset == 0x20 {
            let enable = (value & 0x80) != 0;
            if !enable {
                self.master = false;
                self.square1.silence();
                self.square2.silence();
                self.wave.silence();
                self.noise.silence();
            } else {
                self.master = true;
            }
            return;
        }

        if !self.master {
            return;
        }

        match offset {
            0x00 => self.square1.write_sweep(value),
            0x02 => self.square1.write_duty_env(value),
            0x04 => self.square1.write_freq(value),
            0x08 => self.square2.write_duty_env(value),
            0x0C => self.square2.write_freq(value),
            0x10 => self.wave.write_dac(value),
            0x12 => self.wave.write_length_vol(value),
            0x14 => self.wave.write_freq(value),
            0x18 => self.noise.write_env(value),
            0x1C => self.noise.write_poly(value),
            0x30..=0x3F => self.wave.write_ram(offset, value),
            _ => {}
        }
    }

    /// SOUNDCNT_X: bits 0–3 channel status, bit 7 master enable.
    pub fn read_master(&self) -> u16 {
        let mut v = 0u16;
        if self.square1.enabled {
            v |= 1 << 0;
        }
        if self.square2.enabled {
            v |= 1 << 1;
        }
        if self.wave.enabled {
            v |= 1 << 2;
        }
        if self.noise.enabled {
            v |= 1 << 3;
        }
        if self.master {
            v |= 1 << 7;
        }
        v
    }

    /// Advance `cpu_cycles`. Returns latest sample per channel in -15..=15.
    /// Index 0 square1, 1 square2, 2 wave, 3 noise. Off channels are 0.
    pub fn step(&mut self, cpu_cycles: u32) -> [i16; 4] {
        if !self.master {
            return [0, 0, 0, 0];
        }

        self.cycle_accum += cpu_cycles;
        while self.cycle_accum >= 4 {
            self.cycle_accum -= 4;
            self.tick_sound();
        }

        [
            if self.square1.enabled {
                self.square1.sample
            } else {
                0
            },
            if self.square2.enabled {
                self.square2.sample
            } else {
                0
            },
            if self.wave.enabled {
                self.wave.sample
            } else {
                0
            },
            if self.noise.enabled {
                self.noise.sample
            } else {
                0
            },
        ]
    }

    fn tick_sound(&mut self) {
        self.square1.tick_sound();
        self.square2.tick_sound();
        self.wave.tick_sound();
        self.noise.tick_sound();

        if self.frame_timer > 0 {
            self.frame_timer -= 1;
        }
        if self.frame_timer == 0 {
            self.frame_timer = 8192;
            self.clock_frame();
        }
    }

    fn clock_frame(&mut self) {
        let step = self.frame_step;
        self.frame_step = (self.frame_step + 1) & 7;

        // Length: steps 0, 2, 4, 6
        if step & 1 == 0 {
            self.square1.tick_length();
            self.square2.tick_length();
            self.wave.tick_length();
            self.noise.tick_length();
        }
        // Sweep: steps 2, 6
        if step == 2 || step == 6 {
            self.square1.tick_sweep();
        }
        // Envelope: steps 7
        if step == 7 {
            self.square1.tick_envelope();
            self.square2.tick_envelope();
            self.noise.tick_envelope();
        }
    }
}

impl Default for Psg {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn master_off_silences_even_after_trigger() {
        let mut psg = Psg::new();
        psg.write(0x20, 0x80);
        // Square 2: volume 15, duty 50%, restart
        psg.write(0x08, 0xF000 | (2 << 6));
        psg.write(0x0C, 0x8000 | 0x100);
        let _ = psg.step(10_000);
        psg.write(0x20, 0x00);
        assert_eq!(psg.step(10_000), [0, 0, 0, 0]);
    }

    #[test]
    fn square2_produces_nonzero_sample() {
        let mut psg = Psg::new();
        psg.write(0x20, 0x80);
        // volume 15, duty 2 (50%), envelope period 0
        psg.write(0x08, 0xF000 | (2 << 6));
        // low frequency, restart
        psg.write(0x0C, 0x8000 | 0x100);
        let mut saw = false;
        for _ in 0..100_000 {
            let s = psg.step(4);
            if s[1] != 0 {
                saw = true;
                break;
            }
        }
        assert!(saw, "square 2 should emit a non-zero sample");
    }

    #[test]
    fn wave_produces_nonzero_sample() {
        let mut psg = Psg::new();
        psg.write(0x20, 0x80);
        // Wave RAM: high nibble 0xF in first byte (plays first)
        psg.write(0x30, 0x00F0);
        // DAC on
        psg.write(0x10, 0x80);
        // volume code 1 (100%)
        psg.write(0x12, 1 << 13);
        // restart, moderate frequency
        psg.write(0x14, 0x8000 | 0x100);
        let mut saw = false;
        for _ in 0..100_000 {
            let s = psg.step(4);
            if s[2] != 0 {
                saw = true;
                break;
            }
        }
        assert!(saw, "wave should emit a non-zero sample");
    }

    #[test]
    fn wave_plays_high_nibble_before_low() {
        let mut psg = Psg::new();
        psg.write(0x20, 0x80);
        // Byte 0xF0: high nibble 15, then low nibble 0
        psg.write(0x30, 0x00F0);
        psg.write(0x10, 0x80);
        psg.write(0x12, 1 << 13);
        // Low frequency so the first nibble is held for many steps
        psg.write(0x14, 0x8000);
        let mut first = None;
        let mut second = None;
        for _ in 0..50_000 {
            let s = psg.step(4)[2];
            match first {
                None => {
                    if s != 0 {
                        first = Some(s);
                    }
                }
                Some(a) if second.is_none() && s != a => {
                    second = Some(s);
                    break;
                }
                _ => {}
            }
        }
        assert_eq!(first, Some(7), "high nibble 15 centers to 7");
        assert_eq!(second, Some(-8), "low nibble 0 centers to -8");
    }

    #[test]
    fn noise_produces_nonzero_sample() {
        let mut psg = Psg::new();
        psg.write(0x20, 0x80);
        // volume 15, envelope period 0
        psg.write(0x18, 0xF000);
        // small shift/divisor, restart
        psg.write(0x1C, 0x8000);
        let mut saw = false;
        for _ in 0..100_000 {
            let s = psg.step(4);
            if s[3] != 0 {
                saw = true;
                break;
            }
        }
        assert!(saw, "noise should emit a non-zero sample");
    }

    #[test]
    fn square1_enable_bit_and_master_clear() {
        let mut psg = Psg::new();
        psg.write(0x20, 0x80);
        psg.write(0x02, 0xF000 | (2 << 6));
        psg.write(0x04, 0x8000 | 0x100);
        let master = psg.read_master();
        assert_ne!(master & 1, 0, "bit 0 set after square 1 restart");
        assert_ne!(master & 0x80, 0, "master bit set");

        psg.write(0x20, 0x00);
        let cleared = psg.read_master();
        assert_eq!(cleared & 0x80, 0);
        assert_eq!(cleared & 0xF, 0);
    }
}
