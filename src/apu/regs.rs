//! APU register file — SOUNDCNT_* / SOUNDBIAS / Wave RAM / FIFO ports (P6).
//!
//! Cited: GBATEK — GBA Sound Channel Overview / Sound Control Registers
//!   https://problemkaputt.de/gbatek.htm
//! Cited: gbadoc — Sound registers
//!   https://gbadev.net/gbadoc/audio/registers.html
//! Research: Project store `docs/graycart-gba/04-apu.md` §2–§9
//! Note: master enable clear forces PSG regs `060`–`081` to 0; H + bias stay.

/// I/O offsets relative to `0x04000000`.
pub const OFF_SOUND1CNT_L: usize = 0x60;
pub const OFF_SOUND1CNT_H: usize = 0x62;
pub const OFF_SOUND1CNT_X: usize = 0x64;
pub const OFF_SOUND2CNT_L: usize = 0x68;
pub const OFF_SOUND2CNT_H: usize = 0x6C;
pub const OFF_SOUND3CNT_L: usize = 0x70;
pub const OFF_SOUND3CNT_H: usize = 0x72;
pub const OFF_SOUND3CNT_X: usize = 0x74;
pub const OFF_SOUND4CNT_L: usize = 0x78;
pub const OFF_SOUND4CNT_H: usize = 0x7C;
pub const OFF_SOUNDCNT_L: usize = 0x80;
pub const OFF_SOUNDCNT_H: usize = 0x82;
pub const OFF_SOUNDCNT_X: usize = 0x84;
pub const OFF_SOUNDBIAS: usize = 0x88;
pub const OFF_WAVE_RAM: usize = 0x90;
pub const OFF_FIFO_A: usize = 0xA0;
pub const OFF_FIFO_B: usize = 0xA4;

/// Default SOUNDBIAS (center of unsigned 10-bit).
pub const SOUNDBIAS_DEFAULT: u16 = 0x0200;

/// SOUNDCNT_X master enable (bit 7).
pub const MASTER_ENABLE: u16 = 1 << 7;

/// Writable mask for SOUNDCNT_X (only bit 7).
pub const SOUNDCNT_X_WRITABLE: u16 = MASTER_ENABLE;

/// Register file for sound MMIO `0x60`–`0xA6` (gaps return 0).
#[derive(Debug, Clone)]
pub struct ApuRegs {
    pub sound1cnt_l: u16,
    pub sound1cnt_h: u16,
    pub sound1cnt_x: u16,
    pub sound2cnt_l: u16,
    pub sound2cnt_h: u16,
    pub sound3cnt_l: u16,
    pub sound3cnt_h: u16,
    pub sound3cnt_x: u16,
    pub sound4cnt_l: u16,
    pub sound4cnt_h: u16,
    pub soundcnt_l: u16,
    pub soundcnt_h: u16,
    /// Bit7 = master; bits 0–3 = channel ON (read-only hardware flags).
    pub soundcnt_x: u16,
    pub soundbias: u16,
    /// Dual Wave RAM banks (16 bytes each = 32 nibbles).
    pub wave_ram: [[u8; 16]; 2],
    /// Latched channel-on flags (bits 0–3 of SOUNDCNT_X).
    pub channel_on: u8,
}

impl Default for ApuRegs {
    fn default() -> Self {
        Self::new()
    }
}

