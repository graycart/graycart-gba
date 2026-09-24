use super::super::super::audio::{AudioOut, bind_bus_host_rate};
use super::super::super::boot_rom::{load_cached_boot_rom, load_cached_cgb_boot_rom};
use super::super::super::host_input::HostCommand;
use super::super::super::launch::Verbosity;
use super::super::super::rewind::{RewindFrame, RewindRing};
use super::super::super::rom::OpenedRom;
use super::super::super::state_slots::{
    PendingSlotOp, load_arm_slot, load_slot, save_arm_slot, save_slot,
};
use super::super::commands::EmuCommand;
use super::machine::{Machine, exe_dir, sm83_from_cart};
use super::state::EmuState;
use crate::compat::{AGB_A, AGB_B};
use crate::debug::{MachineDebug as GbaMachineDebug, live_cpu_line};
use crate::ppu::sprite_count;
use graycart::hw::CGB_BOOT_ROM_SIZE;
use graycart::{
    BootMode, Cpu, ExecSession, GameBoyButton, HardwareModel, capture, flush_save, prepare_boot,
    restore,
};

impl EmuState {
    pub(super) fn handle_command(&mut self, cmd: EmuCommand) {
        match cmd {
            EmuCommand::Quit => self.quit = true,
            EmuCommand::Start => {
                match AudioOut::open_pref(&self.config.audio_output) {
                    Ok(a) => {
                        if self.config.verbosity != Verbosity::Quiet {
                            println!("{}", a.startup_line());
                        }
                        self.audio = Some(a);
                        self.audio_init_error = None;
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        self.set_status_toast(format!("AUDIO OFFLINE: {}", e.message));
                        self.audio = None;
                        self.audio_init_error = Some(e.message);
                    }
                }
                if let Some(launch) = self.config.initial_rom.take() {
                    let _ = self.install_opened(launch.opened);
                }
                self.ready = true;
            }
            EmuCommand::LoadRom(opened) => {
                let _ = self.flush_save();
                let _ = self.install_opened(opened);
            }
            EmuCommand::Reset { boot_mode } => {
                let _ = self.reset_machine(boot_mode);
            }
            EmuCommand::SetButtons(mask) => {
                self.apply_button_mask(mask);
            }
            EmuCommand::HostPress(cmd) => {
                if cmd == HostCommand::Rewind || cmd == HostCommand::FastForwardHold {
                    self.host_down.insert(cmd);
                }
            }
            EmuCommand::HostRelease(cmd) => {
                self.host_down.remove(&cmd);
                if cmd == HostCommand::Rewind
                    && let Some(ring) = self.rewind_ring.as_mut()
                {
                    ring.end_scrub();
                }
            }
            EmuCommand::FocusLost {
                pause_when_unfocused,
            } => {
                let mut ff_hold = self.host_down.contains(&HostCommand::FastForwardHold);
                let mut rewind_held = self.host_down.contains(&HostCommand::Rewind);
                if super::super::super::playback::clear_transient_holds(
                    &mut ff_hold,
                    &mut rewind_held,
                ) {
                    self.host_down.remove(&HostCommand::FastForwardHold);
                    self.host_down.remove(&HostCommand::Rewind);
                    if let Some(ring) = self.rewind_ring.as_mut() {
                        ring.end_scrub();
                    }
                }
                if pause_when_unfocused {
                    self.paused = true;
                }
            }
            EmuCommand::SetPaused(p) => {
                self.paused = p;
                if p {
                    self.host_down.remove(&HostCommand::FastForwardHold);
                }
            }
            EmuCommand::SetFfSpeed(s) => self.ff_speed = s,
            EmuCommand::SetFfToggle(t) => self.ff_toggle = t,
            EmuCommand::SetRewindEnabled(e) => {
                self.rewind_setting = e;
                if !e {
                    self.rewind_ring = None;
                }
            }
            EmuCommand::SetAudioGain(g) => self.audio_gain = g,
            EmuCommand::SetTickProfiling(on) => {
                self.tick_profiling = on;
                if let Some(Machine::Sm83 { bus, .. }) = self.machine.as_mut() {
                    bus.set_tick_profiling(on);
                }
            }
            EmuCommand::SetDebugPublish(on) => {
                self.debug_publish = on;
            }
            EmuCommand::SlotSave(slot) => {
                self.pending_slot = Some(PendingSlotOp::Save(slot));
            }
            EmuCommand::SlotLoad(slot) => {
                self.pending_slot = Some(PendingSlotOp::Load(slot));
            }
            EmuCommand::FrameAdvance => {
                if self.paused {
                    self.frame_advance_pending = true;
                }
            }
            EmuCommand::FlushSave { reply } => {
                let result = self.flush_save();
                let _ = reply.send(result);
            }
            EmuCommand::SetHardwarePref(pref) => {
                self.config.hardware_pref = pref;
                let Some(Machine::Sm83 {
                    path,
                    save_path,
                    title,
                    ..
                }) = self.machine.as_ref()
                else {
                    self.set_status_toast(
                        "Hardware preference applies to Game Boy carts only".into(),
                    );
                    return;
                };
                let path = path.clone();
                let save_path = save_path.clone();
                let title = title.clone();
                match graycart::Cartridge::load(&path) {
                    Ok(cart) => {
                        let _ = self.flush_save();
                        match sm83_from_cart(path, save_path, title, cart, pref) {
                            Ok(m) => {
                                self.machine = Some(m);
                                let _ = self.reset_machine(self.config.boot_mode);
                            }
                            Err(e) => self.set_status_toast(e),
                        }
                    }
                    Err(e) => self.set_status_toast(e.to_string()),
                }
            }
            EmuCommand::SetAudioOutput(pref) => {
                self.config.audio_output = pref.clone();
                match AudioOut::open_pref(&pref) {
                    Ok(a) => {
                        if self.config.verbosity != Verbosity::Quiet {
                            println!("{}", a.startup_line());
                        }
                        self.audio = Some(a);
                        self.audio_init_error = None;
                        self.bind_host_audio();
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        self.set_status_toast(format!("AUDIO OFFLINE: {}", e.message));
                        self.audio = None;
                        self.audio_init_error = Some(e.message);
                    }
                }
            }
        }
    }

