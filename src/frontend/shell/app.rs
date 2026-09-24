//! Phase 9 — winit app shell + GPU present + egui UI.
//!
//! Emulator core stays untouched: this module only drives Boot/CPU/Bus through
//! public APIs and maps host input/audio/display/UI.

use super::boot_rom::{
    any_cached_boot_firmware, clear_boot_rom, install_boot_rom, install_cgb_boot_rom,
};
use super::brand;
use super::controls::ControlsWindow;
use super::debug::{
    DebugMonitor, DebugMonitorAction, DiagnosticRing, FrameProfile, HostMetrics,
    RuntimeAudioMetrics, build_snapshot_report,
};
use super::host::{
    AppFocus, InputCapture, app_focus_from_surfaces, emit_app_unfocused, input_policy,
};
use super::host_input::{HostCommand, HostCommandMap, InputFrontend, PollResult};
use super::launch::{LaunchRom, Verbosity, cold_launch_rom_attached};
use super::pace::{FramePacer, target_fps};
use super::playback::{on_main_window_focus_loss, toggle_paused};
use super::report::{
    ConsentOutcome, collect_bug_defaults, crash_dir, install_panic_hook, load_pending,
    write_fault_envelope,
};
use super::rom::{OpenedRom, is_rom_path, open_rom_file, rom_dialog_extensions};
use super::runtime::{EmuCommand, EmulationRuntime, PresentFrame, RuntimeConfig};
use super::screenshot::write_presented_png;
use super::settings::FrontendSettings;
use super::telemetry::{HostTelemetry, PaceDiag};
use super::ui::{Gui, UiAction};
use super::video::{Renderer, SCALE, window_attributes};
use graycart::hw::CGB_BOOT_ROM_SIZE;
use graycart::{BootMode, SCREEN_HEIGHT, SCREEN_WIDTH};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::WindowId;

