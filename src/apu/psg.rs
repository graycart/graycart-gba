//! PSG channels 1–4 (CGB-like + AGB wave deltas) — P6.
//!
//! Cited: GBATEK — Sound Channel 1–4 / Wave RAM
//!   https://problemkaputt.de/gbatek.htm
//! Cited: jsgroth — mixing bias notes (secondary)
//!   https://jsgroth.dev/blog/posts/gba-audio/
//! Research: Project store `docs/graycart-gba/04-apu.md` §3
//! Note: functional envelopes/length/duty/sweep/LFSR; not cycle-perfect vs HW.

use super::regs::ApuRegs;

/// Duty patterns (8 steps): 12.5 / 25 / 50 / 75%.
const DUTY: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

/// Square / noise / wave generators + 512 Hz frame sequencer.
#[derive(Debug, Clone)]
pub struct Psg {
    pub ch1: SquareSweep,
    pub ch2: Square,
    pub ch3: Wave,
    pub ch4: Noise,
    /// Cycles toward next 512 Hz frame tick (system clock).
    frame_accum: u32,
    frame_step: u8,
}

impl Default for Psg {
    fn default() -> Self {
        Self::new()
    }
}

impl Psg {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ch1: SquareSweep::new(),
            ch2: Square::new(),
            ch3: Wave::new(),
            ch4: Noise::new(),
            frame_accum: 0,
            frame_step: 0,
        }
    }

    pub fn reset_all(&mut self) {
        *self = Self::new();
    }

    /// Advance PSG by `cycles` system clocks (~16.78 MHz).
    pub fn step(&mut self, cycles: u64, regs: &mut ApuRegs) {
        if cycles == 0 || !regs.master_enabled() {
            return;
        }
        // Channel generators tick at ~1.048576 MHz equivalent via period counters
        // derived from 131072/(2048-n) — we count system clocks with period*16.
        let mut left = cycles;
        while left > 0 {
            let chunk = left.min(64);
            self.ch1.tick(chunk, regs);
            self.ch2.tick(chunk, regs);
            self.ch3.tick(chunk, regs);
            self.ch4.tick(chunk, regs);
            self.frame_accum += chunk as u32;
            // 512 Hz frame sequencer: 16777216 / 512 = 32768
            while self.frame_accum >= 32768 {
                self.frame_accum -= 32768;
                self.frame_sequencer(regs);
            }
            left -= chunk;
        }
        regs.channel_on = self.on_flags();
    }

    fn frame_sequencer(&mut self, regs: &mut ApuRegs) {
        let step = self.frame_step;
        // Length clock: steps 0,2,4,6
        if step & 1 == 0 {
            self.ch1.clock_length(regs);
            self.ch2.clock_length(regs);
            self.ch3.clock_length(regs);
            self.ch4.clock_length(regs);
        }
        // Sweep: steps 2,6
        if step == 2 || step == 6 {
            self.ch1.clock_sweep(regs);
        }
        // Envelope: steps 7
        if step == 7 {
            self.ch1.clock_envelope();
            self.ch2.clock_envelope();
            self.ch4.clock_envelope();
        }
        self.frame_step = (self.frame_step + 1) & 7;
    }

    #[must_use]
    pub fn on_flags(&self) -> u8 {
        let mut f = 0u8;
        if self.ch1.enabled {
            f |= 1;
        }
        if self.ch2.enabled {
            f |= 2;
        }
        if self.ch3.enabled {
            f |= 4;
        }
        if self.ch4.enabled {
            f |= 8;
        }
        f
    }

    /// Digital sample for channel (−15…+15 style after bias).
    #[must_use]
    pub fn sample_ch1(&self) -> i32 {
        self.ch1.digital_sample()
    }
    #[must_use]
    pub fn sample_ch2(&self) -> i32 {
        self.ch2.digital_sample()
    }
    #[must_use]
    pub fn sample_ch3(&self, regs: &ApuRegs) -> i32 {
        self.ch3.digital_sample(regs)
    }
    #[must_use]
    pub fn sample_ch4(&self) -> i32 {
        self.ch4.digital_sample()
    }

    pub fn trigger_ch1(&mut self, regs: &ApuRegs) {
        self.ch1.trigger(regs);
    }
    pub fn trigger_ch2(&mut self, regs: &ApuRegs) {
        self.ch2.trigger(regs);
    }
    pub fn trigger_ch3(&mut self, regs: &ApuRegs) {
        self.ch3.trigger(regs);
    }
    pub fn trigger_ch4(&mut self, regs: &ApuRegs) {
        self.ch4.trigger(regs);
    }
}

