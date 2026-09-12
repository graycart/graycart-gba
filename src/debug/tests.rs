//! Unit tests for console debug config / breadcrumbs (legal fixtures only).

use super::*;
use crate::Gba;

#[test]
fn default_config_is_quiet() {
    let cfg = DebugConfig::default();
    assert_eq!(cfg.level, DebugLevel::Off);
    assert!(!cfg.enabled());
}

#[test]
fn cli_debug_enables_without_env() {
    let cfg = DebugConfig::from_env_and_cli(true, false, None, Verbosity::Normal);
    assert_eq!(cfg.level, DebugLevel::Debug);
}

#[test]
fn cli_trace_steps_selects_trace_level() {
    let cfg = DebugConfig::from_env_and_cli(false, false, Some(3), Verbosity::Quiet);
    assert_eq!(cfg.level, DebugLevel::Trace);
    assert_eq!(cfg.trace_steps, Some(3));
}

#[test]
fn rom_load_emits_breadcrumb_when_debug_on() {
    capture_start();
    let mut gba = Gba::new();
    gba.debug.set_config(DebugConfig {
        level: DebugLevel::Debug,
        ..DebugConfig::default()
    });
    // Minimal header-shaped image (entry + title region + fixed 0x96).
    let mut rom = vec![0u8; 0x200];
    rom[0..4].copy_from_slice(&0xEA00_0000u32.to_le_bytes()); // B +0
    rom[0xA0..0xAC].copy_from_slice(b"SYNTHTEST\0\0\0");
    rom[0xAC..0xB0].copy_from_slice(b"TEST");
    rom[0xB2] = 0x96;
    gba.load_rom(&rom);
    let lines = capture_take();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("rom") && l.contains("SYNTHTEST")),
        "expected rom breadcrumb, got {lines:?}"
    );
}

#[test]
fn reset_emits_bios_line() {
    capture_start();
    let mut gba = Gba::new();
    gba.debug.set_config(DebugConfig {
        level: DebugLevel::Debug,
        period_frames: 1,
        stuck_frames: 10_000,
        ..DebugConfig::default()
    });
    let mut rom = vec![0u8; 0x200];
    rom[0..4].copy_from_slice(&0xEAFF_FFFEu32.to_le_bytes()); // B .
    gba.load_rom(&rom);
    gba.reset_bios_hle();
    let lines = capture_take();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("bios") && l.contains("BiosHle")),
        "expected bios breadcrumb, got {lines:?}"
    );
}

#[test]
fn debug_off_emits_nothing_on_load() {
    capture_start();
    let mut gba = Gba::new();
    assert!(!gba.debug.enabled());
    gba.load_rom(&[0u8; 0x100]);
    let lines = capture_take();
    assert!(lines.is_empty(), "quiet default leaked: {lines:?}");
}

#[test]
fn stuck_detection_fires_on_tight_loop() {
    capture_start();
    let mut gba = Gba::new();
    gba.debug.set_config(DebugConfig {
        level: DebugLevel::Debug,
        period_frames: 10_000,
        stuck_frames: 2,
        ..DebugConfig::default()
    });
    let mut rom = vec![0u8; 0x200];
    // ARM B . at cart entry — same PC forever.
    rom[0..4].copy_from_slice(&0xEAFF_FFFEu32.to_le_bytes());
    gba.load_rom(&rom);
    gba.reset_bios_hle();
    // Need pipeline refill before decode PC is stable.
    for _ in 0..8 {
        gba.step_instruction();
    }
    gba.run_frames(4);
    let lines = capture_take();
    assert!(
        lines.iter().any(|l| l.contains("stuck")),
        "expected stuck breadcrumb, got {lines:?}"
    );
}
