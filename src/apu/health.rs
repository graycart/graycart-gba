//! APU health telemetry for console `--debug` summaries (GB-inspired).
//!
//! Cited: graycart-gb `ApuDebug` peak / sample counters
//!   https://github.com/graycart/graycart-gb (src/debug/machine.rs, src/apu)
//! Cited: GBATEK — FIFO / SOUNDCNT / SOUNDBIAS
//!   https://problemkaputt.de/gbatek.htm
//! Note: counters are always updated (cheap); printing is gated by `--debug`.

use super::pcm::PcmFrame;
use super::regs::ApuRegs;

/// |PCM| ≥ this → clip / extreme peak (near full-scale i16).
pub const CLIP_ABS: i16 = 30_000;
/// |mean PCM| above this → DC / stuck-latch risk (rumble / saw).
pub const DC_WARN_ABS: i32 = 6_000;
/// |PCM| above this (but below clip) → extreme peak warn band.
pub const EXTREME_ABS: i16 = 24_000;

/// Rolling APU health counters (lifetime + period snapshots via [`Self::snapshot_and_reset_period`]).
#[derive(Debug, Clone)]
pub struct ApuHealth {
    pub samples: u64,
    pub peak_min: i16,
    pub peak_max: i16,
    pub sum_l: i64,
    pub sum_r: i64,
    pub clip_hits: u64,
    pub extreme_hits: u64,
    /// Timer pops while FIFO empty **and** that FIFO is routed to an ear.
    pub empty_drain_a: u64,
    pub empty_drain_b: u64,
    /// Extra timer ticks while empty+routed after the first underrun (refill lag).
    pub refill_lag_a: u64,
    pub refill_lag_b: u64,
    pub dma_req_a: u64,
    pub dma_req_b: u64,
    /// Period-scoped copies (reset each debug period).
    pub period: ApuHealthPeriod,
    awaiting_refill_a: bool,
    awaiting_refill_b: bool,
}

/// Counters accumulated since the last debug period flush.
#[derive(Debug, Clone, Copy, Default)]
pub struct ApuHealthPeriod {
    pub samples: u64,
    pub peak_min: i16,
    pub peak_max: i16,
    pub sum_l: i64,
    pub sum_r: i64,
    pub clip_hits: u64,
    pub extreme_hits: u64,
    pub empty_drain_a: u64,
    pub empty_drain_b: u64,
    pub refill_lag_a: u64,
    pub refill_lag_b: u64,
    pub dma_req_a: u64,
    pub dma_req_b: u64,
    pub underrun_a: u64,
    pub underrun_b: u64,
    pub overrun_a: u64,
    pub overrun_b: u64,
}

impl Default for ApuHealth {
    fn default() -> Self {
        Self::new()
    }
}