#[derive(Debug, Clone)]
pub struct Square {
    pub enabled: bool,
    duty: u8,
    length: u8,
    length_enabled: bool,
    volume: u8,
    envelope_add: bool,
    envelope_period: u8,
    envelope_timer: u8,
    freq: u16,
    period_counter: u32,
    duty_pos: u8,
}

impl Square {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            enabled: false,
            duty: 0,
            length: 0,
            length_enabled: false,
            volume: 0,
            envelope_add: false,
            envelope_period: 0,
            envelope_timer: 0,
            freq: 0,
            period_counter: 0,
            duty_pos: 0,
        }
    }

    pub fn trigger(&mut self, regs: &ApuRegs) {
        let n11 = regs.sound2cnt_l;
        let n14 = regs.sound2cnt_h;
        self.duty = ((n11 >> 6) & 0b11) as u8;
        self.length = 64 - (n11 & 0x3F) as u8;
        self.volume = ((n11 >> 12) & 0xF) as u8;
        self.envelope_add = n11 & (1 << 11) != 0;
        self.envelope_period = ((n11 >> 8) & 0x7) as u8;
        self.envelope_timer = self.envelope_period;
        self.freq = n14 & 0x7FF;
        self.length_enabled = n14 & (1 << 14) != 0;
        self.enabled = true;
        self.duty_pos = 0;
        self.period_counter = square_period(self.freq);
        if self.length == 0 {
            self.length = 64;
        }
    }

    pub fn load_from_ch1_regs(&mut self, regs: &ApuRegs) {
        let n11 = regs.sound1cnt_h;
        let n14 = regs.sound1cnt_x;
        self.duty = ((n11 >> 6) & 0b11) as u8;
        self.length = 64 - (n11 & 0x3F) as u8;
        self.volume = ((n11 >> 12) & 0xF) as u8;
        self.envelope_add = n11 & (1 << 11) != 0;
        self.envelope_period = ((n11 >> 8) & 0x7) as u8;
        self.envelope_timer = self.envelope_period;
        self.freq = n14 & 0x7FF;
        self.length_enabled = n14 & (1 << 14) != 0;
        self.enabled = true;
        self.duty_pos = 0;
        self.period_counter = square_period(self.freq);
        if self.length == 0 {
            self.length = 64;
        }
    }

    fn tick(&mut self, cycles: u64, _regs: &ApuRegs) {
        if !self.enabled {
            return;
        }
        let mut c = cycles;
        while c > 0 && self.enabled {
            if self.period_counter == 0 {
                self.period_counter = square_period(self.freq);
                self.duty_pos = (self.duty_pos + 1) & 7;
            }
            let step = u64::from(self.period_counter).min(c);
            self.period_counter -= step as u32;
            c -= step;
        }
    }

    fn clock_length(&mut self, _regs: &ApuRegs) {
        if self.length_enabled && self.length > 0 {
            self.length -= 1;
            if self.length == 0 {
                self.enabled = false;
            }
        }
    }

    fn clock_envelope(&mut self) {
        if self.envelope_period == 0 || !self.enabled {
            return;
        }
        if self.envelope_timer > 0 {
            self.envelope_timer -= 1;
        }
        if self.envelope_timer == 0 {
            self.envelope_timer = self.envelope_period;
            if self.envelope_add {
                if self.volume < 15 {
                    self.volume += 1;
                }
            } else if self.volume > 0 {
                self.volume -= 1;
            }
        }
    }

    #[must_use]
    pub fn digital_sample(&self) -> i32 {
        if !self.enabled {
            return 0;
        }
        let bit = DUTY[self.duty as usize][self.duty_pos as usize];
        let sample = if bit != 0 { i32::from(self.volume) } else { 0 };
        // pulse bias: 2*sample - volume
        2 * sample - i32::from(self.volume)
    }
}

#[derive(Debug, Clone)]
pub struct SquareSweep {
    pub square: Square,
    pub enabled: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_timer: u8,
    shadow_freq: u16,
}

