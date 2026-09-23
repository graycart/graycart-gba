//! APU. Four PSG channels, two DMA FIFOs, and the GBA mixer.
//!
//! Cited: GBATEK Sound Controller.
//! <https://problemkaputt.de/gbatek.htm>
//!
//! Do not link the graycart-gb APU. This is not the Game Boy mixer.

mod fifo;
mod mix;
mod psg;

#[cfg(test)]
mod tests;

use std::collections::VecDeque;

use crate::timer::Timers;

pub use fifo::source_allowed;
pub use fifo::Fifo;
pub use psg::Psg;

use mix::{mix, MixIn};

/// One mixed stereo sample every this many CPU cycles (32768 Hz).
const CYCLES_PER_SAMPLE: u32 = 512;
/// Bound on retained PCM frames in the core (no host device).
const PCM_CAP: usize = 2048;

/// GBA APU: PSG, DMA FIFOs, mixer, and a short PCM ring.
pub struct Apu {
    pub psg: Psg,
    pub fifo_a: Fifo,
    pub fifo_b: Fifo,
    cnt_l: u16,
    cnt_h: u16,
    bias: u16,
    /// Most recently popped FIFO A sample (0 until the first pop).
    sample_a: i8,
    /// Most recently popped FIFO B sample (0 until the first pop).
    sample_b: i8,
    mix_cycles: u32,
    /// Last mixed stereo frames (left, right), newest at the back.
    pcm: VecDeque<(i16, i16)>,
    sample_count: u64,
    sum_left: i64,
    sum_right: i64,
    peak_min: i16,
    peak_max: i16,
    have_peak: bool,
    clip: u32,
    extreme: u32,
}

impl std::fmt::Debug for Apu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Apu")
            .field("cnt_l", &self.cnt_l)
            .field("cnt_h", &self.cnt_h)
            .field("bias", &self.bias)
            .field("pcm_len", &self.pcm.len())
            .field("sample_count", &self.sample_count)
            .field("clip", &self.clip)
            .finish_non_exhaustive()
    }
}

impl Apu {
    pub fn new() -> Self {
        Self {
            psg: Psg::new(),
            fifo_a: Fifo::new(),
            fifo_b: Fifo::new(),
            cnt_l: 0,
            cnt_h: 0,
            bias: 0,
            sample_a: 0,
            sample_b: 0,
            mix_cycles: 0,
            pcm: VecDeque::with_capacity(PCM_CAP),
            sample_count: 0,
            sum_left: 0,
            sum_right: 0,
            peak_min: 0,
            peak_max: 0,
            have_peak: false,
            clip: 0,
            extreme: 0,
        }
    }

    pub fn cnt_l(&self) -> u16 {
        self.cnt_l
    }

    pub fn cnt_h(&self) -> u16 {
        self.cnt_h
    }

    pub fn bias(&self) -> u16 {
        self.bias
    }

    pub fn set_cnt_l(&mut self, value: u16) {
        self.cnt_l = value;
    }

    pub fn set_cnt_h(&mut self, value: u16) {
        if value & (1 << 11) != 0 {
            self.fifo_a.reset();
        }
        if value & (1 << 15) != 0 {
            self.fifo_b.reset();
        }
        // Bits 11 / 15 are write-only FIFO resets.
        self.cnt_h = value & !((1 << 11) | (1 << 15));
    }

    pub fn set_bias(&mut self, value: u16) {
        self.bias = value;
    }

    /// Mixed PCM frames retained in the core (oldest first).
    pub fn pcm(&self) -> &VecDeque<(i16, i16)> {
        &self.pcm
    }

    /// Advance one CPU cycle: PSG step, optional FIFO pops, optional mix.
    ///
    /// Returns whether FIFO A and/or B latched a DMA refill request this cycle.
    pub fn tick(&mut self, timer_overflow: u8) -> (bool, bool) {
        let timer_a = fifo_timer_index(self.cnt_h, true);
        let timer_b = fifo_timer_index(self.cnt_h, false);

        if timer_overflow & (1 << timer_a) != 0 {
            self.sample_a = self.fifo_a.pop();
        }
        if timer_overflow & (1 << timer_b) != 0 {
            self.sample_b = self.fifo_b.pop();
        }

        let req_a = self.fifo_a.take_dma_request();
        let req_b = self.fifo_b.take_dma_request();

        let psg = self.psg.step(1);
        self.mix_cycles += 1;
        if self.mix_cycles >= CYCLES_PER_SAMPLE {
            self.mix_cycles -= CYCLES_PER_SAMPLE;
            let out = mix(&MixIn {
                psg,
                fifo_a: self.sample_a,
                fifo_b: self.sample_b,
                cnt_l: self.cnt_l,
                cnt_h: self.cnt_h,
                bias: self.bias,
            });
            self.push_pcm(out.left, out.right);
            if out.clip {
                self.clip = self.clip.saturating_add(1);
                self.extreme = self.extreme.saturating_add(1);
            }
        }

        (req_a, req_b)
    }

