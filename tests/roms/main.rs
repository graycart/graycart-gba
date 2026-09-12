//! ROM / conformance harnesses (jsmolka gates in default CI; SoC-depth `#[ignore]`).
//!
//! Cited: graycart-gb `tests/roms/main.rs` layout
//!   https://github.com/graycart/graycart-gb/blob/main/tests/roms/main.rs
//! Cited: graycart-gba audit-test-harness §4 / workstream D–E
//!   Project store: `internal/graycart-gba/audit-test-harness.md`
//! Note: default CI asserts jsmolka arm+thumb+memory PASS; mGBA/NBA rows stay ignored.

mod harness;
mod jsmolka;
mod mgba_suite;
mod nba_hw_test;
mod p3;
mod p4;
mod p5;
mod p6;

use harness::{
    jsmolka_r12_oracle, load_gba, run_test_rom, GbaTestRom, Outcome, RomLaunchMode, RunBudget,
};
use std::path::Path;

/// Tiny stand-in so CI proves the trait default is still Skipped.
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
fn runner_default_trait_is_skipped() {
    let outcome = run_test_rom(&StubRom);
    assert_eq!(
        outcome.label(),
        "SKIPPED",
        "unimplemented suites stay Skipped; got {} ({})",
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

#[test]
fn jsmolka_r12_oracle_pass_and_fail() {
    let mut gba = graycart_gba::Gba::new();
    gba.reset_bios_hle();
    assert_eq!(jsmolka_r12_oracle(&gba).label(), "PASS");
    gba.cpu.regs.set(12, 42);
    let fail = jsmolka_r12_oracle(&gba);
    assert_eq!(fail.label(), "FAIL");
    assert!(fail.detail().contains("42"));
}

#[test]
fn load_gba_bios_hle_maps_rom() {
    let bytes = std::fs::read("tests/fixtures/jsmolka/thumb/thumb.gba").expect("thumb.gba");
    let gba = load_gba(&bytes, RomLaunchMode::BiosHle).expect("BiosHle load");
    assert!(!gba.bus.rom.is_empty());
    assert_eq!(gba.cpu.regs.pc(), 0x0800_0000);
    let _ = RunBudget::default();
}
