//! Wall-clock frame pacing for the windowed host (~GBA 59.73 Hz).
//!
//! Cited: GBATEK LCD timing — 228 lines × 1232 cycles
//!   https://problemkaputt.de/gbatek.htm
//! Note: host-only; silicon timing lives in `ppu::timing`.

use graycart_gba::ppu::FRAME_CYCLES;
use std::thread;
use std::time::{Duration, Instant};

/// GBA master clock (Hz).
pub const GBA_HZ: u64 = 16_777_216;

/// Nominal frame period from [`FRAME_CYCLES`].
pub const FRAME_DURATION: Duration =
    Duration::from_nanos((FRAME_CYCLES as u64) * 1_000_000_000 / GBA_HZ);

/// Sleep after each presented frame so wall-clock ≈ GBA cadence.
#[derive(Debug)]
pub struct FramePacer {
    enabled: bool,
    next_deadline: Option<Instant>,
}

impl FramePacer {
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            next_deadline: None,
        }
    }

    /// Call once after presenting a frame. No-op when disabled / unthrottled.
    pub fn after_present(&mut self) {
        if !self.enabled {
            return;
        }
        let now = Instant::now();
        let Some(deadline) = self.next_deadline else {
            self.next_deadline = Some(now + FRAME_DURATION);
            return;
        };
        if now < deadline {
            thread::sleep(deadline.saturating_duration_since(now));
        }
        let after = Instant::now();
        let mut next = deadline + FRAME_DURATION;
        if next < after {
            next = after + FRAME_DURATION;
        }
        self.next_deadline = Some(next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_duration_is_positive_and_near_60hz() {
        assert!(FRAME_DURATION.as_millis() >= 15 && FRAME_DURATION.as_millis() <= 18);
    }
}