    pub(super) fn install_opened(&mut self, opened: OpenedRom) -> Result<(), String> {
        match opened {
            OpenedRom::Arm {
                path,
                save_path,
                title,
                machine,
            } => {
                self.machine = Some(Machine::Arm {
                    path,
                    save_path,
                    title,
                    inner: machine,
                });
                self.button_mask = 0;
                self.reset_rewind_ring();
                self.paused = false;
                Ok(())
            }
            OpenedRom::Sm83 {
                path,
                save_path,
                title,
                cart,
            } => {
                let mut machine =
                    sm83_from_cart(path, save_path, title, cart, self.config.hardware_pref)?;
                if let Some(Machine::Sm83 { bus, .. }) = Some(&mut machine)
                    && let Some(ref a) = self.audio
                {
                    bind_bus_host_rate(bus, Some(a.sample_rate));
                }
                self.machine = Some(machine);
                let _ = self.reset_machine(self.config.boot_mode);
                Ok(())
            }
        }
    }

    pub(super) fn flush_save(&mut self) -> Result<(), String> {
        let Some(machine) = self.machine.as_mut() else {
            return Ok(());
        };
        match machine {
            Machine::Arm { inner, .. } => match inner.flush_save() {
                Ok(()) => Ok(()),
                Err(e) => {
                    eprintln!("gba-debug: warn save flush");
                    Err(e)
                }
            },
            Machine::Sm83 { save_path, bus, .. } => {
                let path_display = save_path.display().to_string();
                match flush_save(save_path, &mut bus.cartridge) {
                    Ok(true) => {
                        if self.config.verbosity != Verbosity::Quiet {
                            println!("save: wrote {path_display}");
                        }
                        Ok(())
                    }
                    Ok(false) => Ok(()),
                    Err(e) => {
                        eprintln!("gba-debug: warn save flush");
                        Err(format!("failed to write save {path_display}: {e}"))
                    }
                }
            }
        }
    }

