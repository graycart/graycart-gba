//! NBA hw-test stubs — stretch SoC-depth corpus (ignored; LICENSE/path only).
//!
//! Cited: nba-emu/hw-test (BSD-3-Clause) — Codeberg is the active mirror
//!   https://codeberg.org/nba-emu/hw-test
//!   https://github.com/nba-emu/hw-test
//! Cited: graycart-gba apparatus §1.2 secondary / gates stretch
//!   Project store: `docs/graycart-gba/11-test-apparatus.md`
//!   Project store: `docs/graycart-gba/12-test-gates.md`
//! Note: no binaries unless a small curated BSD prebuilt is pinned later.

use crate::harness::{fixture_path, run_test_rom, GbaTestRom, Outcome, RomLaunchMode};
use std::path::Path;

/// Placeholder identity until a curated ROM path is chosen.
struct NbaHwTestRom;

impl GbaTestRom for NbaHwTestRom {
    fn id(&self) -> &str {
        "nba-hw-test/curated"
    }

    fn relative_path(&self) -> &str {
        // Path reserved; no file until curated. try_load_rom → SKIPPED.
        "nba-hw-test/curated.gba"
    }

    fn launch_mode(&self) -> RomLaunchMode {
        RomLaunchMode::BiosHle
    }

    fn run(&self, _rom: &[u8]) -> Outcome {
        Outcome::Skipped("NBA hw-test oracle not wired (stretch after mGBA suite)".into())
    }
}

#[test]
fn nba_hw_test_license_and_path_stubs_present() {
    let root = Path::new("tests/fixtures/nba-hw-test");
    assert!(root.is_dir(), "missing {}", root.display());
    let license = root.join("LICENSE");
    assert!(
        license.is_file(),
        "missing {} — keep upstream BSD-3-Clause LICENSE stub",
        license.display()
    );
    let text = std::fs::read_to_string(&license).expect("read LICENSE");
    assert!(
        text.contains("fleroviux") || text.contains("BSD"),
        "nba-hw-test LICENSE should retain upstream BSD-3 attribution"
    );
    assert!(
        root.join("README.md").is_file(),
        "missing {}/README.md",
        root.display()
    );
    assert!(
        !fixture_path("nba-hw-test/curated.gba").is_file(),
        "curated.gba unexpectedly present — wire oracle before claiming stretch gate"
    );
}

/// Opt-in stretch row. Absent ROM → SKIPPED via [`run_test_rom`].
///
/// `cargo test -p graycart-gba --test roms -- --ignored --nocapture`
#[test]
#[ignore = "NBA hw-test curated ROM not vendored — stretch SoC-depth after mGBA"]
fn nba_hw_test_matrix() {
    let outcome = run_test_rom(&NbaHwTestRom);
    eprintln!(
        "{:<40} {}  ({})",
        NbaHwTestRom.id(),
        outcome.label(),
        outcome.detail()
    );
    assert_eq!(
        outcome.label(),
        "SKIPPED",
        "until curated ROM lands, expect SKIPPED; got {} ({})",
        outcome.label(),
        outcome.detail()
    );
}
