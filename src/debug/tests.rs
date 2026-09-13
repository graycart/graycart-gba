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
    let cfg =
        DebugConfig::from_env_and_cli(Some(DebugLevel::Debug), false, None, Verbosity::Normal);
    assert_eq!(cfg.level, DebugLevel::Debug);
}

#[test]
fn cli_debug_trace_token_selects_trace() {
    let cfg =
        DebugConfig::from_env_and_cli(Some(DebugLevel::Trace), false, None, Verbosity::Normal);
    assert_eq!(cfg.level, DebugLevel::Trace);
}

#[test]
fn parse_debug_arg_accepts_summary_and_trace() {
    assert_eq!(parse_debug_arg("--debug"), Some(Ok(DebugLevel::Debug)));
    assert_eq!(
        parse_debug_arg("--debug=summary"),
        Some(Ok(DebugLevel::Debug))
    );
    assert_eq!(
        parse_debug_arg("--debug=trace"),
        Some(Ok(DebugLevel::Trace))
    );
    assert!(parse_debug_arg("--frames").is_none());
    assert!(parse_debug_arg("--debug=nope").unwrap().is_err());
}

#[test]
fn cli_trace_steps_selects_trace_level() {
    let cfg = DebugConfig::from_env_and_cli(None, false, Some(3), Verbosity::Quiet);
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

#[test]
fn unhandled_swi_is_loud_at_summary() {
    capture_start();
    let mut gba = Gba::new();
    gba.debug.set_config(DebugConfig {
        level: DebugLevel::Debug,
        period_frames: 10_000,
        stuck_frames: 10_000,
        ..DebugConfig::default()
    });
    gba.debug.on_unhandled_swi(0x12, 0x0800_1234);
    gba.debug.on_unhandled_swi(0x12, 0x0800_1238);
    let lines = capture_take();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("warn swi unhandled") && l.contains("0x12")),
        "expected loud SWI warn, got {lines:?}"
    );
    // First + early repeats; not buried / suppressed at summary.
    assert!(
        lines.iter().filter(|l| l.contains("swi unhandled")).count() >= 2,
        "expected early SWI repeats visible, got {lines:?}"
    );
}

#[test]
fn summary_dma_does_not_emit_per_event_lines() {
    use crate::dma::{ChannelId, Dma, DmaRunReport};

    capture_start();
    let mut tracker = DebugTracker::new(DebugConfig {
        level: DebugLevel::Debug,
        ..DebugConfig::default()
    });
    let dma = Dma::new();
    let report = DmaRunReport {
        completed: vec![ChannelId::Ch1, ChannelId::Ch1, ChannelId::Ch1],
        ..DmaRunReport::default()
    };
    tracker.on_dma_report(&report, &dma);
    // Force a period summary without per-event spam.
    tracker.frames = 60;
    tracker.last_period_frame = 0;
    tracker.log_dma_summary();
    let lines = capture_take();
    assert!(
        !lines.iter().any(|l| l.contains("dma ch=")),
        "summary must not spam per-DMA lines, got {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("dma summary") && l.contains("total=3")),
        "expected aggregated dma summary, got {lines:?}"
    );
}

#[test]
fn trace_dma_emits_per_event_lines() {
    use crate::dma::{ChannelId, Dma, DmaRunReport};

    capture_start();
    let mut tracker = DebugTracker::new(DebugConfig {
        level: DebugLevel::Trace,
        ..DebugConfig::default()
    });
    let dma = Dma::new();
    let report = DmaRunReport {
        completed: vec![ChannelId::Ch1],
        ..DmaRunReport::default()
    };
    tracker.on_dma_report(&report, &dma);
    let lines = capture_take();
    assert!(
        lines.iter().any(|l| l.contains("dma ch=")),
        "trace should emit per-DMA lines, got {lines:?}"
    );
}

#[test]
fn openbus_pc_is_loud() {
    use crate::cpu::pipeline::IsaState;

    capture_start();
    let mut gba = Gba::new();
    gba.debug.set_config(DebugConfig {
        level: DebugLevel::Debug,
        period_frames: 10_000,
        stuck_frames: 10_000,
        ..DebugConfig::default()
    });
    let mut rom = vec![0u8; 0x200];
    rom[0..4].copy_from_slice(&0xEAFF_FFFEu32.to_le_bytes());
    gba.load_rom(&rom);
    gba.reset_bios_hle();
    // Jump into unused-low open bus (classic empty-BIOS runaway band).
    gba.cpu.pipeline.redirect(0x0011_2328, IsaState::Arm);
    gba.cpu.regs.set_pc(0x0011_2328);
    for _ in 0..8 {
        gba.step_instruction();
    }
    let lines = capture_take();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("warn openbus") && l.contains("UnusedLow")),
        "expected openbus warn, got {lines:?}"
    );
}