    pub(super) fn reset_machine(&mut self, boot_mode: BootMode) -> Result<(), String> {
        let Some(machine) = self.machine.as_mut() else {
            return Ok(());
        };
        self.pending_slot = None;
        match machine {
            Machine::Arm {
                path,
                save_path,
                title,
                ..
            } => {
                let path = path.clone();
                let save_path = save_path.clone();
                let title = title.clone();
                match crate::hw::Machine::open(&path) {
                    Ok(inner) => {
                        self.machine = Some(Machine::Arm {
                            path,
                            save_path,
                            title,
                            inner: Box::new(inner),
                        });
                    }
                    Err(e) => self.set_status_toast(e),
                }
            }
            Machine::Sm83 {
                cpu, bus, session, ..
            } => {
                let mut mode = boot_mode;
                let cgb_silicon = matches!(bus.hardware_model(), HardwareModel::Cgb { .. });

                enum Overlay {
                    Dmg(Box<[u8; 256]>),
                    Cgb(Box<[u8; CGB_BOOT_ROM_SIZE]>),
                    None,
                }

                let overlay = if mode == BootMode::BootRom {
                    if cgb_silicon {
                        match exe_dir().and_then(|dir| load_cached_cgb_boot_rom(&dir)) {
                            Some(bytes) => Overlay::Cgb(bytes),
                            None => {
                                mode = BootMode::Fast;
                                Overlay::None
                            }
                        }
                    } else {
                        match exe_dir().and_then(|dir| load_cached_boot_rom(&dir)) {
                            Some(bytes) => Overlay::Dmg(Box::new(bytes)),
                            None => {
                                mode = BootMode::Fast;
                                Overlay::None
                            }
                        }
                    }
                } else {
                    Overlay::None
                };

                *cpu = Cpu::new();
                *session = ExecSession::new();
                bus.power_on_keep_battery();
                match &overlay {
                    Overlay::Dmg(bytes) => bus.enable_boot_rom(bytes),
                    Overlay::Cgb(bytes) => bus.enable_cgb_boot_rom(bytes),
                    Overlay::None => {}
                }
                prepare_boot(mode, cpu, bus);
                Machine::apply_agb_regs(cpu);
                bind_bus_host_rate(bus, self.audio.as_ref().map(|a| a.sample_rate));
                let _ = bus.apu.take_samples();
                bus.apu.stats.reset_window();
            }
        }
        self.paused = false;
        self.ff_toggle = false;
        self.host_down.remove(&HostCommand::FastForwardHold);
        self.sync_buttons_to_joypad();
        self.reset_rewind_ring();
        Ok(())
    }

    pub(super) fn apply_button_mask(&mut self, mask: u16) {
        if mask == self.button_mask {
            return;
        }
        let Some(machine) = self.machine.as_mut() else {
            self.button_mask = mask;
            return;
        };
        match machine {
            Machine::Arm { inner, .. } => {
                inner.bus.keypad.set_pressed(mask);
            }
            Machine::Sm83 { bus, .. } => {
                let prev = self.button_mask as u8;
                let now = mask as u8;
                for i in 0..8u8 {
                    let bit = 1u8 << i;
                    let was = prev & bit != 0;
                    let is = now & bit != 0;
                    if was == is {
                        continue;
                    }
                    let button = GameBoyButton::ALL[i as usize];
                    if is {
                        bus.press_button(button);
                    } else {
                        bus.release_button(button);
                    }
                }
            }
        }
        self.button_mask = mask;
    }

    pub(super) fn sync_buttons_to_joypad(&mut self) {
        let mask = self.button_mask;
        self.button_mask = mask.wrapping_add(1); // force apply on next call mismatch
        self.apply_button_mask(mask);
    }

    pub(super) fn bind_host_audio(&mut self) {
        let rate = self.audio.as_ref().map(|a| a.sample_rate);
        if let Some(Machine::Sm83 { bus, .. }) = self.machine.as_mut() {
            bind_bus_host_rate(bus, rate);
        }
    }

    pub(super) fn after_machine_restore(&mut self) {
        let rate = self.audio.as_ref().map(|a| a.sample_rate);
        if let Some(a) = &self.audio {
            a.after_restore();
        }
        match self.machine.as_mut() {
            Some(Machine::Sm83 { bus, .. }) => {
                bus.disable_boot_rom();
                bind_bus_host_rate(bus, rate);
                let _ = bus.apu.take_samples();
            }
            Some(Machine::Arm { .. }) | None => {}
        }
        self.sync_buttons_to_joypad();
    }

    pub(super) fn sync_rewind_ring(&mut self) {
        if !self.rewind_setting || self.machine.is_none() {
            self.rewind_ring = None;
            return;
        }
        if self.rewind_ring.is_none() {
            match self.machine.as_ref().unwrap() {
                Machine::Sm83 { cpu, bus, .. } => {
                    if !bus.boot_rom_active() {
                        let template = RewindFrame::Sm83(capture(cpu, bus));
                        self.rewind_ring = Some(RewindRing::new(template));
                    }
                }
                Machine::Arm { inner, .. } => {
                    let template = RewindFrame::Arm(inner.capture_state());
                    self.rewind_ring = Some(RewindRing::new(template));
                }
            }
        }
    }