impl SquareSweep {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            square: Square::new(),
            enabled: false,
            sweep_period: 0,
            sweep_negate: false,
            sweep_shift: 0,
            sweep_timer: 0,
            shadow_freq: 0,
        }
    }

    pub fn trigger(&mut self, regs: &ApuRegs) {
        self.square.load_from_ch1_regs(regs);
        let nr10 = regs.sound1cnt_l;
        self.sweep_period = ((nr10 >> 4) & 0x7) as u8;
        self.sweep_negate = nr10 & (1 << 3) != 0;
        self.sweep_shift = (nr10 & 0x7) as u8;
        self.shadow_freq = self.square.freq;
        self.sweep_timer = if self.sweep_period == 0 {
            8
        } else {
            self.sweep_period
        };
        self.enabled = self.square.enabled;
        if self.sweep_shift != 0 && !self.sweep_calc(true) {
            self.enabled = false;
            self.square.enabled = false;
        }
    }

    fn tick(&mut self, cycles: u64, regs: &ApuRegs) {
        self.square.tick(cycles, regs);
        self.enabled = self.square.enabled;
    }

    fn clock_length(&mut self, regs: &mut ApuRegs) {
        self.square.clock_length(regs);
        self.enabled = self.square.enabled;
    }

    fn clock_envelope(&mut self) {
        self.square.clock_envelope();
    }

    fn clock_sweep(&mut self, regs: &mut ApuRegs) {
        if !self.enabled {
            return;
        }
        if self.sweep_timer > 0 {
            self.sweep_timer -= 1;
        }
        if self.sweep_timer == 0 {
            self.sweep_timer = if self.sweep_period == 0 {
                8
            } else {
                self.sweep_period
            };
            if self.sweep_period != 0 {
                let ok = self.sweep_calc(false);
                if !ok {
                    self.enabled = false;
                    self.square.enabled = false;
                } else if self.sweep_shift != 0 {
                    // Update freq in regs shadow
                    regs.sound1cnt_x = (regs.sound1cnt_x & !0x7FF) | (self.shadow_freq & 0x7FF);
                    self.square.freq = self.shadow_freq;
                    let _ = self.sweep_calc(true); // overflow check
                }
            }
        }
    }

    fn sweep_calc(&mut self, update: bool) -> bool {
        let delta = self.shadow_freq >> self.sweep_shift;
        let new = if self.sweep_negate {
            self.shadow_freq.wrapping_sub(delta)
        } else {
            self.shadow_freq.wrapping_add(delta)
        };
        if new > 0x7FF {
            return false;
        }
        if update && self.sweep_shift != 0 {
            self.shadow_freq = new;
            self.square.freq = new;
        }
        true
    }

    #[must_use]
    pub fn digital_sample(&self) -> i32 {
        if !self.enabled {
            return 0;
        }
        self.square.digital_sample()
    }
}

#[derive(Debug, Clone)]
pub struct Wave {
    pub enabled: bool,
    length: u16,
    length_enabled: bool,
    volume_code: u8,
    force_75: bool,
    freq: u16,
    period_counter: u32,
    sample_index: u8,
    dac_power: bool,
}

impl Wave {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            enabled: false,
            length: 0,
            length_enabled: false,
            volume_code: 0,
            force_75: false,
            freq: 0,
            period_counter: 0,
            sample_index: 0,
            dac_power: false,
        }
    }

    pub fn trigger(&mut self, regs: &ApuRegs) {
        let nr30 = regs.sound3cnt_l;
        let nr31 = regs.sound3cnt_h;
        let nr34 = regs.sound3cnt_x;
        self.dac_power = nr30 & (1 << 7) != 0;
        self.length = 256 - u16::from(nr31 as u8);
        self.force_75 = nr31 & (1 << 15) != 0;
        self.volume_code = ((nr31 >> 13) & 0b11) as u8;
        self.freq = nr34 & 0x7FF;
        self.length_enabled = nr34 & (1 << 14) != 0;
        self.sample_index = 0;
        self.period_counter = wave_period(self.freq);
        self.enabled = self.dac_power;
        if self.length == 0 {
            self.length = 256;
        }
    }

    fn tick(&mut self, cycles: u64, regs: &ApuRegs) {
        if !self.enabled {
            return;
        }
        // Update dac from NR30 live
        self.dac_power = regs.sound3cnt_l & (1 << 7) != 0;
        if !self.dac_power {
            self.enabled = false;
            return;
        }
        let mut c = cycles;
        while c > 0 && self.enabled {
            if self.period_counter == 0 {
                self.period_counter = wave_period(self.freq);
                let max = if regs.wave_dimension_64() { 64 } else { 32 };
                self.sample_index = (self.sample_index + 1) % max;
            }
            let step = u64::from(self.period_counter).min(c);
            self.period_counter -= step as u32;
            c -= step;
        }
    }

    fn clock_length(&mut self, _regs: &ApuRegs) {
        if self.length_enabled && self.length > 0 {
            self.length -= 1;
            if self.length == 0 {
                self.enabled = false;
            }
        }
    }

    #[must_use]
    pub fn digital_sample(&self, regs: &ApuRegs) -> i32 {
        if !self.enabled {
            return 0;
        }
        let bank = if regs.wave_dimension_64() {
            if self.sample_index < 32 {
                regs.wave_play_bank()
            } else {
                1 - regs.wave_play_bank()
            }
        } else {
            regs.wave_play_bank()
        };
        let idx = (self.sample_index % 32) as usize;
        let byte = regs.wave_ram[bank][idx / 2];
        let nibble = if idx & 1 == 0 { byte >> 4 } else { byte & 0x0F };
        let mut sample = i32::from(nibble);
        // Volume
        if self.force_75 {
            sample = (sample * 3) / 4;
        } else {
            sample = match self.volume_code {
                0 => 0,
                1 => sample,
                2 => sample >> 1,
                3 => sample >> 2,
                _ => 0,
            };
        }
        // wave bias always −15
        2 * sample - 15
    }
}

