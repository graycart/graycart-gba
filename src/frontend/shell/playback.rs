//! Host playback helpers (speed presets, stepping, focus policy). Core timing unchanged.

use super::audio::AudioOut;
use super::host_input::HostCommand;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedPreset {
    X1,
    X2,
    X3,
    #[default]
    X4,
    X8,
    Unlimited,
}

impl SpeedPreset {
    pub const ALL: [Self; 6] = [
        Self::X1,
        Self::X2,
        Self::X3,
        Self::X4,
        Self::X8,
        Self::Unlimited,
    ];

    pub fn multiplier(self) -> Option<u32> {
        match self {
            Self::X1 => Some(1),
            Self::X2 => Some(2),
            Self::X3 => Some(3),
            Self::X4 => Some(4),
            Self::X8 => Some(8),
            Self::Unlimited => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::X1 => "1×",
            Self::X2 => "2×",
            Self::X3 => "3×",
            Self::X4 => "4×",
            Self::X8 => "8×",
            Self::Unlimited => "MAX",
        }
    }
}

/// Result of [`run_until_frame_or_budget`]: one emulated frame completed, or step cap hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepPulse {
    FrameReady,
    BudgetExhausted,
}

/// Effective host pacing speed from pause / rewind / FF hold / toggle and the FF target preset.
///
/// Pause and rewind return [`SpeedPreset::X1`] for pacing math; callers handle those paths
/// separately (no catch-up while paused; rewind uses its own scrub loop).
pub fn effective_speed(
    paused: bool,
    rewinding: bool,
    ff_hold: bool,
    ff_toggle: bool,
    target: SpeedPreset,
) -> SpeedPreset {
    if paused || rewinding {
        SpeedPreset::X1
    } else if ff_hold || ff_toggle {
        target
    } else {
        SpeedPreset::X1
    }
}

/// Clear level-triggered host holds on main-window focus loss.
///
/// Returns `true` if either hold was active and is now cleared. Does not affect toggle FF.
pub fn clear_transient_holds(ff_hold: &mut bool, rewind_held: &mut bool) -> bool {
    let had_any = *ff_hold || *rewind_held;
    *ff_hold = false;
    *rewind_held = false;
    had_any
}

/// Step emulation until a frame completes or `budget` CPU steps are consumed.
///
/// Shared by 1× / N× catch-up, frame advance, and boot continuation paths.
/// `step_once` runs one CPU step and returns `true` when a PPU frame completed.
pub fn run_until_frame_or_budget(step_once: &mut dyn FnMut() -> bool, budget: u32) -> StepPulse {
    for _ in 0..budget {
        if step_once() {
            return StepPulse::FrameReady;
        }
    }
    StepPulse::BudgetExhausted
}

/// Toggle pause; entering pause clears transient FF hold from `host_down`.
pub fn toggle_paused(paused: &mut bool, host_down: &mut std::collections::HashSet<HostCommand>) {
    *paused = !*paused;
    if *paused {
        host_down.remove(&HostCommand::FastForwardHold);
    }
}

/// Main-window focus loss: clear level holds; optionally promote into manual pause.
#[allow(dead_code)] // policy helper; emu thread handles FocusLost via [`EmuCommand`]
pub fn on_main_window_focus_loss(
    host_down: &mut std::collections::HashSet<HostCommand>,
    pause_when_unfocused: bool,
    paused: &mut bool,
    end_rewind_scrub: &mut dyn FnMut(),
) {
    let mut ff_hold = host_down.contains(&HostCommand::FastForwardHold);
    let mut rewind_held = host_down.contains(&HostCommand::Rewind);
    if clear_transient_holds(&mut ff_hold, &mut rewind_held) {
        host_down.remove(&HostCommand::FastForwardHold);
        host_down.remove(&HostCommand::Rewind);
        end_rewind_scrub();
    }
    if pause_when_unfocused {
        *paused = true;
    }
}

/// Whether host PCM should be submitted at 1× gain (normal play only).
///
/// Mute during pause, rewind scrub, and any effective speed above 1× (including Unlimited).
pub fn should_submit_audio(speed: SpeedPreset, rewinding: bool, paused: bool) -> bool {
    !paused && !rewinding && speed == SpeedPreset::X1
}

/// Whether a playback-mode change requires flushing the audio ring and re-priming silence.
///
/// Covers 1×↔FF/Unlimited transitions and rewind enter/exit per 9K spec; pause changes do not flush.
pub fn audio_transition_needs_flush(
    prev_speed: SpeedPreset,
    prev_rewinding: bool,
    next_speed: SpeedPreset,
    next_rewinding: bool,
) -> bool {
    let prev_turbo = prev_speed != SpeedPreset::X1;
    let next_turbo = next_speed != SpeedPreset::X1;
    prev_turbo != next_turbo || prev_rewinding != next_rewinding
}

/// Drop stale ring PCM and re-prime silence after a mode transition (delegates to [`AudioOut::after_restore`]).
pub fn flush_and_reprime_audio(audio: &AudioOut) {
    audio.after_restore();
}

/// Apply flush/re-prime when [`audio_transition_needs_flush`] is true.
pub fn apply_audio_transition_if_needed(
    audio: Option<&AudioOut>,
    prev_speed: SpeedPreset,
    prev_rewinding: bool,
    next_speed: SpeedPreset,
    next_rewinding: bool,
) {
    if audio_transition_needs_flush(prev_speed, prev_rewinding, next_speed, next_rewinding)
        && let Some(a) = audio
    {
        flush_and_reprime_audio(a);
    }
}

#[cfg(test)]
mod tests;