    pub(super) fn reset_rewind_ring(&mut self) {
        if let Some(ring) = self.rewind_ring.as_mut() {
            ring.clear();
        }
        self.rewind_ring = None;
        self.sync_rewind_ring();
    }

    pub(super) fn apply_rewind_scrub(&mut self) {
        let Some(ring) = self.rewind_ring.as_mut() else {
            return;
        };
        ring.begin_scrub();
        let Some(state) = ring.scrub_back().cloned() else {
            return;
        };
        match (self.machine.as_mut(), state) {
            (Some(Machine::Sm83 { cpu, bus, .. }), RewindFrame::Sm83(s)) => {
                restore(&s, cpu, bus);
                self.after_machine_restore();
            }
            (Some(Machine::Arm { inner, .. }), RewindFrame::Arm(s)) => {
                inner.restore_state(&s);
                self.after_machine_restore();
            }
            _ => {}
        }
    }

    pub(super) fn apply_pending_slot_op(&mut self) {
        let op = match self.pending_slot.take() {
            Some(op) => op,
            None => return,
        };
        match self.machine.as_mut() {
            Some(Machine::Sm83 {
                path,
                title,
                cpu,
                bus,
                ..
            }) => match op {
                PendingSlotOp::Save(slot) => {
                    if bus.boot_rom_active() {
                        self.pending_slot = Some(PendingSlotOp::Save(slot));
                        return;
                    }
                    let state = capture(cpu, bus);
                    let rom = bus.cartridge.rom_bytes();
                    match save_slot(rom, path, title, slot, &state) {
                        Ok(()) => self.set_status_toast(format!("Saved slot {slot}")),
                        Err(e) => self.set_status_toast(format!("Save failed: {e}")),
                    }
                }
                PendingSlotOp::Load(slot) => {
                    let rom = bus.cartridge.rom_bytes();
                    match load_slot(rom, path, slot) {
                        Ok((state, _)) => {
                            restore(&state, cpu, bus);
                            self.after_machine_restore();
                            self.reset_rewind_ring();
                            self.set_status_toast(format!("Loaded slot {slot}"));
                        }
                        Err(e) => self.set_status_toast(format!("Load rejected: {e}")),
                    }
                }
            },
            Some(Machine::Arm {
                path, title, inner, ..
            }) => match op {
                PendingSlotOp::Save(slot) => {
                    let state = inner.capture_state();
                    let rom = inner.bus.rom.as_slice();
                    match save_arm_slot(rom, path, title, slot, &state) {
                        Ok(()) => self.set_status_toast(format!("Saved slot {slot}")),
                        Err(e) => self.set_status_toast(format!("Save failed: {e}")),
                    }
                }
                PendingSlotOp::Load(slot) => {
                    let rom = inner.bus.rom.clone();
                    match load_arm_slot(&rom, path, slot) {
                        Ok((state, _)) => {
                            inner.restore_state(&state);
                            self.after_machine_restore();
                            self.reset_rewind_ring();
                            self.set_status_toast(format!("Loaded slot {slot}"));
                        }
                        Err(e) => self.set_status_toast(format!("Load rejected: {e}")),
                    }
                }
            },
            None => {}
        }
    }

    pub(super) fn set_status_toast(&mut self, message: String) {
        if self.config.verbosity != Verbosity::Quiet {
            eprintln!("status: {message}");
        }
        self.status_toast = Some(message);
    }

    pub(super) fn take_status_toast(&mut self) -> Option<String> {
        self.status_toast.take()
    }

    pub(super) fn arm_debug_lines(inner: &crate::hw::Machine) -> Vec<String> {
        let frame = inner.frames_done().max(1);
        let debug = GbaMachineDebug::absent();
        let mut summary = debug.summary_lines(frame);
        summary[0] = live_cpu_line(
            frame,
            inner.cpu.exec_pc,
            inner.cpu.cpsr(),
            inner.idle,
            inner.bus.halted,
            inner.bus.irq.ime(),
            inner.bus.irq.ie(),
            inner.bus.irq.iff(),
        );
        let mode = inner.bus.dispcnt() & 7;
        let sprites = sprite_count(inner.bus.oam());
        summary[1] = format!("gba-debug: ppu frame={frame} mode={mode} sprites={sprites}");
        summary[2] = inner.bus.dma_debug_line(frame);
        summary[7] = inner.bus.apu.health_line(frame, &inner.bus.timers);
        summary.insert(
            0,
            format!("gba-debug: machine=arm7 a={AGB_A:02X} b={AGB_B:02X}"),
        );
        summary
    }
}
