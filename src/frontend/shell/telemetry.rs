//! Wall-clock host pacing telemetry (frontend only).

use super::pace::{GB_CPU_HZ, T_CYCLES_PER_FRAME};
use std::time::{Duration, Instant};

/// Rolling ~5 s window of machine + host scheduling stats.
#[derive(Debug)]
pub struct HostTelemetry {
    window_start: Instant,
    frames: u64,
    redraws: u64,
    input_events: u64,
    frame_time_sum: Duration,
    frame_time_max: Duration,
    frame_time_min: Duration,
    render_time_sum: Duration,
    render_time_max: Duration,
}

impl Default for HostTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl HostTelemetry {
    pub fn new() -> Self {
        Self {
            window_start: Instant::now(),
            frames: 0,
            redraws: 0,
            input_events: 0,
            frame_time_sum: Duration::ZERO,
            frame_time_max: Duration::ZERO,
            frame_time_min: Duration::MAX,
            render_time_sum: Duration::ZERO,
            render_time_max: Duration::ZERO,
        }
    }

    pub fn note_input(&mut self) {
        self.input_events = self.input_events.saturating_add(1);
    }

    pub fn note_redraw(&mut self) {
        self.redraws = self.redraws.saturating_add(1);
    }

    /// Record one completed emulated frame + host timings.
    pub fn note_frame(&mut self, frame_time: Duration, render_time: Duration) {
        self.frames = self.frames.saturating_add(1);
        self.frame_time_sum += frame_time;
        self.render_time_sum += render_time;
        if frame_time > self.frame_time_max {
            self.frame_time_max = frame_time;
        }
        if frame_time < self.frame_time_min {
            self.frame_time_min = frame_time;
        }
        if render_time > self.render_time_max {
            self.render_time_max = render_time;
        }
    }

    /// Live rates for the Debug Monitor (does not reset the window).
    ///
    /// Returns `(emu_hz, emu_fps, frame_avg_ms, frame_max_ms, render_avg_ms, render_max_ms, redraws)`.
    pub fn instant_rates(&self) -> (f64, f64, f64, f64, f64, f64, u64) {
        let secs = self.window_start.elapsed().as_secs_f64().max(1e-9);
        if self.frames == 0 {
            return (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, self.redraws);
        }
        let frames = self.frames as f64;
        let emu_hz = (frames * T_CYCLES_PER_FRAME as f64) / secs;
        let fps = frames / secs;
        let avg_ms = self.frame_time_sum.as_secs_f64() * 1000.0 / frames;
        let max_ms = self.frame_time_max.as_secs_f64() * 1000.0;
        let render_avg = self.render_time_sum.as_secs_f64() * 1000.0 / frames;
        let render_max = self.render_time_max.as_secs_f64() * 1000.0;
        (
            emu_hz,
            fps,
            avg_ms,
            max_ms,
            render_avg,
            render_max,
            self.redraws,
        )
    }

    pub fn input_events_per_sec(&self) -> f64 {
        let secs = self.window_start.elapsed().as_secs_f64().max(1e-9);
        self.input_events as f64 / secs
    }

    pub fn frame_ms_min(&self) -> f64 {
        if self.frames == 0 || self.frame_time_min == Duration::MAX {
            0.0
        } else {
            self.frame_time_min.as_secs_f64() * 1000.0
        }
    }

    /// When the window is ≥5 s, format a line and reset. Otherwise `None`.
    pub fn take_report_line(
        &mut self,
        audio_queued: Option<usize>,
        audio_target: Option<usize>,
        audio_underruns: Option<u64>,
        resample_step: Option<f64>,
    ) -> Option<String> {
        let elapsed = self.window_start.elapsed();
        if elapsed < Duration::from_secs(5) || self.frames == 0 {
            return None;
        }
        let (emu_hz, fps, avg_ms, max_ms, render_avg, render_max, _) = self.instant_rates();
        let speed = emu_hz / GB_CPU_HZ as f64;

        let audio = match (audio_queued, audio_target, audio_underruns, resample_step) {
            (Some(q), Some(t), Some(u), Some(s)) => {
                format!(" audio_queue={q}/{t} underruns={u} step={s:.5}")
            }
            _ => String::new(),
        };

        let line = format!(
            "host: emu_tcycles_per_sec={emu_hz:.0} (×{speed:.4}) frames_per_sec={fps:.3} \
             frame_ms_avg={avg_ms:.3} frame_ms_max={max_ms:.3} \
             render_ms_avg={render_avg:.3} render_ms_max={render_max:.3} \
             redraws={} input_events={}{audio}",
            self.redraws, self.input_events
        );

        *self = Self::new();
        Some(line)
    }
}

/// Diagnostic overrides via env (not shipped as UI options).
#[derive(Debug, Clone, Copy)]
pub struct PaceDiag {
    /// When true, pixels uses AutoVsync (default false — FramePacer is sole authority).
    pub vsync: bool,
    /// When false, FramePacer sleeps are disabled.
    pub pacer: bool,
}

impl PaceDiag {
    pub fn from_env(unthrottled: bool) -> Self {
        let vsync = env_flag_migrated("GRAYCART_VSYNC", "GB_EMU_VSYNC", false);
        let pacer = if unthrottled {
            false
        } else {
            env_flag_migrated("GRAYCART_PACER", "GB_EMU_PACER", true)
        };
        Self { vsync, pacer }
    }
}

fn env_flag_migrated(primary: &str, legacy: &str, default: bool) -> bool {
    if std::env::var_os(primary).is_some() {
        return env_flag(primary, default);
    }
    if std::env::var_os(legacy).is_some() {
        return env_flag(legacy, default);
    }
    default
}

fn env_flag(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_waits_for_window() {
        let mut t = HostTelemetry::new();
        t.note_frame(Duration::from_millis(16), Duration::from_millis(1));
        assert!(t.take_report_line(None, None, None, None).is_none());
    }
}
