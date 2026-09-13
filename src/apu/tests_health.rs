//! APU health counter gates (synthetic — no commercial ROMs).
//!
//! Cited: graycart-gb ApuDebug peak / sample counters (spirit)
//!   https://github.com/graycart/graycart-gb (src/debug/machine.rs)
//! Cited: GBATEK — FIFO / SOUNDCNT
//!   https://problemkaputt.de/gbatek.htm

use super::health::{ApuHealth, CLIP_ABS, DC_WARN_ABS, EXTREME_ABS};
use super::pcm::PcmFrame;
use super::regs::{MASTER_ENABLE, OFF_SOUNDCNT_H, OFF_SOUNDCNT_X};
use super::Apu;

#[test]
fn empty_routed_fifo_timer_counts_drain_and_lag() {
    let mut h = ApuHealth::new();
    h.on_fifo_timer_a(true, true, true); // first empty drain
    h.on_fifo_timer_a(true, true, true); // lag while still awaiting refill
    assert_eq!(h.empty_drain_a, 2);
    assert_eq!(h.period.empty_drain_a, 2);
    assert_eq!(h.refill_lag_a, 1);
    assert_eq!(h.period.refill_lag_a, 1);
    h.on_fifo_push_a();
    h.on_fifo_timer_a(false, true, false);
    // After push, next empty starts a new underrun streak (no lag yet).
    h.on_fifo_timer_a(true, true, true);
    assert_eq!(h.empty_drain_a, 3);
    assert_eq!(h.refill_lag_a, 1);
}

#[test]
fn unrouted_fifo_timer_skips_empty_drain() {
    let mut h = ApuHealth::new();
    h.on_fifo_timer_a(true, false, true);
    assert_eq!(h.empty_drain_a, 0);
    assert_eq!(h.period.empty_drain_a, 0);
}

#[test]
fn pcm_peaks_clip_and_extreme_counters() {
    let mut h = ApuHealth::new();
    h.on_pcm(PcmFrame {
        left: CLIP_ABS,
        right: EXTREME_ABS,
    });
    assert_eq!(h.samples, 1);
    assert_eq!(h.clip_hits, 1);
    assert_eq!(h.extreme_hits, 1);
    assert_eq!(h.peak_max, CLIP_ABS);
    assert_eq!(h.period.clip_hits, 1);
    assert_eq!(h.period.extreme_hits, 1);
}

#[test]
fn mean_dc_flags_stuck_latch_band() {
    let mut h = ApuHealth::new();
    for _ in 0..8 {
        h.on_pcm(PcmFrame {
            left: DC_WARN_ABS as i16,
            right: -(DC_WARN_ABS as i16),
        });
    }
    let (l, r) = h.mean_dc();
    assert!(l.unsigned_abs() >= DC_WARN_ABS as u32);
    assert!(r.unsigned_abs() >= DC_WARN_ABS as u32);
}

#[test]
fn take_period_resets_and_zero_samples_clears_peaks() {
    let mut h = ApuHealth::new();
    h.on_pcm(PcmFrame {
        left: 100,
        right: -50,
    });
    let p = h.take_period();
    assert_eq!(p.samples, 1);
    assert_eq!(p.peak_max, 100);
    let empty = h.take_period();
    assert_eq!(empty.samples, 0);
    assert_eq!(empty.peak_min, 0);
    assert_eq!(empty.peak_max, 0);
}

#[test]
fn starved_fifo_a_through_apu_increments_empty_drain() {
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE);
    apu.write16(OFF_SOUNDCNT_H, 0x0300); // A → L+R, TM0
    for _ in 0..80 {
        apu.on_timer_overflows(1, 0);
        apu.step(256);
    }
    assert!(
        apu.health.empty_drain_a >= 64,
        "expected empty_drain_a storm, got {}",
        apu.health.empty_drain_a
    );
    assert!(
        apu.health.refill_lag_a > 0,
        "expected refill lag while starved"
    );
}
