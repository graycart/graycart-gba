//! Host-side metrics packed for the Debug Monitor (frontend only).

use super::super::audio::AudioOut;
use super::super::pace::FramePacer;
use super::super::telemetry::HostTelemetry;
use super::profile::{FrameProfile, ProfileSummary};
use std::time::Duration;

/// Audio counters forwarded from the emulation thread via [`crate::frontend::shell::runtime::FramePacket`].
#[derive(Debug, Clone, Default)]
pub struct RuntimeAudioMetrics {
    pub queued: usize,
    pub target: usize,
    pub underrun_events: u64,
    pub missing_samples: u64,
    pub dropped: u64,
    pub resample_step: f64,
    pub sample_rate: u32,
    pub produced: u64,
    pub consumed: u64,
    pub callbacks: u64,
    pub elapsed_secs: f64,
    pub device: String,
    pub channels: u16,
    pub buffer_size: String,
}

#[derive(Debug, Clone, Default)]
pub struct HostMetrics {
    pub emu_tcycles_per_sec: f64,
    pub emu_fps: f64,
    pub host_fps: f64,
    pub frame_ms_avg: f64,
    pub frame_ms_min: f64,
    pub frame_ms_max: f64,
    pub render_ms_avg: f64,
    pub render_ms_max: f64,
    pub pace_sleep_ms: f64,
    pub missed_frames: u64,
    pub input_events_per_sec: f64,
    pub redraws: u64,
    pub audio_sample_rate: u32,
    pub audio_queued: usize,
    pub audio_target: usize,
    pub audio_queue_pct: f64,
    pub audio_underrun_events: u64,
    pub audio_missing_samples: u64,
    pub audio_overruns: u64,
    pub audio_produced: u64,
    pub audio_consumed: u64,
    pub audio_callbacks: u64,
    pub audio_elapsed_secs: f64,
    pub audio_device: String,
    pub audio_channels: u16,
    pub audio_buffer_size: String,
    pub audio_init_error: Option<String>,
    pub resample_step: f64,
    pub peak_l: f32,
    pub peak_r: f32,
    pub vsync: bool,
    pub pacer: bool,
    pub profile: FrameProfile,
    pub profile_summary: ProfileSummary,
    pub rom_title: String,
}

impl HostMetrics {
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        telemetry: &HostTelemetry,
        audio: Option<&AudioOut>,
        pacer: &FramePacer,
        input_events_per_sec: f64,
        host_fps: f64,
        missed_frames: u64,
        peak_l: f32,
        peak_r: f32,
        vsync: bool,
        pacer_on: bool,
        last_frame: Duration,
        last_render: Duration,
        profile: FrameProfile,
        rom_title: String,
        // Prefer emulation-thread measurement over UI-derived Hz when set.
        runtime_tcycles_per_sec: Option<f64>,
        runtime_emu_fps: Option<f64>,
        audio_from_runtime: Option<RuntimeAudioMetrics>,
        audio_init_error: Option<String>,
    ) -> Self {
        let (emu_hz, emu_fps, frame_avg, frame_max, render_avg, render_max, redraws) =
            telemetry.instant_rates();
        let emu_hz = runtime_tcycles_per_sec.unwrap_or(emu_hz);
        let emu_fps = runtime_emu_fps.unwrap_or(emu_fps);
        let (
            queued,
            target,
            underrun_events,
            missing,
            overruns,
            produced,
            consumed,
            step,
            rate,
            callbacks,
            elapsed,
            device,
            channels,
            buffer_size,
        ) = if let Some(r) = audio_from_runtime {
            (
                r.queued,
                r.target,
                r.underrun_events,
                r.missing_samples,
                r.dropped,
                r.produced,
                r.consumed,
                r.resample_step,
                r.sample_rate,
                r.callbacks,
                r.elapsed_secs,
                r.device,
                r.channels,
                r.buffer_size,
            )
        } else {
            match audio {
                Some(a) => {
                    let s = a.stats();
                    (
                        a.queued_frames(),
                        a.target_frames,
                        s.underrun_events,
                        s.missing_samples,
                        s.dropped,
                        s.produced,
                        s.consumed,
                        a.last_resample_step(),
                        a.sample_rate,
                        a.callbacks(),
                        a.elapsed_secs(),
                        a.device_name.clone(),
                        a.channels,
                        a.buffer_size.clone(),
                    )
                }
                None => (
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    1.0,
                    0,
                    0,
                    0.0,
                    String::new(),
                    0,
                    String::new(),
                ),
            }
        };
        let pct = if target > 0 {
            queued as f64 / target as f64 * 100.0
        } else {
            0.0
        };
        let profile_summary = ProfileSummary::from_last(profile, 16.742);
        Self {
            emu_tcycles_per_sec: emu_hz,
            emu_fps,
            host_fps,
            frame_ms_avg: if frame_avg > 0.0 {
                frame_avg
            } else {
                last_frame.as_secs_f64() * 1000.0
            },
            frame_ms_min: telemetry.frame_ms_min(),
            frame_ms_max: frame_max.max(last_frame.as_secs_f64() * 1000.0),
            render_ms_avg: render_avg.max(last_render.as_secs_f64() * 1000.0),
            render_ms_max: render_max.max(last_render.as_secs_f64() * 1000.0),
            pace_sleep_ms: pacer.last_sleep.as_secs_f64() * 1000.0,
            missed_frames,
            input_events_per_sec,
            redraws,
            audio_sample_rate: rate,
            audio_queued: queued,
            audio_target: target,
            audio_queue_pct: pct,
            audio_underrun_events: underrun_events,
            audio_missing_samples: missing,
            audio_overruns: overruns,
            audio_produced: produced,
            audio_consumed: consumed,
            audio_callbacks: callbacks,
            audio_elapsed_secs: elapsed,
            audio_device: device,
            audio_channels: channels,
            audio_buffer_size: buffer_size,
            audio_init_error,
            resample_step: step,
            peak_l,
            peak_r,
            vsync,
            pacer: pacer_on,
            profile,
            profile_summary,
            rom_title,
        }
    }

    pub fn realtime_pct(&self) -> f64 {
        self.emu_tcycles_per_sec / 4_194_304.0 * 100.0
    }

    /// Configured play with no host stream / sample rate is not "quiet but healthy".
    pub fn audio_offline(&self) -> bool {
        self.audio_sample_rate == 0 || self.audio_target == 0
    }
}
