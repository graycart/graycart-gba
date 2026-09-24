//! Wall-clock frame pacing for the display frontend.
//!
//! Hardware timing lives in the PPU; this module only decides how fast
//! completed frames are shown to the user.

use std::thread;
use std::time::{Duration, Instant};

use super::playback::SpeedPreset;

/// DMG CPU clock (Hz).
pub const GB_CPU_HZ: u64 = 4_194_304;
/// T-cycles per full frame (154 lines × 456).
pub const T_CYCLES_PER_FRAME: u64 = 70_224;

/// Nominal DMG frame period (~16.74 ms ≈ 59.7275 Hz).
pub const FRAME_DURATION: Duration =
    Duration::from_nanos(T_CYCLES_PER_FRAME * 1_000_000_000 / GB_CPU_HZ);

/// Target refresh rate derived from the same constants (~59.7275).
pub fn target_fps() -> f64 {
    GB_CPU_HZ as f64 / T_CYCLES_PER_FRAME as f64
}

/// Sleeps after each presented frame so wall-clock ≈ DMG cadence.
#[derive(Debug)]
pub struct FramePacer {
    #[allow(dead_code)] // used by [`after_present_ex`] (pace unit tests / legacy path)
    enabled: bool,
    #[allow(dead_code)]
    next_deadline: Option<Instant>,
    /// Duration of the most recent sleep (zero if skipped / disabled).
    pub last_sleep: Duration,
}

impl FramePacer {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            next_deadline: None,
            last_sleep: Duration::ZERO,
        }
    }

    /// Call once after presenting a frame. No-op when unthrottled.
    ///
    /// When `skip_sleep` is true (audio reservoir low), advance the deadline
    /// without sleeping so emulation catches up and refills the queue.
    #[allow(dead_code)] // retained for pace unit tests; HostScheduler owns runtime pacing
    pub fn after_present_ex(&mut self, skip_sleep: bool) {
        if !self.enabled {
            self.last_sleep = Duration::ZERO;
            return;
        }

        let now = Instant::now();
        let Some(deadline) = self.next_deadline else {
            self.next_deadline = Some(now + FRAME_DURATION);
            self.last_sleep = Duration::ZERO;
            return;
        };

        if !skip_sleep && now < deadline {
            let sleep_for = deadline.saturating_duration_since(now);
            thread::sleep(sleep_for);
            self.last_sleep = sleep_for;
        } else {
            self.last_sleep = Duration::ZERO;
        }

        // Keep cadence relative to the intended deadline; resync if we fell behind
        // or intentionally skipped sleep to refill audio.
        let after = Instant::now();
        let mut next = deadline + FRAME_DURATION;
        if next < after {
            next = after + FRAME_DURATION;
        }
        self.next_deadline = Some(next);
    }
}

/// How the winit event loop should wait before the next scheduler turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostWake {
    /// Throttled playback: sleep until the next host frame deadline.
    WaitUntil(Instant),
    /// Unlimited / unthrottled: poll continuously.
    Poll,
    /// Paused: block until UI or input wakes the loop.
    Wait,
}

/// Host frame deadline accumulator for N× playback (`WaitUntil` / `Poll` / `Wait`).
///
/// Base period is [`FRAME_DURATION`] (~1/59.7275 s). At N× speed the effective
/// period is `base / N`; catch-up counts how many periods elapse behind
/// `next_deadline` (capped per wake).
#[derive(Debug)]
pub struct HostScheduler {
    enabled: bool,
    next_deadline: Option<Instant>,
}

impl HostScheduler {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            next_deadline: None,
        }
    }

    /// Effective host frame period for a finite speed preset.
    #[allow(dead_code)] // exercised in pace unit tests
    pub fn frame_period(speed: SpeedPreset) -> Duration {
        match speed.multiplier() {
            Some(n) => FRAME_DURATION / n,
            None => FRAME_DURATION,
        }
    }

    /// Advance the deadline after a frame is presented at `speed`.
    pub fn on_frame_presented(&mut self, speed: SpeedPreset, now: Instant) {
        if !self.enabled {
            return;
        }
        let Some(n) = speed.multiplier() else {
            return;
        };
        let period = FRAME_DURATION / n;

        self.next_deadline = Some(match self.next_deadline {
            None => now + period,
            Some(deadline) => {
                let next = deadline + period;
                if next < now { now + period } else { next }
            }
        });
    }

    /// Count frames owed relative to `next_deadline`, capped at `max_frames`.
    pub fn frames_to_catch_up(&mut self, speed: SpeedPreset, now: Instant, max_frames: u32) -> u32 {
        if !self.enabled {
            return 0;
        }
        let Some(n) = speed.multiplier() else {
            return 1.min(max_frames);
        };
        let period = FRAME_DURATION / n;

        let Some(deadline) = self.next_deadline else {
            self.next_deadline = Some(now);
            return 1.min(max_frames);
        };

        if now < deadline {
            return 0;
        }

        let behind = now - deadline;
        let periods_behind = behind.as_nanos() / period.as_nanos().max(1);
        let frames = periods_behind as u32 + 1;
        frames.min(max_frames)
    }

    /// Select the event-loop wait policy for the current playback mode.
    ///
    /// When `skip_sleep` is true (audio reservoir critically low at 1×), poll once
    /// instead of sleeping until the next host deadline so emulation can refill PCM.
    pub fn wake(&self, paused: bool, unlimited: bool, skip_sleep: bool, now: Instant) -> HostWake {
        if paused {
            return HostWake::Wait;
        }
        if unlimited || !self.enabled || skip_sleep {
            return HostWake::Poll;
        }
        HostWake::WaitUntil(self.next_deadline.unwrap_or(now))
    }
}

/// Rolling FPS estimate (updates about once per second).
#[derive(Debug)]
pub struct FpsCounter {
    window_start: Instant,
    frames: u32,
}

impl Default for FpsCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl FpsCounter {
    pub fn new() -> Self {
        Self {
            window_start: Instant::now(),
            frames: 0,
        }
    }

    /// Record one presented frame. Returns `Some(fps)` when the 1s window closes.
    pub fn tick(&mut self) -> Option<f64> {
        self.frames += 1;
        let elapsed = self.window_start.elapsed();
        if elapsed < Duration::from_secs(1) {
            return None;
        }
        let fps = f64::from(self.frames) / elapsed.as_secs_f64();
        self.frames = 0;
        self.window_start = Instant::now();
        Some(fps)
    }
}

#[cfg(test)]
mod tests;
