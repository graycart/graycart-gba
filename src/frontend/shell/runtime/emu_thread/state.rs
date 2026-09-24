//! Emulation-thread mutable state.

use super::super::super::audio::AudioOut;
use super::super::super::host_input::HostCommand;
use super::super::super::pace::{FpsCounter, HostScheduler};
use super::super::super::playback::SpeedPreset;
use super::super::super::rewind::RewindRing;
use super::super::super::state_slots::PendingSlotOp;
use super::super::super::telemetry::HostTelemetry;
use super::super::RuntimeConfig;
use super::machine::Machine;
use std::collections::HashSet;
use std::time::Instant;

pub(super) struct EmuState {
    pub config: RuntimeConfig,
    pub machine: Option<Machine>,
    pub audio: Option<AudioOut>,
    pub audio_init_error: Option<String>,
    pub scheduler: HostScheduler,
    pub fps: FpsCounter,
    pub telemetry: HostTelemetry,
    pub host_down: HashSet<HostCommand>,
    pub rewind_ring: Option<RewindRing>,
    pub rewind_setting: bool,
    pub paused: bool,
    pub ff_toggle: bool,
    pub ff_speed: SpeedPreset,
    pub audio_gain: f32,
    pub button_mask: u16,
    pub pending_slot: Option<PendingSlotOp>,
    pub frame_advance_pending: bool,
    pub frame_advance_flash_until: Option<Instant>,
    pub prev_speed: SpeedPreset,
    pub prev_rewinding: bool,
    pub tick_profiling: bool,
    pub debug_publish: bool,
    pub last_debug_publish: Instant,
    pub diag_window_start: Instant,
    pub diag_cycles: u64,
    pub diag_frames: u64,
    pub runtime_tcycles_per_sec: f64,
    pub runtime_emu_fps: f64,
    pub last_runtime_diag_log: Instant,
    pub missed_frames: u64,
    pub last_host_fps: f64,
    pub peak_l: f32,
    pub peak_r: f32,
    pub ready: bool,
    pub quit: bool,
    pub status_toast: Option<String>,
    pub exit_error: Option<String>,
}

impl EmuState {
    pub(super) fn new(config: RuntimeConfig) -> Self {
        let ff_speed = config.ff_speed;
        let rewind_setting = config.rewind_enabled;
        let audio_gain = config.audio_gain;
        let pacer_enabled = config.pacer_enabled;
        Self {
            config,
            machine: None,
            audio: None,
            audio_init_error: None,
            scheduler: HostScheduler::new(pacer_enabled),
            fps: FpsCounter::new(),
            telemetry: HostTelemetry::new(),
            host_down: HashSet::new(),
            rewind_ring: None,
            rewind_setting,
            paused: false,
            ff_toggle: false,
            ff_speed,
            audio_gain,
            button_mask: 0,
            pending_slot: None,
            frame_advance_pending: false,
            frame_advance_flash_until: None,
            prev_speed: SpeedPreset::X1,
            prev_rewinding: false,
            tick_profiling: false,
            debug_publish: false,
            last_debug_publish: Instant::now(),
            diag_window_start: Instant::now(),
            diag_cycles: 0,
            diag_frames: 0,
            runtime_tcycles_per_sec: 0.0,
            runtime_emu_fps: 0.0,
            last_runtime_diag_log: Instant::now(),
            missed_frames: 0,
            last_host_fps: 0.0,
            peak_l: 0.0,
            peak_r: 0.0,
            ready: false,
            quit: false,
            status_toast: None,
            exit_error: None,
        }
    }
}