    fn push_pcm(&mut self, left: i16, right: i16) {
        if self.pcm.len() >= PCM_CAP {
            let _ = self.pcm.pop_front();
        }
        self.pcm.push_back((left, right));
        self.sample_count += 1;
        self.sum_left += i64::from(left);
        self.sum_right += i64::from(right);
        for s in [left, right] {
            if !self.have_peak {
                self.peak_min = s;
                self.peak_max = s;
                self.have_peak = true;
            } else {
                self.peak_min = self.peak_min.min(s);
                self.peak_max = self.peak_max.max(s);
            }
        }
    }

    /// Live `gba-debug: apu health` line (not the short-file stub).
    pub fn health_line(&self, frame: u32, timers: &Timers) -> String {
        let master = u8::from(self.psg.read_master() & 0x80 != 0);
        let psg_on = self.psg.read_master() & 0xF;
        let fifo_a = fifo_route(self.cnt_h, true);
        let fifo_b = fifo_route(self.cnt_h, false);
        let pwm = pwm_hz(self, timers);
        let (dc_l, dc_r) = self.dc_means();
        let (peak_lo, peak_hi) = if self.have_peak {
            (self.peak_min, self.peak_max)
        } else {
            (0, 0)
        };
        format!(
            "gba-debug: apu health frame={frame} master={master} pwm={pwm}Hz fifoA={fifo_a} underrun={}/{} overrun={}/{} empty={}/{} lag=0/0 dma_req={}/{} peak=[{peak_lo}..{peak_hi}] dc≈[{dc_l},{dc_r}] clip={} extreme={} psg_on=0x{psg_on:X} psg_nr50=0x{:02X} fifoB={fifo_b}",
            self.fifo_a.underrun(),
            self.fifo_b.underrun(),
            self.fifo_a.overrun(),
            self.fifo_b.overrun(),
            self.fifo_a.empty_drain(),
            self.fifo_b.empty_drain(),
            self.fifo_a.dma_requests(),
            self.fifo_b.dma_requests(),
            self.clip,
            self.extreme,
            self.cnt_l,
        )
    }

    /// APU row fields for the live AV report.
    pub fn av_line(&self, timers: &Timers) -> String {
        let master = u8::from(self.psg.read_master() & 0x80 != 0);
        let fifo_a = fifo_route(self.cnt_h, true);
        let fifo_b = fifo_route(self.cnt_h, false);
        let pwm = pwm_hz(self, timers);
        let (dc_l, dc_r) = self.dc_means();
        let (peak_lo, peak_hi) = if self.have_peak {
            (self.peak_min, self.peak_max)
        } else {
            (0, 0)
        };
        format!(
            "APU master={master} pwm={pwm}Hz fifoA={fifo_a} fifoB={fifo_b} underrun={}/{} overrun={}/{} empty_drain={}/{} peak=[{peak_lo}..{peak_hi}] dc≈[{dc_l},{dc_r}] clip={}",
            self.fifo_a.underrun(),
            self.fifo_b.underrun(),
            self.fifo_a.overrun(),
            self.fifo_b.overrun(),
            self.fifo_a.empty_drain(),
            self.fifo_b.empty_drain(),
            self.clip,
        )
    }

    fn dc_means(&self) -> (i64, i64) {
        if self.sample_count == 0 {
            (0, 0)
        } else {
            (
                self.sum_left / self.sample_count as i64,
                self.sum_right / self.sample_count as i64,
            )
        }
    }
}

impl Default for Apu {
    fn default() -> Self {
        Self::new()
    }
}

fn fifo_enabled(cnt_h: u16, is_a: bool) -> bool {
    if is_a {
        // SOUNDCNT_H bits 8–9: FIFO A right/left enable
        cnt_h & ((1 << 8) | (1 << 9)) != 0
    } else {
        // SOUNDCNT_H bits 12–13: FIFO B right/left enable
        cnt_h & ((1 << 12) | (1 << 13)) != 0
    }
}

fn fifo_timer_index(cnt_h: u16, is_a: bool) -> u32 {
    let bit = if is_a { 10 } else { 14 };
    u32::from(cnt_h & (1 << bit) != 0)
}

fn fifo_route(cnt_h: u16, is_a: bool) -> &'static str {
    if !fifo_enabled(cnt_h, is_a) {
        return "off";
    }
    match fifo_timer_index(cnt_h, is_a) {
        0 => "timer0",
        _ => "timer1",
    }
}

fn pwm_hz(apu: &Apu, timers: &Timers) -> u32 {
    let index = if fifo_enabled(apu.cnt_h, true) {
        fifo_timer_index(apu.cnt_h, true) as usize
    } else if fifo_enabled(apu.cnt_h, false) {
        fifo_timer_index(apu.cnt_h, false) as usize
    } else {
        return 0;
    };
    let control = timers.read16((index as u32) * 4 + 2);
    if control & (1 << 7) == 0 {
        return 0;
    }
    let prescale = [1u32, 64, 256, 1024][(control & 0b11) as usize];
    let reload = u32::from(timers.reload(index));
    let period = (65536u32 - reload).saturating_mul(prescale);
    16_777_216u32.checked_div(period).unwrap_or_default()
}
