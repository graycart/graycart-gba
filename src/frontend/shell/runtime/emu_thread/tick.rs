use super::super::super::host_input::HostCommand;
use super::super::super::launch::Verbosity;
use super::super::super::pace::{FRAME_DURATION, HostWake};
use super::super::super::playback::{
    SpeedPreset, StepPulse, apply_audio_transition_if_needed, effective_speed,
    run_until_frame_or_budget, should_submit_audio,
};
use super::super::super::rewind::RewindFrame;
use super::super::commands::EmuCommand;
use super::super::frame::{FramePacket, HostDebug, LatestDebug, LatestFrame, PresentFrame};
use super::super::policy::{
    FRAME_ADVANCE_FLASH, FRAME_STEP_BUDGET, catch_up_cap, pending_slot_ready, should_allow_snapshot,
};
use super::machine::Machine;
use super::state::EmuState;
use super::{
    DEBUG_PUBLISH_INTERVAL, RUNTIME_DIAG_INTERVAL, T_CYCLES_PER_FRAME, drain_commands, wait_until,
};
use graycart::{MachineDebug, StereoSample, capture};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};

const GBA_PCM_HZ: u32 = 32_768;
const GBA_CYCLES_PER_FRAME: u64 = 1232 * 228;

impl EmuState {
    pub(super) fn emulate_until_frame_or_budget(&mut self) -> Option<StepPulse> {
        let pulse = match self.machine.as_mut()? {
            Machine::Arm { inner, .. } => {
                let before = inner.frames_done();
                inner.run_frames(1);
                if inner.frames_done() > before {
                    StepPulse::FrameReady
                } else {
                    StepPulse::BudgetExhausted
                }
            }
            Machine::Sm83 {
                cpu, bus, session, ..
            } => {
                if self.tick_profiling {
                    bus.set_tick_profiling(true);
                    bus.tick_profile.reset();
                }

                let mut fault = None;
                let pulse = run_until_frame_or_budget(
                    &mut || {
                        match session.step(cpu, bus) {
                            Ok(_) => {}
                            Err(e) => {
                                fault = Some(e);
                                return true;
                            }
                        }
                        if bus.ppu.take_frame_ready() {
                            session.note_frame();
                            return true;
                        }
                        false
                    },
                    FRAME_STEP_BUDGET,
                );

                if let Some(e) = fault {
                    let report = session.fault_from_step(e, cpu, bus);
                    eprintln!("{report}");
                    self.exit_error = Some(report.to_string());
                    self.quit = true;
                    return None;
                }
                pulse
            }
        };

        if pulse == StepPulse::FrameReady {
            self.note_emulated_frame();
            if matches!(self.machine, Some(Machine::Arm { .. }))
                && let Err(e) = self.flush_save()
            {
                let _ = e;
            }
        }
        Some(pulse)
    }

    pub(super) fn note_emulated_frame(&mut self) {
        let per = match self.machine.as_ref() {
            Some(Machine::Arm { .. }) => GBA_CYCLES_PER_FRAME,
            _ => T_CYCLES_PER_FRAME,
        };
        self.diag_cycles = self.diag_cycles.saturating_add(per);
        self.diag_frames = self.diag_frames.saturating_add(1);
        self.refresh_runtime_rates();
    }

    pub(super) fn refresh_runtime_rates(&mut self) {
        let elapsed = self.diag_window_start.elapsed();
        if elapsed >= Duration::from_millis(250) {
            let secs = elapsed.as_secs_f64().max(1e-9);
            self.runtime_tcycles_per_sec = self.diag_cycles as f64 / secs;
            self.runtime_emu_fps = self.diag_frames as f64 / secs;
            self.diag_window_start = Instant::now();
            self.diag_cycles = 0;
            self.diag_frames = 0;
        }
    }

