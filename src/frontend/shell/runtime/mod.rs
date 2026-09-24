//! Dedicated emulation thread — sole owner of Machine (CPU/Bus).

mod commands;
pub mod emu_thread;
pub mod frame;
mod policy;
#[cfg(test)]
mod tests;

pub use commands::EmuCommand;
pub use frame::{FramePacket, HostDebug, LatestDebug, LatestFrame, PresentFrame};

use super::launch::{LaunchRom, Verbosity};
use super::playback::SpeedPreset;
use graycart::{BootMode, HostHardwarePref};
use std::sync::mpsc::{self, Sender};
use std::thread::{self as std_thread, JoinHandle};

/// Host-side configuration passed into the emulation thread at spawn.
pub struct RuntimeConfig {
    pub unthrottled: bool,
    pub pacer_enabled: bool,
    #[allow(dead_code)] // consumed by UI renderer; retained for host config symmetry
    pub vsync: bool,
    pub verbosity: Verbosity,
    pub boot_mode: BootMode,
    pub ff_speed: SpeedPreset,
    pub rewind_enabled: bool,
    pub audio_gain: f32,
    pub audio_output: crate::frontend::shell::audio::AudioDevicePref,
    pub hardware_pref: HostHardwarePref,
    pub initial_rom: Option<LaunchRom>,
}

/// UI-thread handle: send commands in, take latest frames / debug out.
pub struct EmulationRuntime {
    cmd_tx: Sender<EmuCommand>,
    latest: LatestFrame,
    latest_debug: LatestDebug,
    join: Option<JoinHandle<()>>,
}

impl EmulationRuntime {
    pub fn spawn(config: RuntimeConfig) -> Self {
        let latest = LatestFrame::new();
        let latest_debug = LatestDebug::new();
        let latest_for_thread = latest.clone();
        let debug_for_thread = latest_debug.clone();
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let join = std_thread::Builder::new()
            .name("graycart-emu".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                emu_thread::emu_loop(config, cmd_rx, latest_for_thread, debug_for_thread)
            })
            .expect("spawn emulation thread");
        Self {
            cmd_tx,
            latest,
            latest_debug,
            join: Some(join),
        }
    }

    pub fn send(&self, cmd: EmuCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn take_frame(&self) -> Option<FramePacket> {
        self.latest.take()
    }

    /// Non-blocking: returns the newest unread debug snapshot, if any.
    pub fn take_debug_snapshot(&self) -> Option<HostDebug> {
        self.latest_debug.take()
    }

    pub fn flush_save(&self) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.send(EmuCommand::FlushSave { reply: tx });
        rx.recv().unwrap_or(Ok(()))
    }

    pub fn shutdown(mut self) -> std_thread::Result<()> {
        self.send(EmuCommand::Quit);
        if let Some(j) = self.join.take() {
            j.join()
        } else {
            Ok(())
        }
    }
}
