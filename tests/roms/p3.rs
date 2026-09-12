//! P3 timers / IRQ / input harness gates (wired early — may be ignored / known-red).
//!
//! Cited: graycart-gba PHASES P3 · 12-test-gates.md · 11-test-apparatus.md
//!   Project store: `docs/graycart-gba/PHASES.md`
//!   Project store: `docs/graycart-gba/12-test-gates.md`
//!   Project store: `docs/graycart-gba/11-test-apparatus.md`
//! Note: do not claim P3 PASS from unit smoke alone; keep arm/thumb/memory green.

use crate::harness::{fixture_path, run_test_rom, GbaTestRom, Outcome, RomLaunchMode, RunBudget};
use std::path::Path;

/// In-house simple IRQ ROM (G3-irq-rom) — binary optional until authored.
struct SimpleIrqRom;

impl GbaTestRom for SimpleIrqRom {
    fn id(&self) -> &str {
        "inhouse/simple-irq"
    }

    fn relative_path(&self) -> &str {
        "inhouse/simple-irq/simple-irq.gba"
    }

    fn launch_mode(&self) -> RomLaunchMode {
        RomLaunchMode::BiosHle
    }

    fn run(&self, _rom: &[u8]) -> Outcome {
        // Placeholder until IRQ + Halt paths can complete a real fixture.
        Outcome::Skipped("P3 simple IRQ ROM runner not implemented yet".into())
    }
}

/// Default CI: fixture path + LICENSE stubs exist (no accuracy claim).
#[test]
fn p3_irq_fixture_stubs_present() {
    let dir = Path::new("tests/fixtures/inhouse/simple-irq");
    assert!(dir.is_dir(), "missing {}", dir.display());
    assert!(
        dir.join("README.md").is_file(),
        "missing simple-irq/README.md"
    );
    assert!(
        Path::new("tests/fixtures/inhouse/LICENSE").is_file(),
        "missing inhouse/LICENSE"
    );
    for stretch in ["io-read", "timer-irq"] {
        let p = Path::new("tests/fixtures/mgba-suite").join(stretch);
        assert!(p.is_dir(), "missing stretch stub {}", p.display());
        assert!(
            p.join("README.md").is_file(),
            "missing {}/README.md",
            p.display()
        );
    }
}

/// G3-irq-rom: ignored until `simple-irq.gba` is vendored (honest gate, not fake green).
///
/// `cargo test -p graycart-gba --test roms -- --ignored p3_simple_irq`
#[test]
#[ignore = "P3 G3-irq-rom: simple-irq.gba not present yet — wire stays; do not undraft ignore until binary lands"]
fn p3_simple_irq_rom_gate() {
    let gba_path = fixture_path(SimpleIrqRom.relative_path());
    assert!(
        gba_path.is_file(),
        "missing {} — remove #[ignore] only after vendoring the ROM",
        gba_path.display()
    );
    let outcome = run_test_rom(&SimpleIrqRom);
    assert_ne!(
        outcome.label(),
        "SKIPPED",
        "ROM present but runner still SKIPPED — implement BiosHle IRQ oracle"
    );
    assert_eq!(
        outcome.label(),
        "PASS",
        "G3-irq-rom expected PASS, got {} ({})",
        outcome.label(),
        outcome.detail()
    );
    let _ = RunBudget::default();
}

/// Stretch: mGBA io-read progress (ignored; no binary in-tree).
#[test]
#[ignore = "P3 stretch: mGBA io-read — fixture not vendored; progress log only"]
fn p3_mgba_io_read_stretch() {
    let p = Path::new("tests/fixtures/mgba-suite/io-read");
    assert!(p.is_dir());
    // When suite automation exists, replace with real PASS/FAIL scrape.
    panic!("mGBA io-read stretch not automated yet");
}

/// Stretch: mGBA timer-irq progress (ignored; no binary in-tree).
#[test]
#[ignore = "P3 stretch: mGBA timer-irq — fixture not vendored; progress log only"]
fn p3_mgba_timer_irq_stretch() {
    let p = Path::new("tests/fixtures/mgba-suite/timer-irq");
    assert!(p.is_dir());
    panic!("mGBA timer-irq stretch not automated yet");
}
