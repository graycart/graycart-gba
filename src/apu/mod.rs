//! Audio Processing Unit — PSG + FIFO + mixer + host PCM (P6).
//!
//! Module layout from graycart-gba implementation plan §2.2 / `04-apu.md` §12.
//! Cited: GBATEK — GBA Sound Channel Overview / FIFO / SOUNDBIAS
//!   https://problemkaputt.de/gbatek.htm
//! Cited: jsgroth — GBA audio (secondary mixing / FIFO notes)
//!   https://jsgroth.dev/blog/posts/gba-audio/
//! Research: Project store `docs/graycart-gba/04-apu.md`
//! Note: P5 FIFO DMA request latch retained for DMA1/2 Special coupling.

mod fifo;
mod health;
mod mixer;
mod pcm;
mod psg;
mod regs;

#[cfg(test)]
mod tests_fifo;
#[cfg(test)]
mod tests_mixer;
#[cfg(test)]
mod tests_pcm;
#[cfg(test)]
mod tests_psg;
#[cfg(test)]
mod tests_regs;

pub use fifo::{Fifo, FifoPair, FIFO_CAPACITY, FIFO_HALF};
pub use health::{
    fifo_route_label, ApuHealth, ApuHealthPeriod, CLIP_ABS, DC_WARN_ABS, EXTREME_ABS,
};
pub use mixer::{apply_pwm_truncate, mix, pwm_period_cycles, pwm_rate_hz, to_i16_pcm, MixedSample};
pub use pcm::{encode_wav_s16le, soft_rms, PcmBuffer, PcmFrame, PCM_CAPACITY};
pub use psg::Psg;
pub use regs::{
    ApuRegs, RegWriteEffect, MASTER_ENABLE, OFF_FIFO_A, OFF_FIFO_B, OFF_SOUNDBIAS, OFF_SOUNDCNT_H,
    OFF_SOUNDCNT_L, OFF_SOUNDCNT_X, OFF_WAVE_RAM, SOUNDBIAS_DEFAULT,
};

/// GBA APU: registers, PSG, FIFOs, mixer, PCM ring.
#[derive(Debug)]
pub struct Apu {
    pub regs: ApuRegs,
    pub psg: Psg,
    pub fifos: FifoPair,
    pub pcm: PcmBuffer,
    /// Bit0 → DMA1 FIFO request, bit1 → DMA2. Cleared when consumed by DMA glue.
    pub fifo_dma_request: u8,
    /// Always-on health counters for `--debug` AV summaries.
    pub health: ApuHealth,
    /// Cycles accumulated toward next PWM output sample.
    pwm_accum: u32,
}

impl Default for Apu {
    fn default() -> Self {
        Self::new()
    }
}

impl Apu {
    #[must_use]
    pub fn new() -> Self {
        Self {
            regs: ApuRegs::new(),
            psg: Psg::new(),
            fifos: FifoPair::new(),
            pcm: PcmBuffer::new(),
            fifo_dma_request: 0,
            health: ApuHealth::new(),
            pwm_accum: 0,
        }
    }

    /// Raise a FIFO refill request (tests / timer sampling).
    pub fn request_fifo_dma(&mut self, dma1: bool, dma2: bool) {
        if dma1 {
            self.fifo_dma_request |= 1;
        }
        if dma2 {
            self.fifo_dma_request |= 2;
        }
    }

    /// Take and clear pending FIFO DMA request bits.
    pub fn take_fifo_dma_request(&mut self) -> u8 {
        let v = self.fifo_dma_request;
        self.fifo_dma_request = 0;
        v
    }

    /// MMIO halfword read (`off` relative to `0x04000000`).
    #[must_use]
    pub fn read16(&self, off: usize) -> u16 {
        self.regs.read16(off)
    }

    /// MMIO halfword write.
    pub fn write16(&mut self, off: usize, value: u16) {
        let effect = self.regs.write16(off, value);
        self.apply_effect(effect);
    }

    /// MMIO 32-bit write (FIFO preferred).
    pub fn write32(&mut self, off: usize, value: u32) {
        let effect = self.regs.write_fifo32(off, value);
        self.apply_effect(effect);
    }

