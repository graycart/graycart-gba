//! Tiny adaptive rate correction for host/emulator clock drift.
//!
//! The APU emits PCM on the emulated DMG clock. The host device clock drifts
//! relative to that. A PI controller on ring occupancy keeps the queue near
//! target. Stretch (queue low) may go further than compress so a mild emu
//! shortfall plus wireless period jitter can still refill the ring.

use graycart::StereoSample;

/// Proportional gain from normalized queue error → rate correction.
const KP: f64 = 0.8;
/// Integral gain (normalized error · seconds ≈ per-frame at 60 Hz).
const KI: f64 = 0.05;
/// Max stretch when queue is low (step down to `1 - this`): +1.2% more outs.
pub const MAX_STRETCH: f64 = 0.012;
/// Max compress when queue is high (step up by this): −0.8% outs.
const MAX_COMPRESS: f64 = 0.008;
/// Ignore tiny occupancy noise near the target.
const DEADBAND: f64 = 0.02;
/// Clamp on integrated error (normalized).
const MAX_INTEGRAL: f64 = 0.08;

/// True when `step` is at (or past) the max-stretch floor.
pub fn step_pegged_low(step: f64) -> bool {
    step <= 1.0 - MAX_STRETCH + 1e-9
}

#[derive(Debug)]
pub struct AdaptiveResampler {
    /// Fractional read cursor into the current chunk (after prepending `held`).
    phase: f64,
    held: Option<StereoSample>,
    /// Accumulated occupancy error for the I term.
    integral: f64,
    /// Last applied input step (1.0 ± correction); for diagnostics.
    pub last_step: f64,
}

impl Default for AdaptiveResampler {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptiveResampler {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            held: None,
            integral: 0.0,
            last_step: 1.0,
        }
    }

    /// Map queue occupancy to an input advancement step.
    ///
    /// Queue high → step > 1 (consume input faster → fewer outs).
    /// Queue low  → step < 1 (consume slower → more outs).
    /// Stretch authority is wider than compress so underrun-biased hosts recover.
    pub fn step_for_queue(&mut self, queued_frames: usize, target_frames: usize) -> f64 {
        let target = target_frames.max(1) as f64;
        let mut err = (queued_frames as f64 - target) / target;
        if err.abs() < DEADBAND {
            // Leak integral toward zero when inside the band so we don't wind up.
            self.integral *= 0.9;
            err = 0.0;
        } else {
            self.integral = (self.integral + err).clamp(-MAX_INTEGRAL, MAX_INTEGRAL);
        }
        let raw = err * KP + self.integral * KI;
        // Negative correction = stretch (more outs); positive = compress.
        let correction = raw.clamp(-MAX_STRETCH, MAX_COMPRESS);
        1.0 + correction
    }

    /// Stateless view for tests (does not update integral).
    #[cfg(test)]
    pub fn step_for_queue_stateless(queued_frames: usize, target_frames: usize) -> f64 {
        let target = target_frames.max(1) as f64;
        let mut err = (queued_frames as f64 - target) / target;
        if err.abs() < DEADBAND {
            err = 0.0;
        }
        let correction = (err * KP).clamp(-MAX_STRETCH, MAX_COMPRESS);
        1.0 + correction
    }

    /// Resample `input` toward centering `queued_frames` on `target_frames`.
    pub fn process(
        &mut self,
        input: &[StereoSample],
        queued_frames: usize,
        target_frames: usize,
    ) -> Vec<StereoSample> {
        if input.is_empty() && self.held.is_none() {
            return Vec::new();
        }

        let step = self.step_for_queue(queued_frames, target_frames);
        self.last_step = step;

        let mut buf = Vec::with_capacity(input.len() + 1);
        if let Some(h) = self.held.take() {
            buf.push(h);
        }
        buf.extend_from_slice(input);
        if buf.is_empty() {
            return Vec::new();
        }

        let mut out = Vec::with_capacity(buf.len() + buf.len() / 200 + 4);

        // Need two points to interpolate; with one sample just emit/hold.
        if buf.len() == 1 {
            out.push(buf[0]);
            self.held = Some(buf[0]);
            self.phase = 0.0;
            return out;
        }

        while (self.phase as usize) + 1 < buf.len() {
            let i = self.phase as usize;
            let t = (self.phase - i as f64) as f32;
            let a = buf[i];
            let b = buf[i + 1];
            out.push(StereoSample {
                left: a.left + (b.left - a.left) * t,
                right: a.right + (b.right - a.right) * t,
            });
            self.phase += step;
        }

        let hold_idx = (self.phase as usize).min(buf.len() - 1);
        self.held = Some(buf[hold_idx]);
        self.phase -= hold_idx as f64;
        if self.phase < 0.0 {
            self.phase = 0.0;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize) -> Vec<StereoSample> {
        (0..n)
            .map(|i| {
                let v = (i as f32) * 0.001;
                StereoSample { left: v, right: v }
            })
            .collect()
    }

    #[test]
    fn low_queue_emits_more_than_input() {
        let mut r = AdaptiveResampler::new();
        let input = tone(1000);
        let out = r.process(&input, 100, 1000);
        assert!(
            out.len() > input.len(),
            "out={} in={}",
            out.len(),
            input.len()
        );
        assert!(r.last_step < 1.0);
    }

    #[test]
    fn high_queue_emits_fewer_than_input() {
        let mut r = AdaptiveResampler::new();
        let input = tone(1000);
        let out = r.process(&input, 2000, 1000);
        assert!(
            out.len() < input.len(),
            "out={} in={}",
            out.len(),
            input.len()
        );
        assert!(r.last_step > 1.0);
    }

    #[test]
    fn correction_is_clamped_asymmetric() {
        let mut r = AdaptiveResampler::new();
        let lo = r.step_for_queue(0, 1000);
        let hi = r.step_for_queue(10_000, 1000);
        assert!((1.0 - lo) <= MAX_STRETCH + 1e-9);
        assert!((hi - 1.0) <= MAX_COMPRESS + 1e-9);
        assert!(
            1.0 - lo > MAX_COMPRESS,
            "stretch should exceed compress authority"
        );
        assert!(step_pegged_low(lo));
        assert!(!step_pegged_low(1.0));
    }

    #[test]
    fn near_target_is_near_unity() {
        let step = AdaptiveResampler::step_for_queue_stateless(1000, 1000);
        assert!((step - 1.0).abs() < 1e-9);
    }

    #[test]
    fn sustained_high_queue_integrates_stronger_correction() {
        let mut r = AdaptiveResampler::new();
        let first = r.step_for_queue(2800, 2400);
        for _ in 0..30 {
            let _ = r.step_for_queue(2800, 2400);
        }
        let later = r.step_for_queue(2800, 2400);
        assert!(later >= first);
        assert!(later > 1.0);
    }
}
