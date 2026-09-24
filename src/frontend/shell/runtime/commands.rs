//! Commands sent from the UI thread to the emulation thread.

use super::super::audio::AudioDevicePref;
use super::super::host_input::HostCommand;
use super::super::playback::SpeedPreset;
use super::super::rom::OpenedRom;
use graycart::{BootMode, HostHardwarePref};
use std::sync::mpsc;

/// One-way messages into the emulation thread.
#[allow(clippy::large_enum_variant)]
pub enum EmuCommand {
    /// Stop the thread after optional save flush.
    Quit,
    /// Begin stepping; sent once after the window is ready.
    ///
    /// Host audio is opened on this thread because `cpal::Stream` is not
    /// `Send`. PCM still goes emu producer → ring → CPAL callback (not winit).
    Start,
    /// Construct a new machine from an opened ROM (UI reads the file).
    LoadRom(OpenedRom),
    /// Power-on reset with the given boot mode (SM83 only; ARM ignores boot firmware).
    Reset {
        boot_mode: BootMode,
    },
    /// Effective button bitmask: GBA KEYINPUT bits in the low 10, or Game Boy bits in the low 8.
    SetButtons(u16),
    HostPress(HostCommand),
    HostRelease(HostCommand),
    /// Main-window focus loss: clear transient holds; optionally pause.
    FocusLost {
        pause_when_unfocused: bool,
    },
    SetPaused(bool),
    SetFfSpeed(SpeedPreset),
    SetFfToggle(bool),
    SetRewindEnabled(bool),
    SetAudioGain(f32),
    SetTickProfiling(bool),
    /// When true, the runtime periodically publishes debug into the latest-debug mailbox.
    SetDebugPublish(bool),
    SlotSave(u8),
    SlotLoad(u8),
    FrameAdvance,
    /// Flush battery `.sav` to disk; reply when done (shutdown / rare UI path only).
    FlushSave {
        reply: mpsc::Sender<Result<(), String>>,
    },
    /// Host hardware preference. Reloads the current SM83 ROM; ignored for ARM.
    SetHardwarePref(HostHardwarePref),
    /// Re-open the CPAL stream for a new output preference (emu thread owns Stream).
    SetAudioOutput(AudioDevicePref),
}
