//! ROM / conformance harnesses (often `#[ignore]` — apparatus stub).
//!
//! Cited: graycart-gb `tests/roms/main.rs` layout
//!   https://github.com/graycart/graycart-gb/blob/main/tests/roms/main.rs
//! Cited: graycart-gba audit-test-harness §4 first PR slice
//!   Project store: `internal/graycart-gba/audit-test-harness.md`
//! Note: default CI runs only non-ignored tests here; matrices stay opt-in.

mod harness;
mod jsmolka;

use harness::{run_test_rom, GbaTestRom, Outcome, RomLaunchMode};
use std::path::Path;

/// Tiny stand-in so CI proves the trait default is Skipped (no fixture I/O).
struct StubRom;

impl GbaTestRom for StubRom {
    fn id(&self) -> &str {
        "stub"
    }

    fn relative_path(&self) -> &str {
        "jsmolka/arm/arm.gba"
    }

    fn launch_mode(&self) -> RomLaunchMode {
        RomLaunchMode::BiosHle
    }
}

#[test]
fn harness_outcome_labels_include_skipped() {
    assert_eq!(Outcome::Pass.label(), "PASS");
    assert_eq!(Outcome::Fail("x".into()).label(), "FAIL");
    assert_eq!(Outcome::Timeout.label(), "TIMEOUT");
    assert_eq!(
        Outcome::Unsupported("no bios".into()).label(),
        "UNSUPPORTED"
    );
    assert_eq!(Outcome::Skipped("apparatus".into()).label(), "SKIPPED");
    assert_eq!(Outcome::Skipped("n".into()).detail(), "n");
    let _ = RomLaunchMode::BiosHle;
    let _ = RomLaunchMode::BiosLle;
    let _ = RomLaunchMode::Multiboot;
}

#[test]
fn runner_is_skipped_by_default() {
    let outcome = run_test_rom(&StubRom);
    assert_eq!(
        outcome.label(),
        "SKIPPED",
        "expected SKIPPED until cart load + oracle; got {} ({})",
        outcome.label(),
        outcome.detail()
    );
    assert!(!outcome.is_pass());
}

#[test]
fn fixtures_license_table_present() {
    let readme = Path::new("tests/fixtures/README.md");
    assert!(
        readme.is_file(),
        "missing {} — mandatory fixture license table",
        readme.display()
    );
    let text = std::fs::read_to_string(readme).expect("read fixtures README");
    assert!(
        text.contains("jsmolka") && text.contains("License"),
        "fixtures README should keep the upstream license table"
    );
    assert!(
        text.contains("tests/roms") || text.contains("roms/"),
        "fixtures README should point at the shared roms/ harness"
    );
}