#[derive(Debug, Clone)]
pub struct Noise {
    pub enabled: bool,
    length: u8,
    length_enabled: bool,
    volume: u8,
    envelope_add: bool,
    envelope_period: u8,
    envelope_timer: u8,
    clock_shift: u8,
    width7: bool,
    divisor_code: u8,
    lfsr: u16,
    period_counter: u32,
}

impl Noise {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            enabled: false,
            length: 0,
            length_enabled: false,
            volume: 0,
            envelope_add: false,
            envelope_period: 0,
            envelope_timer: 0,
            clock_shift: 0,
            width7: false,
            divisor_code: 0,
            lfsr: 0x7FFF,
            period_counter: 0,
        }
    }

    pub fn trigger(&mut self, regs: &ApuRegs) {
        let n41 = regs.sound4cnt_l;
        let n43 = regs.sound4cnt_h;
        self.length = 64 - (n41 & 0x3F) as u8;
        self.volume = ((n41 >> 12) & 0xF) as u8;
        self.envelope_add = n41 & (1 << 11) != 0;
        self.envelope_period = ((n41 >> 8) & 0x7) as u8;
        self.envelope_timer = self.envelope_period;
        self.clock_shift = ((n43 >> 4) & 0xF) as u8;
        self.width7 = n43 & (1 << 3) != 0;
        self.divisor_code = (n43 & 0x7) as u8;
        self.length_enabled = n43 & (1 << 14) != 0;
        self.lfsr = if self.width7 { 0x7F } else { 0x7FFF };
        self.period_counter = noise_period(self.divisor_code, self.clock_shift);
        self.enabled = true;
        if self.length == 0 {
            self.length = 64;
        }
    }

    fn tick(&mut self, cycles: u64, _regs: &ApuRegs) {
        if !self.enabled {
            return;
        }
        let mut c = cycles;
        while c > 0 && self.enabled {
            if self.period_counter == 0 {
                self.period_counter = noise_period(self.divisor_code, self.clock_shift);
                let bit = (self.lfsr ^ (self.lfsr >> 1)) & 1;
                self.lfsr >>= 1;
                self.lfsr &= !(1 << 14);
                self.lfsr |= bit << 14;
                if self.width7 {
                    self.lfsr &= !(1 << 6);
                    self.lfsr |= bit << 6;
                }
            }
            let step = u64::from(self.period_counter).min(c);
            self.period_counter -= step as u32;
            c -= step;
        }
    }

    fn clock_length(&mut self, _regs: &ApuRegs) {
        if self.length_enabled && self.length > 0 {
            self.length -= 1;
            if self.length == 0 {
                self.enabled = false;
            }
        }
    }

    fn clock_envelope(&mut self) {
        if self.envelope_period == 0 || !self.enabled {
            return;
        }
        if self.envelope_timer > 0 {
            self.envelope_timer -= 1;
        }
        if self.envelope_timer == 0 {
            self.envelope_timer = self.envelope_period;
            if self.envelope_add {
                if self.volume < 15 {
                    self.volume += 1;
                }
            } else if self.volume > 0 {
                self.volume -= 1;
            }
        }
    }

    #[must_use]
    pub fn digital_sample(&self) -> i32 {
        if !self.enabled {
            return 0;
        }
        let bit = !(self.lfsr & 1);
        let sample = if bit != 0 { i32::from(self.volume) } else { 0 };
        2 * sample - i32::from(self.volume)
    }
}

#[inline]
fn square_period(freq: u16) -> u32 {
    // f = 131072/(2048-n); period in system clocks ≈ 16*(2048-n)
    // (system 16.78MHz / 131072 = 128; half-wave 8 steps → 16*(2048-n))
    let n = u32::from(freq & 0x7FF);
    16 * (2048 - n).max(1)
}

#[inline]
fn wave_period(freq: u16) -> u32 {
    // rate = 2097152/(2048-n); system clocks per sample ≈ 8*(2048-n)
    let n = u32::from(freq & 0x7FF);
    8 * (2048 - n).max(1)
}

#[inline]
fn noise_period(divisor_code: u8, shift: u8) -> u32 {
    let r = if divisor_code == 0 {
        8
    } else {
        u32::from(divisor_code) << 4
    };
    r << (shift + 1)
}