    pub(super) fn maybe_publish_debug(&mut self, latest_debug: &LatestDebug) {
        if !self.debug_publish {
            return;
        }
        if self.last_debug_publish.elapsed() < DEBUG_PUBLISH_INTERVAL {
            return;
        }
        self.last_debug_publish = Instant::now();
        match self.machine.as_ref() {
            Some(Machine::Sm83 {
                cpu, bus, session, ..
            }) => {
                latest_debug.publish(HostDebug::Sm83(MachineDebug::capture(cpu, bus, session)));
            }
            Some(Machine::Arm { inner, .. }) => {
                latest_debug.publish(HostDebug::Arm(Self::arm_debug_lines(inner)));
            }
            None => {}
        }
    }

    pub(super) fn maybe_log_runtime_diag(&mut self, latest: &LatestFrame) {
        if self.last_runtime_diag_log.elapsed() < RUNTIME_DIAG_INTERVAL {
            return;
        }
        self.last_runtime_diag_log = Instant::now();
        let audio_missing = self
            .audio
            .as_ref()
            .map(|a| a.stats().missing_samples)
            .unwrap_or(0);
        let audio_underruns = self
            .audio
            .as_ref()
            .map(|a| a.stats().underrun_events)
            .unwrap_or(0);
        eprintln!(
            "runtime: {:.3} MHz  frames/s={:.1}  frame_replaced={}  audio_underruns={}  audio_missing={}",
            self.runtime_tcycles_per_sec / 1_000_000.0,
            self.runtime_emu_fps,
            latest.replaced_count(),
            audio_underruns,
            audio_missing,
        );
    }

    pub(super) fn push_rewind_snapshot_if_due(&mut self, frame_done: bool) {
        let boot_active = matches!(
            self.machine.as_ref(),
            Some(Machine::Sm83 { bus, .. }) if bus.boot_rom_active()
        );
        if should_allow_snapshot(boot_active, frame_done)
            && self.rewind_setting
            && !self.host_down.contains(&HostCommand::Rewind)
            && let (Some(ring), Some(machine)) = (self.rewind_ring.as_mut(), self.machine.as_mut())
        {
            match machine {
                Machine::Sm83 { cpu, bus, .. } => {
                    ring.push(RewindFrame::Sm83(capture(cpu, bus)));
                }
                Machine::Arm { inner, .. } => {
                    ring.push(RewindFrame::Arm(inner.capture_state()));
                }
            }
        }
    }

    pub(super) fn submit_frame_audio(&mut self, profile_detail: bool, submit: bool) {
        match self.machine.as_mut() {
            Some(Machine::Sm83 { bus, .. }) => {
                let samples = bus.apu.take_samples();
                if profile_detail {
                    let mut peak_l = 0.0f32;
                    let mut peak_r = 0.0f32;
                    for s in &samples {
                        peak_l = peak_l.max(s.left.abs());
                        peak_r = peak_r.max(s.right.abs());
                    }
                    self.peak_l = peak_l;
                    self.peak_r = peak_r;
                } else {
                    let _ = bus.take_tick_profile();
                }
                if submit && let Some(a) = &self.audio {
                    a.submit_gain(&samples, self.audio_gain);
                }
            }
            Some(Machine::Arm { inner, .. }) => {
                let pcm = inner.bus.apu.drain_pcm();
                if profile_detail {
                    let mut peak_l = 0.0f32;
                    let mut peak_r = 0.0f32;
                    for (l, r) in &pcm {
                        peak_l = peak_l.max((*l as f32 / 32768.0).abs());
                        peak_r = peak_r.max((*r as f32 / 32768.0).abs());
                    }
                    self.peak_l = peak_l;
                    self.peak_r = peak_r;
                }
                if submit && let Some(a) = &self.audio {
                    let host_rate = a.sample_rate;
                    let resampled = crate::frontend::resample_linear(&pcm, GBA_PCM_HZ, host_rate);
                    let samples: Vec<StereoSample> = resampled
                        .iter()
                        .map(|(l, r)| StereoSample {
                            left: *l as f32 / 32768.0,
                            right: *r as f32 / 32768.0,
                        })
                        .collect();
                    a.submit_gain(&samples, self.audio_gain);
                }
            }
            None => {}
        }
    }