impl ApuHealth {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            samples: 0,
            peak_min: i16::MAX,
            peak_max: i16::MIN,
            sum_l: 0,
            sum_r: 0,
            clip_hits: 0,
            extreme_hits: 0,
            empty_drain_a: 0,
            empty_drain_b: 0,
            refill_lag_a: 0,
            refill_lag_b: 0,
            dma_req_a: 0,
            dma_req_b: 0,
            period: ApuHealthPeriod {
                samples: 0,
                peak_min: i16::MAX,
                peak_max: i16::MIN,
                sum_l: 0,
                sum_r: 0,
                clip_hits: 0,
                extreme_hits: 0,
                empty_drain_a: 0,
                empty_drain_b: 0,
                refill_lag_a: 0,
                refill_lag_b: 0,
                dma_req_a: 0,
                dma_req_b: 0,
                underrun_a: 0,
                underrun_b: 0,
                overrun_a: 0,
                overrun_b: 0,
            },
            awaiting_refill_a: false,
            awaiting_refill_b: false,
        }
    }

    /// Record one mixed PCM output frame.
    pub fn on_pcm(&mut self, frame: PcmFrame) {
        self.samples = self.samples.saturating_add(1);
        self.period.samples = self.period.samples.saturating_add(1);
        self.note_peak(frame.left);
        self.note_peak(frame.right);
        self.sum_l = self.sum_l.saturating_add(i64::from(frame.left));
        self.sum_r = self.sum_r.saturating_add(i64::from(frame.right));
        self.period.sum_l = self.period.sum_l.saturating_add(i64::from(frame.left));
        self.period.sum_r = self.period.sum_r.saturating_add(i64::from(frame.right));
        for s in [frame.left, frame.right] {
            let a = s.unsigned_abs();
            if a >= CLIP_ABS as u16 {
                self.clip_hits = self.clip_hits.saturating_add(1);
                self.period.clip_hits = self.period.clip_hits.saturating_add(1);
            } else if a >= EXTREME_ABS as u16 {
                self.extreme_hits = self.extreme_hits.saturating_add(1);
                self.period.extreme_hits = self.period.extreme_hits.saturating_add(1);
            }
        }
    }

    fn note_peak(&mut self, s: i16) {
        self.peak_min = self.peak_min.min(s);
        self.peak_max = self.peak_max.max(s);
        self.period.peak_min = self.period.peak_min.min(s);
        self.period.peak_max = self.period.peak_max.max(s);
    }

    /// Timer overflow path for FIFO A: `was_empty` before pop; `routed` if L/R enable.
    pub fn on_fifo_timer_a(&mut self, was_empty: bool, routed: bool, needs_dma: bool) {
        if !routed {
            self.awaiting_refill_a = false;
            return;
        }
        if was_empty {
            self.empty_drain_a = self.empty_drain_a.saturating_add(1);
            self.period.empty_drain_a = self.period.empty_drain_a.saturating_add(1);
            if self.awaiting_refill_a {
                self.refill_lag_a = self.refill_lag_a.saturating_add(1);
                self.period.refill_lag_a = self.period.refill_lag_a.saturating_add(1);
            }
            self.awaiting_refill_a = true;
        } else if needs_dma {
            // Half-empty is normal; only lag once we have already underrun.
        } else {
            self.awaiting_refill_a = false;
        }
    }

    pub fn on_fifo_timer_b(&mut self, was_empty: bool, routed: bool, needs_dma: bool) {
        if !routed {
            self.awaiting_refill_b = false;
            return;
        }
        if was_empty {
            self.empty_drain_b = self.empty_drain_b.saturating_add(1);
            self.period.empty_drain_b = self.period.empty_drain_b.saturating_add(1);
            if self.awaiting_refill_b {
                self.refill_lag_b = self.refill_lag_b.saturating_add(1);
                self.period.refill_lag_b = self.period.refill_lag_b.saturating_add(1);
            }
            self.awaiting_refill_b = true;
        } else if !needs_dma {
            self.awaiting_refill_b = false;
        }
    }

    pub fn on_dma_req(&mut self, dma1: bool, dma2: bool) {
        if dma1 {
            self.dma_req_a = self.dma_req_a.saturating_add(1);
            self.period.dma_req_a = self.period.dma_req_a.saturating_add(1);
        }
        if dma2 {
            self.dma_req_b = self.dma_req_b.saturating_add(1);
            self.period.dma_req_b = self.period.dma_req_b.saturating_add(1);
        }
    }

    pub fn on_fifo_push_a(&mut self) {
        self.awaiting_refill_a = false;
    }

    pub fn on_fifo_push_b(&mut self) {
        self.awaiting_refill_b = false;
    }

    /// Snapshot and clear the period counters (FIFO underrun/overrun filled by caller).
    pub fn take_period(&mut self) -> ApuHealthPeriod {
        let out = self.period;
        self.period = ApuHealthPeriod {
            peak_min: i16::MAX,
            peak_max: i16::MIN,
            ..ApuHealthPeriod::default()
        };
        if out.samples == 0 {
            let mut empty = out;
            empty.peak_min = 0;
            empty.peak_max = 0;
            return empty;
        }
        out
    }

    #[must_use]
    pub fn mean_dc(&self) -> (i32, i32) {
        if self.samples == 0 {
            return (0, 0);
        }
        let n = self.samples as i64;
        ((self.sum_l / n) as i32, (self.sum_r / n) as i32)
    }
}

impl ApuHealthPeriod {
    #[must_use]
    pub fn mean_dc(self) -> (i32, i32) {
        if self.samples == 0 {
            return (0, 0);
        }
        let n = self.samples as i64;
        ((self.sum_l / n) as i32, (self.sum_r / n) as i32)
    }
}

/// Route / volume anomaly one-liner helpers (SOUNDCNT_*).
#[must_use]
pub fn fifo_route_label(regs: &ApuRegs, a: bool) -> String {
    let (l, r, full) = if a {
        (
            regs.fifo_a_left(),
            regs.fifo_a_right(),
            regs.fifo_a_full_volume(),
        )
    } else {
        (
            regs.fifo_b_left(),
            regs.fifo_b_right(),
            regs.fifo_b_full_volume(),
        )
    };
    let ears = match (l, r) {
        (true, true) => "L+R",
        (true, false) => "L",
        (false, true) => "R",
        (false, false) => "off",
    };
    let vol = if full { "100%" } else { "50%" };
    format!("{ears}/{vol}")
}
