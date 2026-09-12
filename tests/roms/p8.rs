//! P8 Timing harness gates — mGBA timing + dma thresholds (honest).
//!
//! Cited: mgba-emu/suite (MIT) — timing / dma sub-suites
//!   https://github.com/mgba-emu/suite
//! Cited: graycart-gba PHASES P8 · 11/12 test docs
//!   Project store: `docs/graycart-gba/PHASES.md`
//! Note: unit gates (G8-*) live under `src/bus|cpu|irq|dma`. Keep
//! arm/thumb/memory green; never fake PASS.

use crate::harness::{fixture_path, run_test_rom, GbaTestRom, Outcome, RomLaunchMode};
use std::path::Path;

/// Agreed mGBA **timing** floor for P8 exit (passes). `0` = not run / absent ROM.
pub const MGBA_TIMING_THRESHOLD: u32 = 0;

/// Agreed mGBA **dma** floor for P8 exit (passes). `0` = not run / absent ROM.
pub const MGBA_DMA_THRESHOLD: u32 = 0;

/// Honest progress log for mGBA timing (G8-mgba-timing).
pub const MGBA_TIMING_PROGRESS: &str =
    "mGBA timing: not run — suite.gba absent; unit G8-prefetch/disable/irq-delay/cycles green";

/// Honest progress log for mGBA dma (G8-mgba-dma).
pub const MGBA_DMA_PROGRESS: &str =
    "mGBA dma: not run — suite.gba absent; unit G8-dma-delay green (raises P5 log)";

struct MgbaTimingRom;

impl GbaTestRom for MgbaTimingRom {
    fn id(&self) -> &str {
        "mgba-suite/timing"
    }

    fn relative_path(&self) -> &str {
        "mgba-suite/suite.gba"
    }

    fn launch_mode(&self) -> RomLaunchMode {
        RomLaunchMode::BiosHle
    }

    fn run(&self, _rom: &[u8]) -> Outcome {
        Outcome::Skipped("mGBA timing sub-suite needs suite.gba + headless automation (P8)".into())
    }
}

struct MgbaDmaRom;

impl GbaTestRom for MgbaDmaRom {
    fn id(&self) -> &str {
        "mgba-suite/dma"
    }

    fn relative_path(&self) -> &str {
        "mgba-suite/suite.gba"
    }

    fn launch_mode(&self) -> RomLaunchMode {
        RomLaunchMode::BiosHle
    }

    fn run(&self, _rom: &[u8]) -> Outcome {
        Outcome::Skipped("mGBA dma sub-suite needs suite.gba + headless automation (P8)".into())
    }
}

#[test]
fn mgba_timing_threshold_recorded() {
    assert_eq!(MGBA_TIMING_THRESHOLD, 0);
    assert!(MGBA_TIMING_PROGRESS.contains("timing") || MGBA_TIMING_PROGRESS.contains("Timing"));
    eprintln!("G8-mgba-timing: threshold={MGBA_TIMING_THRESHOLD} — {MGBA_TIMING_PROGRESS}");
}

#[test]
fn mgba_dma_threshold_recorded() {
    assert_eq!(MGBA_DMA_THRESHOLD, 0);
    assert!(MGBA_DMA_PROGRESS.contains("dma") || MGBA_DMA_PROGRESS.contains("DMA"));
    eprintln!("G8-mgba-dma: threshold={MGBA_DMA_THRESHOLD} — {MGBA_DMA_PROGRESS}");
}

#[test]
fn mgba_suite_path_hygiene() {
    let root = Path::new("tests/fixtures/mgba-suite");
    assert!(root.is_dir());
    assert!(root.join("LICENSE").is_file());
    assert!(root.join("README.md").is_file());
    assert!(!fixture_path("mgba-suite/suite.gba").is_file());
}

/// Opt-in: absent ROM → SKIPPED.
#[test]
#[ignore = "mGBA suite.gba + timing automation not vendored — G8-mgba-timing ROM gate"]
fn mgba_timing_matrix() {
    let outcome = run_test_rom(&MgbaTimingRom);
    eprintln!(
        "{:<40} {}  ({})",
        MgbaTimingRom.id(),
        outcome.label(),
        outcome.detail()
    );
    assert_eq!(outcome.label(), "SKIPPED");
}

#[test]
#[ignore = "mGBA suite.gba + dma automation not vendored — G8-mgba-dma ROM gate"]
fn mgba_dma_matrix() {
    let outcome = run_test_rom(&MgbaDmaRom);
    eprintln!(
        "{:<40} {}  ({})",
        MgbaDmaRom.id(),
        outcome.label(),
        outcome.detail()
    );
    assert_eq!(outcome.label(), "SKIPPED");
}

#[test]
#[ignore = "full mGBA suite board stretch — G8-stretch"]
fn mgba_full_suite_stretch() {
    assert!(
        !fixture_path("mgba-suite/suite.gba").is_file(),
        "suite present — wire full-board oracle before enabling"
    );
}
