//! Pure playback / slot-boundary policy shared by the emulation thread and tests.

use super::super::playback::SpeedPreset;
use super::super::state_slots::PendingSlotOp;

/// Per wake cap: high enough for a normal frame at release pacing,
/// low enough that boot firmware with LCD permanently off cannot freeze the UI.
pub const FRAME_STEP_BUDGET: u32 = 100_000;

/// Brief HUD flash after a successful frame advance while paused.
pub const FRAME_ADVANCE_FLASH: std::time::Duration = std::time::Duration::from_millis(400);

/// Max emulated frames to run in one scheduler wake (avoids death spirals).
pub const CATCH_UP_CAP: u32 = 16;

/// Deferred load runs at frame end while running, or on every tick while paused.
pub fn pending_slot_boundary(paused: bool, frame_done: bool) -> bool {
    paused || frame_done
}

pub fn should_allow_snapshot(boot_rom_active: bool, frame_ready: bool) -> bool {
    frame_ready && !boot_rom_active
}

pub fn should_allow_slot_save(boot_rom_active: bool, paused: bool, frame_done: bool) -> bool {
    should_allow_snapshot(boot_rom_active, paused || frame_done)
}

pub fn pending_slot_ready(
    op: PendingSlotOp,
    boot_rom_active: bool,
    paused: bool,
    frame_done: bool,
) -> bool {
    match op {
        PendingSlotOp::Save(_) => should_allow_slot_save(boot_rom_active, paused, frame_done),
        PendingSlotOp::Load(_) => pending_slot_boundary(paused, frame_done),
    }
}

pub fn catch_up_cap(speed: SpeedPreset) -> u32 {
    speed
        .multiplier()
        .map(|n| (n * 2).clamp(1, CATCH_UP_CAP))
        .unwrap_or(1)
}
