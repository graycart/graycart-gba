//! P5 DMA full harness gates — mGBA dma progress + alyosha stubs.
//!
//! Cited: mgba-emu/suite (MIT) — dma sub-suite
//!   https://github.com/mgba-emu/suite
//! Cited: alyosha-tas/gba-tests (MIT) — DMA_* edges (stretch)
//!   https://github.com/alyosha-tas/gba-tests
//! Cited: graycart-gba PHASES P5 · 11/12 test docs
//!   Project store: `docs/graycart-gba/PHASES.md`
//! Note: unit gates (G5-vblank/hblank/fifo/…) live under `src/dma/`.
//! Keep arm/thumb/memory green; never fake PASS.

use crate::harness::{fixture_path, run_test_rom, GbaTestRom, Outcome, RomLaunchMode};
use std::path::Path;

/// Honest progress log for mGBA dma (G5-mgba-dma). Update when suite automation lands.
pub const MGBA_DMA_PROGRESS: &str =
    "mGBA dma: not run — suite.gba absent; unit G5-* green on P5 branch";

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
        Outcome::Skipped(
            "mGBA dma sub-suite needs suite.gba + headless automation (P8 stretch toward must)"
                .into(),
        )
    }
}

#[test]
fn mgba_dma_progress_logged() {
    // CI-blocking hygiene: progress string is non-empty and mentions dma.
    assert!(MGBA_DMA_PROGRESS.contains("dma") || MGBA_DMA_PROGRESS.contains("DMA"));
    eprintln!("G5-mgba-dma: {MGBA_DMA_PROGRESS}");
}

#[test]
fn mgba_suite_dma_path_hygiene() {
    let root = Path::new("tests/fixtures/mgba-suite");
    assert!(root.is_dir());
    assert!(root.join("LICENSE").is_file());
    assert!(root.join("README.md").is_file());
    // Binary intentionally absent.
    assert!(!fixture_path("mgba-suite/suite.gba").is_file());
}

/// Opt-in: absent ROM → SKIPPED.
#[test]
#[ignore = "mGBA suite.gba + dma automation not vendored — G5-mgba-dma ROM gate"]
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
fn alyosha_dma_fixture_stub() {
    let dir = Path::new("tests/fixtures/alyosha");
    assert!(
        dir.is_dir(),
        "missing {dir:?} — create LICENSE/README stubs for G5-alyosha"
    );
    assert!(dir.join("README.md").is_file());
    assert!(dir.join("LICENSE").is_file());
}

/// Stretch: alyosha DMA_* ROMs when curated.
#[test]
#[ignore = "alyosha DMA_* binaries not curated — G5-alyosha stretch"]
fn alyosha_dma_stretch() {
    let path = fixture_path("alyosha/dma/DMA_demo.gba");
    assert!(
        !path.is_file(),
        "alyosha DMA ROM unexpectedly present — wire oracle before claiming PASS"
    );
}