#[test]
fn forced_blank_edge_is_logged() {
    capture_start();
    let mut gba = Gba::new();
    gba.debug.set_config(DebugConfig {
        level: DebugLevel::Debug,
        period_frames: 10_000,
        stuck_frames: 10_000,
        ..DebugConfig::default()
    });
    let mut rom = vec![0u8; 0x200];
    rom[0..4].copy_from_slice(&0xEAFF_FFFEu32.to_le_bytes());
    gba.load_rom(&rom);
    gba.reset_bios_hle();
    // Seed last_forced_blank via one frame, then flip DISPCNT bit 7 (forced blank).
    gba.run_frames(1);
    gba.ppu.regs.dispcnt |= 1 << 7;
    gba.run_frames(1);
    let lines = capture_take();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("ppu blank") && l.contains("forced_blank=true")),
        "expected blank edge, got {lines:?}"
    );
}

#[test]
fn fifo_underrun_surfaces_in_apu_health_summary() {
    use crate::apu::{MASTER_ENABLE, OFF_SOUNDCNT_H, OFF_SOUNDCNT_X};

    capture_start();
    let mut gba = Gba::new();
    gba.debug.set_config(DebugConfig {
        level: DebugLevel::Debug,
        period_frames: 1,
        stuck_frames: 10_000,
        ..DebugConfig::default()
    });
    // Master on + FIFO A → L+R; never push samples → empty drains on timer.
    gba.apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE);
    gba.apu.write16(OFF_SOUNDCNT_H, 0x0300); // A left+right, TM0
    for _ in 0..200 {
        gba.apu.on_timer_overflows(1, 0);
        gba.apu.step(512);
    }
    let mut dbg = std::mem::take(&mut gba.debug);
    dbg.frames = 0;
    dbg.last_period_frame = 0;
    dbg.on_step(
        &mut gba,
        u64::from(crate::ppu::FRAME_CYCLES),
        crate::cpu::StepOutcome::Ok,
    );
    gba.debug = dbg;
    let lines = capture_take();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("apu health") && l.contains("underrun=")),
        "expected apu health summary, got {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("warn apu fifo") || l.contains("empty=")),
        "expected fifo empty/underrun signal, got {lines:?}"
    );
}

#[test]
fn ppu_health_summary_includes_layers_and_writes() {
    use crate::bus::CpuMem;

    capture_start();
    let mut gba = Gba::new();
    gba.debug.set_config(DebugConfig {
        level: DebugLevel::Debug,
        period_frames: 1,
        stuck_frames: 10_000,
        ..DebugConfig::default()
    });
    gba.ppu.regs.dispcnt = 0x1100; // mode 0, BG0+OBJ
    for i in 0..100u32 {
        gba.bus.write16(0x0600_0000 + i * 2, 0x1234);
    }
    let mut dbg = std::mem::take(&mut gba.debug);
    dbg.frames = 0;
    dbg.last_period_frame = 0;
    dbg.on_step(
        &mut gba,
        u64::from(crate::ppu::FRAME_CYCLES),
        crate::cpu::StepOutcome::Ok,
    );
    gba.debug = dbg;
    let lines = capture_take();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("ppu health") && l.contains("layers=BG0|OBJ")),
        "expected ppu health with layers, got {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("writes vram=") && l.contains("vram=100")),
        "expected vram write count in ppu health, got {lines:?}"
    );
}

#[test]
fn format_av_report_mentions_apu_and_ppu() {
    let gba = Gba::new();
    let report = format_av_report(&gba, 0);
    assert!(report.contains("AV report"));
    assert!(report.contains("PPU"));
    assert!(report.contains("APU"));
}

#[test]
fn format_av_report_includes_unhandled_swi_counts() {
    let mut gba = Gba::new();
    gba.debug.set_config(DebugConfig {
        level: DebugLevel::Debug,
        ..DebugConfig::default()
    });
    gba.debug.on_unhandled_swi(0x0F, 0x0800_1000);
    gba.debug.on_unhandled_swi(0x0F, 0x0800_1004);
    gba.debug.on_unhandled_swi(0x1F, 0x0800_2000);
    let report = format_av_report(&gba, 42);
    assert!(
        report.contains("SWI") && report.contains("0x0F:2") && report.contains("0x1F:1"),
        "expected SWI counts in final AV report, got {report}"
    );
}

#[test]
fn dispcnt_mode_flip_is_logged() {
    capture_start();
    let mut gba = Gba::new();
    gba.debug.set_config(DebugConfig {
        level: DebugLevel::Debug,
        period_frames: 10_000,
        stuck_frames: 10_000,
        ..DebugConfig::default()
    });
    let mut rom = vec![0u8; 0x200];
    rom[0..4].copy_from_slice(&0xEAFF_FFFEu32.to_le_bytes());
    gba.load_rom(&rom);
    gba.reset_bios_hle();
    gba.run_frames(1);
    gba.ppu.regs.dispcnt = (gba.ppu.regs.dispcnt & !0x7) | 3; // mode 3
    gba.run_frames(1);
    let lines = capture_take();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("ppu mode") && l.contains("→3")),
        "expected mode flip, got {lines:?}"
    );
}
