//! mGBA suite stubs — next SoC-depth gate (ignored until `suite.gba` + automation).
//!
//! Cited: mgba-emu/suite (MIT) — endrift accuracy board
//!   https://github.com/mgba-emu/suite
//! Cited: graycart-gba apparatus / gates (P5–P8 stretch → must)
//!   Project store: `docs/graycart-gba/11-test-apparatus.md`
//!   Project store: `docs/graycart-gba/12-test-gates.md`
//! Note: LICENSE/path hygiene is CI-blocking; ROM matrix stays `#[ignore]`.
//! No huge `suite.gba` until a pinned MIT prebuilt is deliberately vendored.

use crate::harness::{fixture_path, run_test_rom, GbaTestRom, Outcome, RomLaunchMode};
use std::path::Path;

/// Placeholder row for stock `suite.gba` (UI-driven upstream; automation TBD).
struct MgbaSuiteRom;

impl GbaTestRom for MgbaSuiteRom {
    fn id(&self) -> &str {
        "mgba-suite/suite"
    }

    fn relative_path(&self) -> &str {
        "mgba-suite/suite.gba"
    }

    fn launch_mode(&self) -> RomLaunchMode {
        RomLaunchMode::BiosHle
    }

    fn run(&self, _rom: &[u8]) -> Outcome {
        Outcome::Skipped("mGBA suite load+SRAM/debug oracle not wired (SoC-depth gate TBD)".into())
    }
}

#[test]
fn mgba_suite_license_and_path_stubs_present() {
    let root = Path::new("tests/fixtures/mgba-suite");
    assert!(root.is_dir(), "missing {}", root.display());
    let license = root.join("LICENSE");
    assert!(
        license.is_file(),
        "missing {} — keep upstream MIT LICENSE stub",
        license.display()
    );
    let text = std::fs::read_to_string(&license).expect("read LICENSE");
    assert!(
        text.contains("Jeffrey Pfau") || text.contains("MIT"),
        "mgba-suite LICENSE should retain upstream MIT attribution"
    );
    assert!(
        root.join("README.md").is_file(),
        "missing {}/README.md",
        root.display()
    );
    // Binary intentionally absent until pinned prebuilt / build script.
    assert!(
        !fixture_path("mgba-suite/suite.gba").is_file(),
        "suite.gba unexpectedly present — update ignored matrix + oracle before claiming gate"
    );
}

/// Opt-in SoC-depth row. Absent ROM → SKIPPED; never default-CI.
///
/// `cargo test -p graycart-gba --test roms -- --ignored --nocapture`
#[test]
#[ignore = "mGBA suite.gba + headless automation not vendored — next SoC-depth gate"]
fn mgba_suite_matrix() {
    let outcome = run_test_rom(&MgbaSuiteRom);
    eprintln!(
        "{:<40} {}  ({})",
        MgbaSuiteRom.id(),
        outcome.label(),
        outcome.detail()
    );
    assert_eq!(
        outcome.label(),
        "SKIPPED",
        "until suite.gba + oracle land, expect SKIPPED; got {} ({})",
        outcome.label(),
        outcome.detail()
    );
}