impl ApuRegs {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sound1cnt_l: 0,
            sound1cnt_h: 0,
            sound1cnt_x: 0,
            sound2cnt_l: 0,
            sound2cnt_h: 0,
            sound3cnt_l: 0,
            sound3cnt_h: 0,
            sound3cnt_x: 0,
            sound4cnt_l: 0,
            sound4cnt_h: 0,
            soundcnt_l: 0,
            soundcnt_h: 0,
            soundcnt_x: 0,
            soundbias: SOUNDBIAS_DEFAULT,
            wave_ram: [[0; 16]; 2],
            channel_on: 0,
        }
    }

    #[inline]
    #[must_use]
    pub fn master_enabled(&self) -> bool {
        self.soundcnt_x & MASTER_ENABLE != 0
    }

    /// Clear PSG + SOUNDCNT_L when master enable goes 1→0 (GBATEK).
    pub fn clear_psg_on_master_off(&mut self) {
        self.sound1cnt_l = 0;
        self.sound1cnt_h = 0;
        self.sound1cnt_x = 0;
        self.sound2cnt_l = 0;
        self.sound2cnt_h = 0;
        self.sound3cnt_l = 0;
        self.sound3cnt_h = 0;
        self.sound3cnt_x = 0;
        self.sound4cnt_l = 0;
        self.sound4cnt_h = 0;
        self.soundcnt_l = 0;
        self.channel_on = 0;
        // Keep soundcnt_h / soundbias / wave_ram / FIFO (FIFO owned elsewhere).
        self.soundcnt_x &= MASTER_ENABLE; // already 0 when calling after clear
        self.soundcnt_x = 0;
    }

    /// Which Wave RAM bank the CPU R/W hits (opposite of play bank).
    #[inline]
    #[must_use]
    pub fn wave_cpu_bank(&self) -> usize {
        let play = (self.sound3cnt_l >> 6) & 1;
        (1 - play) as usize
    }

    #[inline]
    #[must_use]
    pub fn wave_play_bank(&self) -> usize {
        ((self.sound3cnt_l >> 6) & 1) as usize
    }

    #[inline]
    #[must_use]
    pub fn wave_dimension_64(&self) -> bool {
        self.sound3cnt_l & (1 << 5) != 0
    }

    /// Read halfword at I/O offset; gaps / FIFO reads → 0.
    #[must_use]
    pub fn read16(&self, off: usize) -> u16 {
        match off {
            OFF_SOUND1CNT_L => self.sound1cnt_l,
            OFF_SOUND1CNT_H => self.sound1cnt_h,
            OFF_SOUND1CNT_X => self.sound1cnt_x & 0x4000, // only length-flag readable
            OFF_SOUND2CNT_L => self.sound2cnt_l,
            OFF_SOUND2CNT_H => self.sound2cnt_h & 0x4000,
            OFF_SOUND3CNT_L => self.sound3cnt_l,
            OFF_SOUND3CNT_H => self.sound3cnt_h,
            OFF_SOUND3CNT_X => self.sound3cnt_x & 0x4000,
            OFF_SOUND4CNT_L => self.sound4cnt_l,
            OFF_SOUND4CNT_H => self.sound4cnt_h & 0x40FF,
            OFF_SOUNDCNT_L => self.soundcnt_l,
            OFF_SOUNDCNT_H => self.soundcnt_h,
            OFF_SOUNDCNT_X => (self.soundcnt_x & MASTER_ENABLE) | u16::from(self.channel_on & 0x0F),
            OFF_SOUNDBIAS => self.soundbias,
            o if (OFF_WAVE_RAM..OFF_WAVE_RAM + 0x10).contains(&o) && (o & 1) == 0 => {
                let bank = self.wave_cpu_bank();
                let i = o - OFF_WAVE_RAM;
                let lo = u16::from(self.wave_ram[bank][i]);
                let hi = u16::from(self.wave_ram[bank][i + 1]);
                lo | (hi << 8)
            }
            // FIFO ports: reads undefined → 0
            OFF_FIFO_A | OFF_FIFO_B | 0xA2 | 0xA6 => 0,
            _ => 0,
        }
    }

    /// Write halfword. Returns whether FIFO A/B was targeted (caller pushes samples).
    ///
    /// Also returns trigger events for PSG restart bits.
    pub fn write16(&mut self, off: usize, value: u16) -> RegWriteEffect {
        let mut effect = RegWriteEffect::default();
        match off {
            OFF_SOUND1CNT_L => {
                if self.master_enabled() {
                    self.sound1cnt_l = value & 0x007F;
                }
            }
            OFF_SOUND1CNT_H => {
                if self.master_enabled() {
                    self.sound1cnt_h = value;
                }
            }
            OFF_SOUND1CNT_X => {
                if self.master_enabled() {
                    self.sound1cnt_x = value & 0xC7FF;
                    if value & 0x8000 != 0 {
                        effect.trigger_ch1 = true;
                    }
                }
            }
            OFF_SOUND2CNT_L => {
                if self.master_enabled() {
                    self.sound2cnt_l = value;
                }
            }
            OFF_SOUND2CNT_H => {
                if self.master_enabled() {
                    self.sound2cnt_h = value & 0xC7FF;
                    if value & 0x8000 != 0 {
                        effect.trigger_ch2 = true;
                    }
                }
            }
            OFF_SOUND3CNT_L => {
                if self.master_enabled() {
                    self.sound3cnt_l = value & 0x00E0;
                }
            }
            OFF_SOUND3CNT_H => {
                if self.master_enabled() {
                    self.sound3cnt_h = value;
                }
            }
            OFF_SOUND3CNT_X => {
                if self.master_enabled() {
                    self.sound3cnt_x = value & 0xC7FF;
                    if value & 0x8000 != 0 {
                        effect.trigger_ch3 = true;
                    }
                }
            }
            OFF_SOUND4CNT_L => {
                if self.master_enabled() {
                    self.sound4cnt_l = value;
                }
            }
            OFF_SOUND4CNT_H => {
                if self.master_enabled() {
                    self.sound4cnt_h = value & 0xC0FF;
                    if value & 0x8000 != 0 {
                        effect.trigger_ch4 = true;
                    }
                }
            }
            OFF_SOUNDCNT_L => {
                if self.master_enabled() {
                    self.soundcnt_l = value & 0xFF77;
                }
            }
            OFF_SOUNDCNT_H => {
                // Remains R/W when master off (GBATEK); reset bits are write-1 clear.
                if value & (1 << 11) != 0 {
                    effect.reset_fifo_a = true;
                }
                if value & (1 << 15) != 0 {
                    effect.reset_fifo_b = true;
                }
                // Sticky: PSG ratio, DMA vol, route, timer select (no reset bits).
                self.soundcnt_h = value & 0x770F;
            }
            OFF_SOUNDCNT_X => {
                let want = value & SOUNDCNT_X_WRITABLE;
                let was = self.master_enabled();
                let now = want & MASTER_ENABLE != 0;
                if was && !now {
                    self.clear_psg_on_master_off();
                    effect.master_disabled = true;
                }
                self.soundcnt_x = want;
                if !now {
                    self.channel_on = 0;
                }
            }
            OFF_SOUNDBIAS => {
                // Bit 0 unused / forced even; bits 1–9 bias; 14–15 PWM.
                self.soundbias = value & 0xC3FE;
            }
            o if (OFF_WAVE_RAM..OFF_WAVE_RAM + 0x10).contains(&o) && (o & 1) == 0 => {
                let bank = self.wave_cpu_bank();
                let i = o - OFF_WAVE_RAM;
                self.wave_ram[bank][i] = value as u8;
                self.wave_ram[bank][i + 1] = (value >> 8) as u8;
            }
            OFF_FIFO_A => {
                effect.fifo_a_word = Some(u32::from(value));
            }
            0xA2 => {
                // High half of FIFO_A word — rare; treat as high 16 of a push.
                effect.fifo_a_word = Some(u32::from(value) << 16);
            }
            OFF_FIFO_B => {
                effect.fifo_b_word = Some(u32::from(value));
            }
            0xA6 => {
                effect.fifo_b_word = Some(u32::from(value) << 16);
            }
            _ => {}
        }
        effect
    }

    /// 32-bit FIFO write (preferred path).
    pub fn write_fifo32(&mut self, off: usize, value: u32) -> RegWriteEffect {
        let mut effect = RegWriteEffect::default();
        match off {
            OFF_FIFO_A => effect.fifo_a_word = Some(value),
            OFF_FIFO_B => effect.fifo_b_word = Some(value),
            _ => {
                // Split into halfwords for non-FIFO.
                let lo = self.write16(off, value as u16);
                let hi = self.write16(off + 2, (value >> 16) as u16);
                effect.merge(lo);
                effect.merge(hi);
            }
        }
        effect
    }

    /// PSG volume ratio from SOUNDCNT_H[1:0]; prohibited 3 → like 0 (25%).
    #[inline]
    #[must_use]
    pub fn psg_ratio_code(&self) -> u16 {
        let c = self.soundcnt_h & 0b11;
        if c == 3 {
            0
        } else {
            c
        }
    }

    #[inline]
    #[must_use]
    pub fn fifo_a_full_volume(&self) -> bool {
        self.soundcnt_h & (1 << 2) != 0
    }

    #[inline]
    #[must_use]
    pub fn fifo_b_full_volume(&self) -> bool {
        self.soundcnt_h & (1 << 3) != 0
    }

    #[inline]
    #[must_use]
    pub fn fifo_a_timer1(&self) -> bool {
        self.soundcnt_h & (1 << 10) != 0
    }

    #[inline]
    #[must_use]
    pub fn fifo_b_timer1(&self) -> bool {
        self.soundcnt_h & (1 << 14) != 0
    }

    #[inline]
    #[must_use]
    pub fn fifo_a_right(&self) -> bool {
        self.soundcnt_h & (1 << 8) != 0
    }
    #[inline]
    #[must_use]
    pub fn fifo_a_left(&self) -> bool {
        self.soundcnt_h & (1 << 9) != 0
    }
    #[inline]
    #[must_use]
    pub fn fifo_b_right(&self) -> bool {
        self.soundcnt_h & (1 << 12) != 0
    }
    #[inline]
    #[must_use]
    pub fn fifo_b_left(&self) -> bool {
        self.soundcnt_h & (1 << 13) != 0
    }

    /// Bias field bits 1–9 (register often `0x200`).
    #[inline]
    #[must_use]
    pub fn bias_level(&self) -> i32 {
        i32::from(self.soundbias & 0x3FE)
    }

    /// PWM resolution code bits 14–15.
    #[inline]
    #[must_use]
    pub fn pwm_resolution(&self) -> u16 {
        (self.soundbias >> 14) & 0b11
    }
}

/// Side effects from a register write.
#[derive(Debug, Default, Clone, Copy)]
pub struct RegWriteEffect {
    pub trigger_ch1: bool,
    pub trigger_ch2: bool,
    pub trigger_ch3: bool,
    pub trigger_ch4: bool,
    pub reset_fifo_a: bool,
    pub reset_fifo_b: bool,
    pub master_disabled: bool,
    pub fifo_a_word: Option<u32>,
    pub fifo_b_word: Option<u32>,
}

impl RegWriteEffect {
    pub fn merge(&mut self, other: Self) {
        self.trigger_ch1 |= other.trigger_ch1;
        self.trigger_ch2 |= other.trigger_ch2;
        self.trigger_ch3 |= other.trigger_ch3;
        self.trigger_ch4 |= other.trigger_ch4;
        self.reset_fifo_a |= other.reset_fifo_a;
        self.reset_fifo_b |= other.reset_fifo_b;
        self.master_disabled |= other.master_disabled;
        if other.fifo_a_word.is_some() {
            self.fifo_a_word = other.fifo_a_word;
        }
        if other.fifo_b_word.is_some() {
            self.fifo_b_word = other.fifo_b_word;
        }
    }
}