    pub(super) fn audio_emergency_catch_up(
        &self,
        paused: bool,
        unlimited: bool,
        rewinding: bool,
        speed: SpeedPreset,
    ) -> bool {
        !paused
            && !unlimited
            && !rewinding
            && speed == SpeedPreset::X1
            && self.config.pacer_enabled
            && self
                .audio
                .as_ref()
                .is_some_and(|a| a.needs_emergency_catch_up())
    }

    pub(super) fn run_tick(&mut self) -> FramePacket {
        let frame_start = Instant::now();
        let now = frame_start;

        let ff_hold = self.host_down.contains(&HostCommand::FastForwardHold);
        let rewinding = self.rewind_setting && self.host_down.contains(&HostCommand::Rewind);
        let speed = if self.config.unthrottled {
            SpeedPreset::Unlimited
        } else {
            effective_speed(
                self.paused,
                rewinding,
                ff_hold,
                self.ff_toggle,
                self.ff_speed,
            )
        };
        let unlimited = self.config.unthrottled || speed == SpeedPreset::Unlimited;

        apply_audio_transition_if_needed(
            self.audio.as_ref(),
            self.prev_speed,
            self.prev_rewinding,
            speed,
            rewinding,
        );
        self.prev_speed = speed;
        self.prev_rewinding = rewinding;

        let mut frame_done = false;
        self.sync_rewind_ring();

        let profile_detail = self.tick_profiling;
        let submit_audio = should_submit_audio(speed, rewinding, self.paused);

        if self.machine.is_some() {
            if self.paused {
                if self.frame_advance_pending {
                    self.frame_advance_pending = false;
                    if let Some(pulse) = self.emulate_until_frame_or_budget() {
                        frame_done = pulse == StepPulse::FrameReady;
                        if frame_done {
                            self.frame_advance_flash_until = Some(now + FRAME_ADVANCE_FLASH);
                            self.push_rewind_snapshot_if_due(true);
                        }
                    }
                    self.submit_frame_audio(profile_detail, submit_audio);
                }
            } else if rewinding {
                self.apply_rewind_scrub();
            } else if unlimited || !self.config.pacer_enabled {
                if let Some(pulse) = self.emulate_until_frame_or_budget() {
                    frame_done = pulse == StepPulse::FrameReady;
                    self.push_rewind_snapshot_if_due(frame_done);
                }
                self.submit_frame_audio(profile_detail, submit_audio);
            } else {
                let cap = catch_up_cap(speed);
                let mut frames = self.scheduler.frames_to_catch_up(speed, now, cap);
                if frames == 0
                    && self.audio_emergency_catch_up(self.paused, unlimited, rewinding, speed)
                {
                    frames = 1;
                }
                if frames > 0 {
                    for _ in 0..frames {
                        if let Some(pulse) = self.emulate_until_frame_or_budget() {
                            if pulse == StepPulse::FrameReady {
                                frame_done = true;
                                self.push_rewind_snapshot_if_due(true);
                                self.scheduler.on_frame_presented(speed, Instant::now());
                                // Submit per emulated frame so PLL re-evaluates
                                // occupancy instead of dumping a catch-up batch
                                // that spikes the queue then starves.
                                self.submit_frame_audio(profile_detail, submit_audio);
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
        }

        let boot_active = matches!(
            self.machine.as_ref(),
            Some(Machine::Sm83 { bus, .. }) if bus.boot_rom_active()
        );
        if self
            .pending_slot
            .is_some_and(|op| pending_slot_ready(op, boot_active, self.paused, frame_done))
        {
            self.apply_pending_slot_op();
        }

        let overlay_speed = if !self.paused && !rewinding && speed != SpeedPreset::X1 {
            Some(speed)
        } else {
            None
        };

        let is_arm = matches!(self.machine, Some(Machine::Arm { .. }));
        let framebuffer = match self.machine.as_ref() {
            Some(Machine::Sm83 { bus, .. }) => {
                Some(PresentFrame::Sm83(bus.ppu.framebuffer.clone()))
            }
            Some(Machine::Arm { inner, .. }) => Some(PresentFrame::Arm(inner.ppu.pixels.to_vec())),
            None => None,
        };
        let title = self
            .machine
            .as_ref()
            .map(|m| m.title().to_string())
            .unwrap_or_default();

        let frame_time = frame_start.elapsed();
        if self.machine.is_some() && !self.paused {
            self.telemetry.note_frame(frame_time, Duration::ZERO);
            if frame_time > FRAME_DURATION + FRAME_DURATION / 2 {
                self.missed_frames = self.missed_frames.saturating_add(1);
            }
        }

        if let Some(measured) = self.fps.tick() {
            self.last_host_fps = measured;
        }

        let (
            audio_queued,
            audio_target,
            audio_missing,
            audio_underrun_events,
            audio_dropped,
            audio_resample_step,
            audio_sample_rate,
            audio_produced,
            audio_consumed,
            audio_callbacks,
            audio_elapsed_secs,
            audio_device,
            audio_channels,
            audio_buffer_size,
            apu_ch1_debug,
        ) = if let Some(a) = &self.audio {
            let stats = a.stats();
            let apu_line = if self.config.verbosity == Verbosity::Verbose {
                match self.machine.as_mut() {
                    Some(Machine::Sm83 { bus, .. }) => {
                        let line = bus.apu.debug_ch1();
                        bus.apu.stats.reset_window();
                        Some(line.to_string())
                    }
                    _ => None,
                }
            } else {
                None
            };
            (
                Some(a.queued_frames()),
                Some(a.target_frames),
                Some(stats.missing_samples),
                Some(stats.underrun_events),
                Some(stats.dropped),
                Some(a.last_resample_step()),
                Some(a.sample_rate),
                Some(stats.produced),
                Some(stats.consumed),
                Some(a.callbacks()),
                Some(a.elapsed_secs()),
                Some(a.device_name.clone()),
                Some(a.channels),
                Some(a.buffer_size.clone()),
                apu_line,
            )
        } else {
            (
                None, None, None, None, None, None, None, None, None, None, None, None, None, None,
                None,
            )
        };

        FramePacket {
            framebuffer,
            rom_loaded: self.machine.is_some(),
            is_arm,
            title,
            paused: self.paused,
            ff_toggle: self.ff_toggle,
            rewinding,
            overlay_speed,
            frame_advance_flash: self
                .frame_advance_flash_until
                .is_some_and(|until| Instant::now() < until),
            status_toast: self.take_status_toast(),
            fault: self.exit_error.clone(),
            last_frame_time: frame_time,
            host_fps: self.last_host_fps,
            runtime_tcycles_per_sec: self.runtime_tcycles_per_sec,
            runtime_emu_fps: self.runtime_emu_fps,
            frame_publish_replaced: 0, // filled by UI from LatestFrame if needed; keep field for packet
            peak_l: self.peak_l,
            peak_r: self.peak_r,
            missed_frames: self.missed_frames,
            audio_queued,
            audio_target,
            audio_missing,
            audio_underrun_events,
            audio_dropped,
            audio_resample_step,
            audio_sample_rate,
            audio_produced,
            audio_consumed,
            audio_callbacks,
            audio_elapsed_secs,
            audio_device,
            audio_channels,
            audio_buffer_size,
            audio_init_error: self.audio_init_error.clone(),
            apu_ch1_debug,
        }
    }

    /// Pace on this thread only: `thread::sleep` + non-blocking command drain.
    /// Returns `false` if the command channel disconnected.
    pub(super) fn sleep_for_wake(&mut self, wake: HostWake, cmd_rx: &Receiver<EmuCommand>) -> bool {
        match wake {
            HostWake::WaitUntil(deadline) => wait_until(deadline, cmd_rx, self),
            HostWake::Poll => {
                thread::yield_now();
                true
            }
            HostWake::Wait => {
                let end = Instant::now() + Duration::from_millis(16);
                while Instant::now() < end {
                    if !drain_commands(self, cmd_rx) {
                        return false;
                    }
                    if self.quit {
                        return true;
                    }
                    let rem = end.saturating_duration_since(Instant::now());
                    if rem.is_zero() {
                        break;
                    }
                    thread::sleep(rem.min(Duration::from_millis(1)));
                }
                true
            }
        }
    }
}
