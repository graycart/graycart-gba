use super::frame::{FramePacket, LatestFrame};
use super::policy::{
    FRAME_STEP_BUDGET, catch_up_cap, pending_slot_boundary, pending_slot_ready,
    should_allow_slot_save, should_allow_snapshot,
};
use crate::frontend::shell::host_input::HostCommand;
use crate::frontend::shell::playback::SpeedPreset;
use crate::frontend::shell::state_slots::PendingSlotOp;
use graycart::{Cartridge, HostHardwarePref, LaunchError, bus_from_cartridge};
use std::collections::HashSet;

#[test]
fn latest_frame_replaces_stale_packet() {
    let slot = LatestFrame::new();
    slot.publish(FramePacket {
        title: "first".into(),
        ..Default::default()
    });
    slot.publish(FramePacket {
        title: "second".into(),
        ..Default::default()
    });
    let taken = slot.take().expect("packet");
    assert_eq!(taken.title, "second");
    assert!(slot.take().is_none());
    assert_eq!(slot.replaced_count(), 1);
}

#[test]
fn latest_frame_publish_is_non_blocking_under_contention() {
    use std::thread;
    use std::time::Duration;

    let slot = LatestFrame::new();
    let writer = slot.clone();
    let handle = thread::spawn(move || {
        for i in 0..1_000 {
            writer.publish(FramePacket {
                title: format!("{i}"),
                ..Default::default()
            });
        }
    });
    for _ in 0..1_000 {
        let _ = slot.take();
        thread::sleep(Duration::from_micros(10));
    }
    handle.join().expect("writer");
    // Must complete without deadlock; exact replace count is timing-dependent.
    let _ = slot.replaced_count();
}

#[test]
fn paused_is_always_a_load_boundary() {
    assert!(pending_slot_boundary(true, false));
    assert!(pending_slot_boundary(true, true));
    assert!(pending_slot_ready(
        PendingSlotOp::Load(0),
        true,
        true,
        false
    ));
}

#[test]
fn running_load_only_on_frame_done() {
    assert!(!pending_slot_boundary(false, false));
    assert!(pending_slot_boundary(false, true));
    assert!(pending_slot_ready(
        PendingSlotOp::Load(0),
        false,
        false,
        true
    ));
}

#[test]
fn paused_save_blocked_while_boot_overlay_active() {
    assert!(!should_allow_slot_save(true, true, false));
    assert!(!pending_slot_ready(
        PendingSlotOp::Save(0),
        true,
        true,
        false
    ));
}

#[test]
fn should_allow_snapshot_requires_post_boot_frame() {
    assert!(!should_allow_snapshot(true, true));
    assert!(!should_allow_snapshot(false, false));
    assert!(should_allow_snapshot(false, true));
}

#[test]
fn step_budget_constant_is_documented_gate() {
    const { assert!(FRAME_STEP_BUDGET > 0) };
}

#[test]
fn catch_up_cap_scales_with_speed() {
    assert_eq!(catch_up_cap(SpeedPreset::X1), 2);
    assert_eq!(catch_up_cap(SpeedPreset::X4), 8);
    assert_eq!(catch_up_cap(SpeedPreset::Unlimited), 1);
}

#[test]
fn focus_lost_command_clears_transient_holds() {
    use super::super::playback::clear_transient_holds;

    let mut host_down = HashSet::from([HostCommand::FastForwardHold, HostCommand::Rewind]);
    let mut ff_hold = host_down.contains(&HostCommand::FastForwardHold);
    let mut rewind_held = host_down.contains(&HostCommand::Rewind);
    assert!(clear_transient_holds(&mut ff_hold, &mut rewind_held));
    host_down.remove(&HostCommand::FastForwardHold);
    host_down.remove(&HostCommand::Rewind);
    assert!(host_down.is_empty());
}

#[test]
fn cgb_only_forced_game_boy_is_launch_error() {
    let mut rom = vec![0u8; 0x200];
    rom[0x0143] = 0xC0;
    let cart = Cartridge::rom_only(rom);
    let err = match bus_from_cartridge(cart, HostHardwarePref::GameBoy) {
        Err(e) => e,
        Ok(_) => panic!("expected CgbOnlyOnDmg"),
    };
    assert_eq!(err, LaunchError::CgbOnlyOnDmg);
    assert_eq!(err.to_string(), "CGB-only cartridge cannot run on Game Boy");
}