/// Open the application window. Flushes `.sav` on exit when a ROM was loaded.
pub fn run() -> Result<(), String> {
    // SM83 bus is larger than the default Windows main-thread stack.
    std::thread::Builder::new()
        .name("gba-play".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(run_on_thread)
        .map_err(|e| e.to_string())?
        .join()
        .unwrap_or_else(|_| Err("play thread panicked".into()))
}

fn run_on_thread() -> Result<(), String> {
    run_inner(None, BootMode::Fast, Verbosity::Normal, false, None)
}

fn run_inner(
    initial: Option<LaunchRom>,
    boot_mode: BootMode,
    verbosity: Verbosity,
    unthrottled: bool,
    hardware_cli: Option<graycart::HostHardwarePref>,
) -> Result<(), String> {
    install_panic_hook();
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;

    let mut settings = FrontendSettings::load();
    if matches!(boot_mode, BootMode::Fast) {
        settings.skip_boot = true;
    }
    let hardware_pref = hardware_cli.unwrap_or(settings.hardware_pref);

    let pace = PaceDiag::from_env(unthrottled);

    let resolved_boot = if settings.skip_boot {
        BootMode::Fast
    } else if exe_dir().is_some_and(|dir| any_cached_boot_firmware(&dir)) {
        BootMode::BootRom
    } else {
        BootMode::Fast
    };

    let initial_path = initial.as_ref().map(|l| l.path());
    let initial_loaded = cold_launch_rom_attached(initial.as_ref());

    let runtime = EmulationRuntime::spawn(RuntimeConfig {
        unthrottled,
        pacer_enabled: pace.pacer,
        vsync: pace.vsync,
        verbosity,
        boot_mode: resolved_boot,
        ff_speed: settings.ff_speed,
        rewind_enabled: settings.rewind_enabled,
        audio_gain: settings.audio_gain(),
        audio_output: settings.audio_output.clone(),
        hardware_pref,
        initial_rom: initial,
    });

    let mut app = App {
        verbosity,
        unthrottled,
        pace,
        settings,
        runtime,
        renderer: None,
        gui: None,
        debug_monitor: None,
        controls_window: None,
        input: InputFrontend::new(),
        last_input_poll: PollResult::default(),
        keys_down: HashSet::new(),
        telemetry: HostTelemetry::new(),
        diagnostics: DiagnosticRing::new(),
        last_profile: FrameProfile::default(),
        sample_clock: Instant::now(),
        last_frame_time: Duration::ZERO,
        last_render_time: Duration::ZERO,
        last_host_fps: 0.0,
        runtime_tcycles_per_sec: 0.0,
        runtime_emu_fps: 0.0,
        runtime_audio: None,
        audio_init_error: None,
        peak_l: 0.0,
        peak_r: 0.0,
        missed_frames: 0,
        cached_title: String::new(),
        cached_rom_path: initial_path.unwrap_or_default(),
        rom_loaded: initial_loaded,
        is_arm: false,
        fb_w: SCREEN_WIDTH,
        fb_h: SCREEN_HEIGHT,
        display_pacer: FramePacer::new(pace.pacer),
        ready: false,
        exit_error: None,
        fullscreen: false,
        host_down: HashSet::new(),
        app_focus: AppFocus::Main,
        main_focused: true,
        aux_focused: false,
        status_toast: None,
        status_toast_until: None,
        pending_crash_checked: false,
    };

    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.settings.save();
    let _ = app.runtime.shutdown();
    if let Some(err) = app.exit_error {
        return Err(err);
    }
    Ok(())
}

struct App {
    verbosity: Verbosity,
    unthrottled: bool,
    pace: PaceDiag,
    settings: FrontendSettings,
    runtime: EmulationRuntime,
    renderer: Option<Renderer>,
    gui: Option<Gui>,
    debug_monitor: Option<DebugMonitor>,
    controls_window: Option<ControlsWindow>,
    input: InputFrontend,
    last_input_poll: PollResult,
    keys_down: HashSet<KeyCode>,
    telemetry: HostTelemetry,
    diagnostics: DiagnosticRing,
    last_profile: FrameProfile,
    sample_clock: Instant,
    last_frame_time: Duration,
    last_render_time: Duration,
    last_host_fps: f64,
    runtime_tcycles_per_sec: f64,
    runtime_emu_fps: f64,
    runtime_audio: Option<RuntimeAudioMetrics>,
    audio_init_error: Option<String>,
    peak_l: f32,
    peak_r: f32,
    missed_frames: u64,
    cached_title: String,
    cached_rom_path: PathBuf,
    rom_loaded: bool,
    is_arm: bool,
    fb_w: usize,
    fb_h: usize,
    display_pacer: FramePacer,
    ready: bool,
    exit_error: Option<String>,
    fullscreen: bool,
    host_down: HashSet<HostCommand>,
    app_focus: AppFocus,
    main_focused: bool,
    aux_focused: bool,
    status_toast: Option<String>,
    status_toast_until: Option<Instant>,
    pending_crash_checked: bool,
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

fn set_clipboard_text(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut cb| cb.set_text(text.to_string()))
        .map_err(|e| e.to_string())
}

impl App {
    fn boot_mode(&self) -> BootMode {
        if self.settings.skip_boot {
            BootMode::Fast
        } else if exe_dir().is_some_and(|dir| any_cached_boot_firmware(&dir)) {
            BootMode::BootRom
        } else {
            BootMode::Fast
        }
    }

    fn capture_mode(&self) -> InputCapture {
        if self.input.is_listening() {
            InputCapture::Rebinding
        } else if self
            .gui
            .as_ref()
            .is_some_and(|g| g.text_input_owns_keyboard())
        {
            InputCapture::TextEntry
        } else {
            InputCapture::Gameplay
        }
    }

    fn aux_window_open(&self) -> bool {
        self.controls_window.is_some() || self.debug_monitor.is_some()
    }

    fn apply_input(&mut self) {
        let policy = input_policy(self.app_focus, self.capture_mode(), self.aux_window_open());
        if policy.clear_held_keyboard {
            self.keys_down.clear();
        }
        let deliver = self.rom_loaded;
        let result = self.input.poll(
            &mut self.settings,
            &self.keys_down,
            policy.keyboard_to_joypad,
            deliver,
        );
        if result.frame.settings_dirty {
            self.settings.save();
        }
        let mask = if policy.gamepad_to_joypad {
            if self.is_arm {
                result.gba_mask
            } else {
                u16::from(result.effective_mask)
            }
        } else {
            0
        };
        self.runtime.send(EmuCommand::SetButtons(mask));
        self.last_input_poll = result;
    }

    fn service_host(&mut self, event_loop: &ActiveEventLoop) {
        if !self.ready {
            return;
        }
        self.maybe_prompt_pending_crash();
        self.sync_status_toast();
        self.dispatch_ui_actions(event_loop);
        self.sync_runtime_settings();
        self.apply_input();
        if let Some(packet) = self.runtime.take_frame() {
            self.apply_frame_packet(event_loop, packet);
        }
        self.refresh_debug_monitor();
    }

    fn sync_runtime_settings(&mut self) {
        self.runtime
            .send(EmuCommand::SetRewindEnabled(self.settings.rewind_enabled));
        self.runtime
            .send(EmuCommand::SetAudioGain(self.settings.audio_gain()));
        self.runtime
            .send(EmuCommand::SetFfSpeed(self.settings.ff_speed));
    }

    fn set_status_toast(&mut self, message: String) {
        self.status_toast = Some(message);
        self.status_toast_until = Some(Instant::now() + Duration::from_secs(3));
        if self.verbosity != Verbosity::Quiet {
            eprintln!("status: {}", self.status_toast.as_deref().unwrap_or(""));
        }
    }

    fn sync_status_toast(&mut self) {
        if let Some(g) = self.gui.as_mut() {
            g.runtime.status_toast = self.status_toast.clone();
        }
        if let Some(until) = self.status_toast_until
            && Instant::now() >= until
        {
            self.status_toast = None;
            self.status_toast_until = None;
            if let Some(g) = self.gui.as_mut() {
                g.runtime.status_toast = None;
            }
        }
    }

    fn take_screenshot(&mut self) {
        if !self.rom_loaded {
            return;
        };
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        let rgba = renderer.presented_rgba();
        match write_presented_png(&self.cached_title, rgba, self.fb_w as u32, self.fb_h as u32) {
            Ok(path) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                self.set_status_toast(format!("Screenshot: {name}"));
            }
            Err(e) => self.set_status_toast(format!("Screenshot failed: {e}")),
        }
    }

    fn host_command_for_key_map(map: &HostCommandMap, key: KeyCode) -> Option<HostCommand> {
        HostCommandMap::ALL
            .iter()
            .copied()
            .find(|cmd| map.key(*cmd) == key)
    }

    fn handle_host_command_press(&mut self, cmd: HostCommand, event_loop: &ActiveEventLoop) {
        match cmd {
            HostCommand::QuickSave => self.runtime.send(EmuCommand::SlotSave(0)),
            HostCommand::QuickLoad => self.runtime.send(EmuCommand::SlotLoad(0)),
            HostCommand::Screenshot => self.take_screenshot(),
            HostCommand::Rewind => {
                self.host_down.insert(HostCommand::Rewind);
                self.runtime
                    .send(EmuCommand::HostPress(HostCommand::Rewind));
            }
            HostCommand::Fullscreen => {
                self.fullscreen = !self.fullscreen;
                if let Some(r) = self.renderer.as_ref() {
                    r.set_fullscreen(self.fullscreen);
                }
            }
            HostCommand::Monitor => self.toggle_debug_monitor(event_loop),
            HostCommand::Pause => {
                if let Some(g) = self.gui.as_mut() {
                    toggle_paused(&mut g.runtime.paused, &mut self.host_down);
                    self.runtime.send(EmuCommand::SetPaused(g.runtime.paused));
                }
            }
            HostCommand::FrameAdvance => self.runtime.send(EmuCommand::FrameAdvance),
            HostCommand::FastForwardHold => {
                self.host_down.insert(HostCommand::FastForwardHold);
                self.runtime
                    .send(EmuCommand::HostPress(HostCommand::FastForwardHold));
            }
            HostCommand::ToggleFastForward => {
                if let Some(g) = self.gui.as_mut() {
                    g.runtime.ff_toggle = !g.runtime.ff_toggle;
                    self.runtime
                        .send(EmuCommand::SetFfToggle(g.runtime.ff_toggle));
                }
            }
        }
    }

    fn quit(&mut self, event_loop: &ActiveEventLoop) {
        self.settings.save();
        if let Err(e) = self.runtime.flush_save() {
            self.exit_error = Some(e);
        }
        event_loop.exit();
    }

    fn apply_frame_packet(
        &mut self,
        event_loop: &ActiveEventLoop,
        packet: super::runtime::FramePacket,
    ) {
        self.rom_loaded = packet.rom_loaded;
        self.cached_title = packet.title.clone();
        self.last_frame_time = packet.last_frame_time;
        self.last_host_fps = packet.host_fps;
        self.runtime_tcycles_per_sec = packet.runtime_tcycles_per_sec;
        self.runtime_emu_fps = packet.runtime_emu_fps;
        self.runtime_audio = match (
            packet.audio_queued,
            packet.audio_target,
            packet.audio_missing,
            packet.audio_underrun_events,
            packet.audio_dropped,
            packet.audio_resample_step,
            packet.audio_sample_rate,
            packet.audio_produced,
            packet.audio_consumed,
            packet.audio_callbacks,
            packet.audio_elapsed_secs,
            packet.audio_device,
            packet.audio_channels,
            packet.audio_buffer_size,
        ) {
            (
                Some(queued),
                Some(target),
                Some(missing),
                Some(underrun),
                Some(dropped),
                Some(step),
                Some(rate),
                Some(produced),
                Some(consumed),
                Some(callbacks),
                Some(elapsed),
                Some(device),
                Some(channels),
                Some(buffer_size),
            ) => Some(RuntimeAudioMetrics {
                queued,
                target,
                underrun_events: underrun,
                missing_samples: missing,
                dropped,
                resample_step: step,
                sample_rate: rate,
                produced,
                consumed,
                callbacks,
                elapsed_secs: elapsed,
                device,
                channels,
                buffer_size,
            }),
            _ => None,
        };
        self.audio_init_error = packet.audio_init_error;
        self.peak_l = packet.peak_l;
        self.peak_r = packet.peak_r;
        self.missed_frames = packet.missed_frames;

        if let Some(g) = self.gui.as_mut() {
            g.runtime.rom_loaded = packet.rom_loaded;
            g.runtime.is_arm = packet.is_arm;
            g.runtime.paused = packet.paused;
            g.runtime.ff_toggle = packet.ff_toggle;
            g.runtime.overlay_rewinding = packet.rewinding;
            g.runtime.overlay_paused = packet.paused;
            g.runtime.overlay_frame_advance = packet.frame_advance_flash;
            g.runtime.overlay_speed = packet.overlay_speed;
        }

        if let Some(renderer) = self.renderer.as_mut()
            && packet.rom_loaded
        {
            renderer.set_base_title(&brand::window_title(&packet.title));
        }

        self.is_arm = packet.is_arm;
        if packet.is_arm {
            self.fb_w = 240;
            self.fb_h = 160;
        } else if packet.rom_loaded {
            self.fb_w = SCREEN_WIDTH;
            self.fb_h = SCREEN_HEIGHT;
        }

        if let Some(toast) = packet.status_toast {
            self.set_status_toast(toast);
        }

        if let Some(fb) = packet.framebuffer {
            let palette = self.settings.active_palette();
            let mode = self.settings.display_mode;
            if let Some(renderer) = self.renderer.as_mut() {
                match fb {
                    PresentFrame::Sm83(fb) => {
                        if !packet.is_arm {
                            renderer.write_framebuffer(&fb, palette, mode);
                        }
                    }
                    PresentFrame::Arm(pixels) => {
                        renderer.write_gba_bgr555(&pixels);
                    }
                }
            }
            if !packet.paused {
                self.telemetry
                    .note_frame(packet.last_frame_time, self.last_render_time);
            }
        }

        if self.verbosity == Verbosity::Verbose
            && let (Some(q), Some(t), Some(u), Some(step)) = (
                packet.audio_queued,
                packet.audio_target,
                packet.audio_missing,
                packet.audio_resample_step,
            )
            && let Some(line) =
                self.telemetry
                    .take_report_line(Some(q), Some(t), Some(u), Some(step))
        {
            eprintln!("{line}");
            if let Some(apu) = &packet.apu_ch1_debug {
                eprintln!("{apu}");
            }
        }

        if let Some(fault) = packet.fault {
            eprintln!("{fault}");
            let fields = collect_bug_defaults(
                &self.settings,
                &self.cached_title,
                &self.cached_rom_path.to_string_lossy(),
                self.boot_mode(),
                &fault,
                "Emulation fault (FaultReport)",
            );
            if let Err(e) = write_fault_envelope(&fault, &fields) {
                eprintln!("crash envelope: {e}");
            }
            if let Some(gui) = self.gui.as_mut() {
                if let Some(dir) = crash_dir()
                    && let Some((meta, md)) = load_pending(&dir)
                {
                    gui.report.begin_crash_consent(meta, md, true);
                } else {
                    // Still quit if we couldn't open consent UI state.
                    self.exit_error = Some(fault);
                    self.quit(event_loop);
                }
            } else {
                self.exit_error = Some(fault);
                self.quit(event_loop);
            }
        }
    }

    fn maybe_prompt_pending_crash(&mut self) {
        if self.pending_crash_checked {
            return;
        }
        self.pending_crash_checked = true;
        let Some(gui) = self.gui.as_mut() else {
            return;
        };
        if gui.report.crash_consent.is_some() {
            return;
        }
        let Some(dir) = crash_dir() else {
            return;
        };
        if let Some((meta, md)) = load_pending(&dir) {
            // Next-launch path for panics (and unanswered fault envelopes).
            gui.report.begin_crash_consent(meta, md, false);
        }
    }

    fn handle_report_consent(&mut self, event_loop: &ActiveEventLoop) {
        let Some(gui) = self.gui.as_mut() else {
            return;
        };
        let quit_after = gui.report.quit_after_consent;
        let status = gui.report.last_status.take();
        let outcome = gui.take_consent_outcome();
        if quit_after && matches!(outcome, ConsentOutcome::Sent | ConsentOutcome::Dismissed) {
            if let Some(g) = self.gui.as_mut() {
                g.report.quit_after_consent = false;
            }
            if let Some(msg) = status {
                self.set_status_toast(msg);
            }
            self.quit(event_loop);
            return;
        }
        if let Some(msg) = status {
            self.set_status_toast(msg);
        }
    }

    fn present_frame(&mut self, timed: bool) -> Result<(Duration, Duration, Duration), String> {
        let gui_present = self.gui.is_some() && self.renderer.is_some();
        if !gui_present {
            return Ok((Duration::ZERO, Duration::ZERO, Duration::ZERO));
        }

        if !self.rom_loaded {
            let palette = self.settings.active_palette();
            self.renderer.as_mut().unwrap().write_empty(palette);
        }

        let needed = self.fb_w.saturating_mul(self.fb_h).saturating_mul(4);
        let game_rgba: Option<Vec<u8>> = self.rom_loaded.then(|| {
            let full = self.renderer.as_ref().unwrap().presented_rgba();
            full.get(..needed).unwrap_or(full).to_vec()
        });

        if !timed {
            {
                let renderer = self.renderer.as_ref().unwrap();
                let gui = self.gui.as_mut().unwrap();
                gui.prepare(
                    renderer.window().as_ref(),
                    &mut self.settings,
                    game_rgba.as_deref(),
                    self.fb_w,
                    self.fb_h,
                );
            }
            {
                let gui = self.gui.as_mut().unwrap();
                let renderer = self.renderer.as_mut().unwrap();
                renderer.present_with(|encoder, target, ctx| {
                    gui.render(encoder, target, ctx);
                    Ok(())
                })?;
            }
            return Ok((Duration::ZERO, Duration::ZERO, Duration::ZERO));
        }

        let t_egui_prep = Instant::now();
        {
            let renderer = self.renderer.as_ref().unwrap();
            let gui = self.gui.as_mut().unwrap();
            gui.prepare(
                renderer.window().as_ref(),
                &mut self.settings,
                game_rgba.as_deref(),
                self.fb_w,
                self.fb_h,
            );
        }
        let egui_prep = t_egui_prep.elapsed();

        let t_fb = Instant::now();
        let framebuffer = t_fb.elapsed();

        let t_gpu = Instant::now();
        {
            let gui = self.gui.as_mut().unwrap();
            let renderer = self.renderer.as_mut().unwrap();
            renderer.present_with(|encoder, target, ctx| {
                gui.render(encoder, target, ctx);
                Ok(())
            })?;
        }
        let gpu = t_gpu.elapsed();
        Ok((framebuffer, egui_prep, gpu))
    }

    fn poll_ui_frame(&mut self, event_loop: &ActiveEventLoop) {
        let frame_start = Instant::now();
        self.service_host(event_loop);

        let profile_detail = self.debug_monitor.is_some() || self.diagnostics.capture_pending();
        let (framebuffer, egui, gpu) = match self.present_frame(profile_detail) {
            Ok(t) => t,
            Err(e) => {
                self.exit_error = Some(e);
                self.quit(event_loop);
                return;
            }
        };
        self.handle_report_consent(event_loop);
        let render_time = if profile_detail {
            framebuffer + egui + gpu
        } else {
            frame_start.elapsed()
        };
        self.last_render_time = render_time;

        if self.verbosity == Verbosity::Verbose
            && self.last_host_fps > 0.0
            && let Some(r) = self.renderer.as_ref()
        {
            r.set_fps_title(self.last_host_fps);
        }

        let debug_monitor_cost = self
            .debug_monitor
            .as_ref()
            .map(|m| m.last_redraw_cost)
            .unwrap_or(Duration::ZERO);

        self.last_profile = FrameProfile::from_parts(
            Duration::ZERO,
            graycart::TickProfile::default(),
            Duration::ZERO,
            framebuffer,
            egui,
            gpu,
            Duration::ZERO,
            debug_monitor_cost,
            self.last_frame_time,
        );

        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(8),
        ));
    }

    fn build_host_metrics(&mut self, debug_snapshot: Duration) -> HostMetrics {
        let mut profile = self.last_profile;
        profile.debug_snapshot = debug_snapshot;
        if let Some(m) = self.debug_monitor.as_ref() {
            profile.debug_monitor = m.last_redraw_cost;
        }
        HostMetrics::from_parts(
            &self.telemetry,
            None,
            &self.display_pacer,
            self.telemetry.input_events_per_sec(),
            self.last_host_fps,
            self.missed_frames,
            self.peak_l,
            self.peak_r,
            self.pace.vsync,
            self.pace.pacer,
            self.last_frame_time,
            self.last_render_time,
            profile,
            self.cached_title.clone(),
            Some(self.runtime_tcycles_per_sec),
            Some(self.runtime_emu_fps),
            self.runtime_audio.clone(),
            self.audio_init_error.clone(),
        )
    }

    fn refresh_debug_monitor(&mut self) {
        self.refresh_debug_monitor_ex(false);
    }

    fn refresh_debug_monitor_ex(&mut self, force: bool) {
        let due = force || self.sample_clock.elapsed() >= Duration::from_millis(66);
        if !due {
            if let Some(mon) = self.debug_monitor.as_mut() {
                mon.maybe_request_redraw();
            }
            return;
        }
        self.sample_clock = Instant::now();

        let want_machine = self.debug_monitor.is_some() || self.diagnostics.capture_pending();
        let (debug, snap_cost) = if want_machine {
            let t_snap = Instant::now();
            let d = self.runtime.take_debug_snapshot();
            (d, t_snap.elapsed())
        } else {
            (None, Duration::ZERO)
        };
        let host = self.build_host_metrics(snap_cost);
        self.diagnostics.push(host.clone(), host.profile);

        let pending = self.diagnostics.capture_pending();
        let remaining = self
            .diagnostics
            .capture_remaining()
            .map(|d| d.as_secs_f32());
        let has_report = self.diagnostics.last_report.is_some();
        let last_report = self.diagnostics.last_report.clone();

        if let Some(mon) = self.debug_monitor.as_mut() {
            mon.update_cache(debug, host, pending, remaining, has_report, last_report);
            mon.maybe_request_redraw();
        }
    }

    fn arm_capture(&mut self) {
        let title = if self.cached_title.is_empty() {
            "(none)".to_string()
        } else {
            self.cached_title.clone()
        };
        let path = self.cached_rom_path.display().to_string();
        let debug = self.runtime.take_debug_snapshot();
        self.diagnostics.arm_capture(title, path, debug);
        eprintln!("debug: capture armed (−15s … +5s)");
    }

    fn copy_text_to_clipboard(&mut self, label: &str, text: &str) {
        match set_clipboard_text(text) {
            Ok(()) => {
                self.set_status_toast(format!(
                    "Copied {label} to clipboard ({} bytes)",
                    text.len()
                ));
            }
            Err(e) => {
                self.set_status_toast(format!("Clipboard copy failed ({e}) — use Save… instead"));
            }
        }
    }

    fn copy_report(&mut self) {
        match self.diagnostics.last_report.clone() {
            Some(report) => self.copy_text_to_clipboard("report", &report),
            None => self.set_status_toast("No capture report yet — arm Capture first".into()),
        }
    }

    fn copy_snapshot(&mut self) {
        let title = if self.cached_title.is_empty() {
            "(none)".to_string()
        } else {
            self.cached_title.clone()
        };
        let path = self.cached_rom_path.display().to_string();
        let debug = self.runtime.take_debug_snapshot();
        let host = self.build_host_metrics(Duration::ZERO);
        let report = build_snapshot_report(&title, &path, &host, debug.as_ref());
        self.copy_text_to_clipboard("snapshot", &report);
    }

    fn save_report(&mut self) {
        let Some(report) = self.diagnostics.last_report.as_ref() else {
            return;
        };
        let path = rfd::FileDialog::new()
            .set_file_name("graycart-diagnostic.md")
            .add_filter("Markdown", &["md", "txt"])
            .save_file();
        if let Some(path) = path {
            match std::fs::write(&path, report) {
                Ok(()) => eprintln!("debug: wrote {}", path.display()),
                Err(e) => eprintln!("debug: save failed: {e}"),
            }
        }
    }

    fn persist_monitor_settings(&mut self) {
        if let Some(mon) = self.debug_monitor.as_ref() {
            self.settings.monitor = mon.export_settings();
            self.settings.save();
        }
    }

    fn toggle_debug_monitor(&mut self, event_loop: &ActiveEventLoop) {
        if self.debug_monitor.is_some() {
            self.persist_monitor_settings();
            self.debug_monitor = None;
            self.runtime.send(EmuCommand::SetTickProfiling(false));
            self.runtime.send(EmuCommand::SetDebugPublish(false));
            if let Some(g) = self.gui.as_mut() {
                g.runtime.debug_monitor_open = false;
            }
            self.diagnostics.note_event("monitor closed");
            return;
        }
        let game = self.renderer.as_ref().map(|r| r.window().as_ref());
        match DebugMonitor::open(event_loop, game, &self.settings.monitor) {
            Ok(mon) => {
                self.debug_monitor = Some(mon);
                self.runtime.send(EmuCommand::SetTickProfiling(true));
                self.runtime.send(EmuCommand::SetDebugPublish(true));
                if let Some(g) = self.gui.as_mut() {
                    g.runtime.debug_monitor_open = true;
                }
                self.diagnostics.note_event("monitor opened");
                self.refresh_debug_monitor_ex(true);
            }
            Err(e) => eprintln!("debug monitor: {e}"),
        }
    }

    fn toggle_controls_window(&mut self, event_loop: &ActiveEventLoop) {
        if self.controls_window.is_some() {
            self.close_controls_window();
            return;
        }
        let game = self.renderer.as_ref().map(|r| r.window().as_ref());
        match ControlsWindow::open(event_loop, game) {
            Ok(win) => {
                self.controls_window = Some(win);
                if let Some(g) = self.gui.as_mut() {
                    g.runtime.controls_open = true;
                }
            }
            Err(e) => eprintln!("controls window: {e}"),
        }
    }

    fn close_controls_window(&mut self) {
        self.input.cancel();
        self.controls_window = None;
        if let Some(g) = self.gui.as_mut() {
            g.runtime.controls_open = false;
        }
    }

    fn refresh_controls_window(&mut self) {
        let Some(win) = self.controls_window.as_mut() else {
            return;
        };
        if let Err(e) = win.redraw(&mut self.settings, &mut self.input, &self.last_input_poll) {
            eprintln!("controls window: {e}");
            self.close_controls_window();
        }
    }

    fn handle_monitor_actions(&mut self) {
        let action = self.debug_monitor.as_mut().and_then(|m| m.take_action());
        match action {
            Some(DebugMonitorAction::Capture) => self.arm_capture(),
            Some(DebugMonitorAction::CopyReport) => self.copy_report(),
            Some(DebugMonitorAction::SaveReport) => self.save_report(),
            Some(DebugMonitorAction::CopySnapshot) => self.copy_snapshot(),
            Some(DebugMonitorAction::ViewReport) => {}
            None => {}
        }
    }

    fn dispatch_ui_actions(&mut self, event_loop: &ActiveEventLoop) {
        let actions = self
            .gui
            .as_mut()
            .map(|g| g.take_actions())
            .unwrap_or_default();
        for action in actions {
            match action {
                UiAction::Quit => {
                    self.quit(event_loop);
                    return;
                }
                UiAction::TogglePause => {
                    if let Some(g) = self.gui.as_mut() {
                        toggle_paused(&mut g.runtime.paused, &mut self.host_down);
                        self.runtime.send(EmuCommand::SetPaused(g.runtime.paused));
                    }
                }
                UiAction::SetFfSpeed(preset) => {
                    self.settings.ff_speed = preset;
                    self.settings.save();
                    self.runtime.send(EmuCommand::SetFfSpeed(preset));
                }
                UiAction::SetHardwarePref(pref) => {
                    self.settings.hardware_pref = pref;
                    self.settings.save();
                    self.runtime.send(EmuCommand::SetHardwarePref(pref));
                }
                UiAction::SetAudioOutput(pref) => {
                    self.settings.audio_output = pref.clone();
                    self.settings.save();
                    self.runtime.send(EmuCommand::SetAudioOutput(pref));
                }
                UiAction::Reset => self.runtime.send(EmuCommand::Reset {
                    boot_mode: self.boot_mode(),
                }),
                UiAction::OpenRomDialog => {
                    if let Some(path) = self.pick_rom()
                        && let Err(e) = self.load_rom_path(&path)
                    {
                        eprintln!("{e}");
                    }
                }
                UiAction::LoadRom(path) => {
                    if let Err(e) = self.load_rom_path(&path) {
                        eprintln!("{e}");
                    }
                }
                UiAction::ToggleFullscreen => {
                    self.fullscreen = !self.fullscreen;
                    if let Some(r) = self.renderer.as_ref() {
                        r.set_fullscreen(self.fullscreen);
                    }
                }
                UiAction::ToggleDebugMonitor => {
                    self.toggle_debug_monitor(event_loop);
                }
                UiAction::ToggleControls => {
                    self.toggle_controls_window(event_loop);
                }
                UiAction::QuickSave => self.runtime.send(EmuCommand::SlotSave(0)),
                UiAction::QuickLoad => self.runtime.send(EmuCommand::SlotLoad(0)),
                UiAction::SaveSlot(slot) => self.runtime.send(EmuCommand::SlotSave(slot)),
                UiAction::LoadSlot(slot) => self.runtime.send(EmuCommand::SlotLoad(slot)),
                UiAction::TakeScreenshot => self.take_screenshot(),
                UiAction::InstallBootRom => self.install_boot_rom_from_dialog(),
                UiAction::ClearBootRom => self.clear_installed_boot_rom(),
                UiAction::ReportBug => self.open_report_bug(),
                UiAction::RequestFeature => {
                    if let Some(g) = self.gui.as_mut() {
                        g.report.open_feature();
                    }
                }
                UiAction::ConfigureGithubToken => {
                    if let Some(g) = self.gui.as_mut() {
                        g.report.open_token_dialog(&self.settings);
                    }
                }
            }
        }
    }

    fn open_report_bug(&mut self) {
        let diagnostics = self
            .diagnostics
            .last_report
            .clone()
            .unwrap_or_else(|| "none".into());
        let fields = collect_bug_defaults(
            &self.settings,
            &self.cached_title,
            &self.cached_rom_path.to_string_lossy(),
            self.boot_mode(),
            &diagnostics,
            "",
        );
        if let Some(g) = self.gui.as_mut() {
            g.report.open_bug(fields);
        }
    }

    fn install_boot_rom_from_dialog(&mut self) {
        if self.is_arm {
            self.set_status_toast("Boot ROM install applies to Game Boy carts only".into());
            return;
        }
        let Some(exe_dir) = exe_dir() else {
            self.set_status_toast(
                "Install failed: could not resolve executable directory".to_string(),
            );
            return;
        };
        let picked = rfd::FileDialog::new()
            .add_filter("Boot ROM", rom_dialog_extensions())
            .set_title("Install Boot ROM")
            .pick_file();
        let Some(path) = picked else {
            return;
        };
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                self.set_status_toast(format!("Install failed: {e}"));
                return;
            }
        };
        let result = match bytes.len() {
            256 => install_boot_rom(&exe_dir, &bytes),
            CGB_BOOT_ROM_SIZE => install_cgb_boot_rom(&exe_dir, &bytes),
            n => Err(format!(
                "boot ROM must be exactly 256 (DMG) or {CGB_BOOT_ROM_SIZE} (CGB) bytes (got {n})"
            )),
        };
        match result {
            Ok(()) => {
                self.set_status_toast("Boot ROM installed (takes effect on next reset)".to_string())
            }
            Err(e) => self.set_status_toast(format!("Install failed: {e}")),
        }
    }

    fn clear_installed_boot_rom(&mut self) {
        if self.is_arm {
            self.set_status_toast("Boot ROM clear applies to Game Boy carts only".into());
            return;
        }
        let Some(exe_dir) = exe_dir() else {
            self.set_status_toast(
                "Clear failed: could not resolve executable directory".to_string(),
            );
            return;
        };
        match clear_boot_rom(&exe_dir) {
            Ok(()) => {
                self.set_status_toast("Boot ROM cleared (takes effect on next reset)".to_string())
            }
            Err(e) => self.set_status_toast(format!("Clear failed: {e}")),
        }
    }

    fn pick_rom(&self) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new()
            .add_filter("GBA / Game Boy ROM", rom_dialog_extensions())
            .set_title("Open ROM");
        if let Some(dir) = &self.settings.last_rom_dir {
            dialog = dialog.set_directory(dir);
        }
        dialog.pick_file()
    }

    fn load_rom_path(&mut self, path: &Path) -> Result<(), String> {
        self.runtime.flush_save()?;
        let opened = open_rom_file(path)?;
        if self.verbosity != Verbosity::Quiet {
            println!("loaded {}", opened.path().display());
            println!("title: {}", opened.title());
            println!("save: {}", opened.save_path().display());
        }
        self.install_opened(opened)?;
        Ok(())
    }

    fn install_opened(&mut self, opened: OpenedRom) -> Result<(), String> {
        self.is_arm = opened.is_arm();
        if self.is_arm {
            self.fb_w = 240;
            self.fb_h = 160;
        } else {
            self.fb_w = SCREEN_WIDTH;
            self.fb_h = SCREEN_HEIGHT;
        }
        let path = opened.path().to_path_buf();
        let title = opened.title().to_string();
        self.settings.remember_rom(&path, &title);
        self.settings.save();
        self.cached_rom_path = path;
        self.rom_loaded = true;
        self.input.reset_edges();
        self.runtime.send(EmuCommand::LoadRom(opened));
        Ok(())
    }

    fn note_window_focus(&mut self, window_id: WindowId, focused: bool) {
        let is_main = self
            .renderer
            .as_ref()
            .is_some_and(|r| r.window().id() == window_id);
        let is_aux = self
            .debug_monitor
            .as_ref()
            .is_some_and(|m| m.window_id() == window_id)
            || self
                .controls_window
                .as_ref()
                .is_some_and(|w| w.window_id() == window_id);
        if is_main {
            self.main_focused = focused;
        }
        if is_aux {
            self.aux_focused = focused;
        }
        self.app_focus = app_focus_from_surfaces(self.main_focused, self.aux_focused);
        if emit_app_unfocused(self.app_focus, self.aux_window_open()) {
            let pause_when_unfocused = self.settings.pause_when_unfocused;
            if let Some(g) = self.gui.as_mut() {
                on_main_window_focus_loss(
                    &mut self.host_down,
                    pause_when_unfocused,
                    &mut g.runtime.paused,
                    &mut || {},
                );
                self.runtime.send(EmuCommand::SetPaused(g.runtime.paused));
            }
            self.runtime.send(EmuCommand::FocusLost {
                pause_when_unfocused,
            });
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }

        let title = brand::window_title(&self.cached_title);
        let attrs = window_attributes(&title);
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.exit_error = Some(e.to_string());
                event_loop.exit();
                return;
            }
        };

        let renderer = match Renderer::new(Arc::clone(&window), &title, self.pace.vsync) {
            Ok(r) => r,
            Err(e) => {
                self.exit_error = Some(e);
                event_loop.exit();
                return;
            }
        };

        let size = window.inner_size();
        let mut gui = Gui::new(
            event_loop,
            size.width,
            size.height,
            window.scale_factor() as f32,
            renderer.pixels(),
        );
        gui.runtime.rom_loaded = self.rom_loaded;
        gui.runtime.is_arm = self.is_arm;

        self.renderer = Some(renderer);
        self.gui = Some(gui);

        if self.verbosity != Verbosity::Quiet {
            println!("audio: CPAL stream owned by emulation thread; callback consumes the ring");
            print!("{}", super::audio::probe_audio_backends());
            if self.unthrottled {
                println!("display: 160×144 @ {SCALE}× scale, unthrottled");
            } else {
                println!(
                    "display: 160×144 @ {SCALE}× scale, ~{:.1} FPS pacing",
                    target_fps()
                );
            }
            println!(
                "pace: HostScheduler vsync={} pacer={} (GRAYCART_VSYNC / GRAYCART_PACER)",
                self.pace.vsync, self.pace.pacer
            );
            #[cfg(debug_assertions)]
            println!("note: DEBUG build — play with `cargo run --release` (debug is not realtime)");
            println!(
                "keys: arrows=D-pad  Z=B  X=A  Right Shift=Select  Enter=Start  F5=QuickSave  F8=QuickLoad  F6=Screenshot  hold R=Rewind  F11=Fullscreen  F12=Monitor  F9=Capture"
            );
        }

        self.runtime.send(EmuCommand::Start);
        self.sync_runtime_settings();
        self.ready = true;
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.ready {
            self.poll_ui_frame(event_loop);
        }
        if let Some(mon) = self.debug_monitor.as_mut() {
            mon.maybe_request_redraw();
        }
        if let Some(win) = self.controls_window.as_ref() {
            win.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::Focused(focused) = &event {
            self.note_window_focus(window_id, *focused);
        }
        self.service_host(event_loop);

        if self
            .debug_monitor
            .as_ref()
            .is_some_and(|m| m.window_id() == window_id)
        {
            match &event {
                WindowEvent::CloseRequested => {
                    self.persist_monitor_settings();
                    self.debug_monitor = None;
                    self.runtime.send(EmuCommand::SetTickProfiling(false));
                    self.runtime.send(EmuCommand::SetDebugPublish(false));
                    if let Some(g) = self.gui.as_mut() {
                        g.runtime.debug_monitor_open = false;
                    }
                    self.diagnostics.note_event("monitor closed");
                }
                WindowEvent::RedrawRequested => {
                    if let Some(mon) = self.debug_monitor.as_mut()
                        && let Err(e) = mon.redraw()
                    {
                        eprintln!("debug monitor: {e}");
                        self.persist_monitor_settings();
                        self.debug_monitor = None;
                        self.runtime.send(EmuCommand::SetTickProfiling(false));
                        self.runtime.send(EmuCommand::SetDebugPublish(false));
                        if let Some(g) = self.gui.as_mut() {
                            g.runtime.debug_monitor_open = false;
                        }
                    }
                    self.handle_monitor_actions();
                }
                other => {
                    if let Some(mon) = self.debug_monitor.as_mut() {
                        let _ = mon.handle_event(other);
                    }
                }
            }
            return;
        }

        // Configure Controls — native window; remapping keys arrive here when focused.
        if self
            .controls_window
            .as_ref()
            .is_some_and(|w| w.window_id() == window_id)
        {
            match &event {
                WindowEvent::CloseRequested => {
                    self.close_controls_window();
                }
                WindowEvent::RedrawRequested => {
                    self.refresh_controls_window();
                }
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key: PhysicalKey::Code(code),
                            state,
                            repeat: false,
                            ..
                        },
                    ..
                } => match state {
                    ElementState::Pressed => {
                        if self.input.is_listening() {
                            let consumed = self.input.on_listen_key(
                                *code,
                                &mut self.settings.input.keyboard,
                                &self.settings.host_commands,
                            );
                            if consumed && *code != KeyCode::Escape {
                                self.settings.save();
                            }
                            return;
                        }
                        if *code == KeyCode::Escape {
                            self.close_controls_window();
                        }
                    }
                    ElementState::Released => {}
                },
                other => {
                    if let Some(win) = self.controls_window.as_mut() {
                        let _ = win.handle_event(other);
                    }
                }
            }
            return;
        }

        match &event {
            WindowEvent::KeyboardInput { .. }
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. } => {
                self.telemetry.note_input();
            }
            WindowEvent::RedrawRequested => {
                self.telemetry.note_redraw();
            }
            _ => {}
        }

        if let Some(gui) = self.gui.as_mut()
            && let Some(renderer) = self.renderer.as_ref()
        {
            let _ = gui.handle_event(renderer.window(), &event);
        }

        match event {
            WindowEvent::CloseRequested => {
                self.quit(event_loop);
            }
            WindowEvent::DroppedFile(path) => {
                if is_rom_path(&path)
                    && let Err(e) = self.load_rom_path(&path)
                {
                    eprintln!("{e}");
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(r) = self.renderer.as_mut()
                    && let Err(e) = r.resize_surface(size.width, size.height)
                {
                    self.exit_error = Some(e);
                    self.quit(event_loop);
                    return;
                }
                if let Some(g) = self.gui.as_mut() {
                    g.resize(size.width, size.height);
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(g) = self.gui.as_mut() {
                    g.scale_factor(scale_factor);
                }
            }
            WindowEvent::Focused(_) => {}
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        repeat: false,
                        ..
                    },
                ..
            } => {
                let text_owns = self
                    .gui
                    .as_ref()
                    .is_some_and(|g| g.text_input_owns_keyboard());
                match state {
                    ElementState::Pressed => {
                        if self.input.is_listening() {
                            let consumed = self.input.on_listen_key(
                                code,
                                &mut self.settings.input.keyboard,
                                &self.settings.host_commands,
                            );
                            if consumed && code != KeyCode::Escape {
                                self.settings.save();
                            }
                            return;
                        }
                        if code == KeyCode::F9 {
                            self.arm_capture();
                            return;
                        }
                        if let Some(cmd) =
                            Self::host_command_for_key_map(&self.settings.host_commands, code)
                        {
                            self.handle_host_command_press(cmd, event_loop);
                            return;
                        }
                        if code == KeyCode::Escape && !self.input.is_listening() {
                            if self.controls_window.is_some() {
                                self.close_controls_window();
                                return;
                            }
                            if self
                                .gui
                                .as_mut()
                                .is_some_and(|g| g.dismiss_config_on_escape())
                            {
                                return;
                            }
                            if !text_owns {
                                self.quit(event_loop);
                                return;
                            }
                        }
                        self.keys_down.insert(code);
                    }
                    ElementState::Released => {
                        let host_cmd =
                            Self::host_command_for_key_map(&self.settings.host_commands, code);
                        if let Some(cmd) = host_cmd {
                            self.host_down.remove(&cmd);
                            self.runtime.send(EmuCommand::HostRelease(cmd));
                        } else {
                            self.keys_down.remove(&code);
                        }
                    }
                }
            }
            // Present path lives in `about_to_wait` so OS redraw frequency cannot
            // throttle emulated wall-clock. Count-only here for telemetry.
            WindowEvent::RedrawRequested => {}
            _ => {}
        }
    }
}