    fn apply_effect(&mut self, effect: RegWriteEffect) {
        if effect.master_disabled {
            self.psg.reset_all();
        }
        if effect.reset_fifo_a {
            self.fifos.reset_a();
            self.health.on_fifo_push_a();
        }
        if effect.reset_fifo_b {
            self.fifos.reset_b();
            self.health.on_fifo_push_b();
        }
        if let Some(w) = effect.fifo_a_word {
            self.fifos.a.push_word(w);
            self.health.on_fifo_push_a();
        }
        if let Some(w) = effect.fifo_b_word {
            self.fifos.b.push_word(w);
            self.health.on_fifo_push_b();
        }
        if effect.trigger_ch1 {
            self.psg.trigger_ch1(&self.regs);
        }
        if effect.trigger_ch2 {
            self.psg.trigger_ch2(&self.regs);
        }
        if effect.trigger_ch3 {
            self.psg.trigger_ch3(&self.regs);
        }
        if effect.trigger_ch4 {
            self.psg.trigger_ch4(&self.regs);
        }
    }

    /// True if `off` is in the sound MMIO window we own.
    #[inline]
    #[must_use]
    pub fn owns_offset(off: usize) -> bool {
        (0x60..=0xA6).contains(&off)
    }

    /// TM0/TM1 overflow counts → FIFO sample clock + DMA request bits.
    pub fn on_timer_overflows(&mut self, tm0: u64, tm1: u64) {
        if tm0 == 0 && tm1 == 0 {
            return;
        }
        let a_tm1 = self.regs.fifo_a_timer1();
        let b_tm1 = self.regs.fifo_b_timer1();
        let mut dma1 = false;
        let mut dma2 = false;

        let fire_a = if a_tm1 { tm1 } else { tm0 };
        let fire_b = if b_tm1 { tm1 } else { tm0 };

        let routed_a = self.regs.fifo_a_left() || self.regs.fifo_a_right();
        let routed_b = self.regs.fifo_b_left() || self.regs.fifo_b_right();
        for _ in 0..fire_a {
            let was_empty = self.fifos.a.is_empty();
            let needs = self.fifos.on_timer_a();
            self.health
                .on_fifo_timer_a(was_empty, routed_a, self.fifos.a.needs_dma());
            if needs {
                dma1 = true;
            }
        }
        for _ in 0..fire_b {
            let was_empty = self.fifos.b.is_empty();
            let needs = self.fifos.on_timer_b();
            self.health
                .on_fifo_timer_b(was_empty, routed_b, self.fifos.b.needs_dma());
            if needs {
                dma2 = true;
            }
        }
        if dma1 || dma2 {
            self.health.on_dma_req(dma1, dma2);
        }
        self.request_fifo_dma(dma1, dma2);
    }

    /// Advance PSG + emit PCM at PWM rate for `cycles` system clocks.
    pub fn step(&mut self, cycles: u64) {
        if cycles == 0 {
            return;
        }
        self.psg.step(cycles, &mut self.regs);
        let period = pwm_period_cycles(self.regs.pwm_resolution());
        let mut left = cycles;
        while left > 0 {
            let need = u64::from(period.saturating_sub(self.pwm_accum));
            if need == 0 {
                self.emit_pcm_frame();
                self.pwm_accum = 0;
                continue;
            }
            let take = left.min(need);
            self.pwm_accum += take as u32;
            left -= take;
            if self.pwm_accum >= period {
                self.emit_pcm_frame();
                self.pwm_accum = 0;
            }
        }
    }

    fn emit_pcm_frame(&mut self) {
        let mixed = mix(&self.regs, &self.psg, &self.fifos);
        let bias = self.regs.bias_level();
        let frame = PcmFrame::from_mixed(mixed, bias);
        self.health.on_pcm(frame);
        self.pcm.push(frame);
    }

    /// Pull host PCM frames (consumes ring).
    pub fn pull_samples(&mut self, out: &mut [PcmFrame]) -> usize {
        self.pcm.pull(out)
    }

    /// Current mix snapshot (does not advance).
    #[must_use]
    pub fn mix_now(&self) -> MixedSample {
        mix(&self.regs, &self.psg, &self.fifos)
    }

    /// Encode drained ring (non-consuming snapshot) as WAV at current PWM rate.
    #[must_use]
    pub fn soft_wav_bytes(&self) -> Vec<u8> {
        let frames = self.pcm.snapshot();
        let rate = pwm_rate_hz(self.regs.pwm_resolution());
        encode_wav_s16le(&frames, rate)
    }
}
