//! jsmolka/gba-tests matrix — real BiosHle load + r12/idle oracle.
//!
//! Cited: jsmolka/gba-tests (MIT) — arm / thumb / memory gate ROMs
//!   https://github.com/jsmolka/gba-tests
//! Cited: graycart-gba test gates / apparatus (P1–P2 musts)
//!   Project store: `docs/graycart-gba/12-test-gates.md`
//!   Project store: `docs/graycart-gba/11-test-apparatus.md`
//! Note: default CI asserts arm+thumb+memory PASS (honest green after PC+12 fix).
//! Never fake green.

use crate::harness::{
    fixture_path, run_jsmolka, run_test_rom, GbaTestRom, Outcome, RomLaunchMode, RunBudget,
};
use std::path::Path;

/// One jsmolka prebuilt under `tests/fixtures/jsmolka/`.
struct JsmolkaRom {
    id: &'static str,
    relative_path: &'static str,
}

impl GbaTestRom for JsmolkaRom {
    fn id(&self) -> &str {
        self.id
    }

    fn relative_path(&self) -> &str {
        self.relative_path
    }

    fn launch_mode(&self) -> RomLaunchMode {
        RomLaunchMode::BiosHle
    }

    fn run(&self, rom: &[u8]) -> Outcome {
        run_jsmolka(rom, RunBudget::default())
    }
}

const ARM: JsmolkaRom = JsmolkaRom {
    id: "jsmolka/arm",
    relative_path: "jsmolka/arm/arm.gba",
};
const THUMB: JsmolkaRom = JsmolkaRom {
    id: "jsmolka/thumb",
    relative_path: "jsmolka/thumb/thumb.gba",
};
const MEMORY: JsmolkaRom = JsmolkaRom {
    id: "jsmolka/memory",
    relative_path: "jsmolka/memory/memory.gba",
};

/// P1/P2 must rows.
const JSMOLKA_MUST: &[JsmolkaRom] = &[ARM, THUMB, MEMORY];

/// Placeholder suite identities (LICENSE stubs only; matrices in sibling modules).
const PLACEHOLDER_SUITES: &[(&str, &str)] = &[
    ("mgba-suite/suite.gba", "tests/fixtures/mgba-suite"),
    ("nba-hw-test/ (BSD-3-Clause)", "tests/fixtures/nba-hw-test"),
    ("fuzzarm/ (GPL-3.0 ROMs)", "tests/fixtures/fuzzarm"),
];

fn print_row(name: &str, outcome: &Outcome) {
    let detail = outcome.detail();
    if detail.is_empty() {
        eprintln!("{:<40} {}", name, outcome.label());
    } else {
        eprintln!("{:<40} {}  ({detail})", name, outcome.label());
    }
}

#[test]
fn jsmolka_license_and_path_stubs_present() {
    let license = Path::new("tests/fixtures/jsmolka/LICENSE");
    assert!(
        license.is_file(),
        "missing {} — keep upstream MIT LICENSE stub",
        license.display()
    );
    let text = std::fs::read_to_string(license).expect("read LICENSE");
    assert!(
        text.contains("Julian Smolka") || text.contains("MIT"),
        "jsmolka LICENSE should retain upstream MIT attribution"
    );

    for rom in JSMOLKA_MUST {
        let dir = fixture_path(rom.relative_path())
            .parent()
            .expect("rom path has parent")
            .to_path_buf();
        assert!(
            dir.is_dir(),
            "missing fixture dir {} for {}",
            dir.display(),
            rom.id
        );
        assert!(
            dir.join("README.md").is_file(),
            "missing {}/README.md path stub",
            dir.display()
        );
        let gba = fixture_path(rom.relative_path());
        assert!(
            gba.is_file(),
            "missing vendored {} — workstream C requires MIT prebuilts in-tree",
            gba.display()
        );
        assert!(
            gba.metadata().map(|m| m.len() > 0).unwrap_or(false),
            "{} must be non-empty",
            gba.display()
        );
    }

    for (label, dir) in PLACEHOLDER_SUITES {
        assert!(
            Path::new(dir).is_dir(),
            "missing placeholder suite dir {dir} ({label})"
        );
        assert!(
            Path::new(dir).join("LICENSE").is_file()
                || Path::new(dir).join("NOTICE").is_file()
                || Path::new(dir).join("README.md").is_file(),
            "{dir} needs LICENSE/NOTICE/README stub"
        );
    }
}

/// CI smoke: loader + soft-boot + a few steps (not a suite PASS claim).
#[test]
fn jsmolka_arm_soft_boot_advances_past_header() {
    let bytes = std::fs::read("tests/fixtures/jsmolka/arm/arm.gba").expect("arm.gba vendored");
    let mut gba = graycart_gba::Gba::new();
    gba.load_rom(&bytes);
    gba.reset_bios_hle();
    for _ in 0..8 {
        gba.step_instruction();
    }
    let pc = gba.decode_pc().unwrap_or(0);
    assert!(
        pc >= 0x0800_00C0,
        "expected soft-boot to leave header toward main, pc={pc:#010x}"
    );
}

/// P1 gate (partial): thumb.gba must PASS under BiosHle + r12/idle oracle.
#[test]
fn jsmolka_thumb_suite_passes() {
    let outcome = run_test_rom(&THUMB);
    assert_eq!(
        outcome.label(),
        "PASS",
        "jsmolka/thumb: expected PASS, got {} ({})",
        outcome.label(),
        outcome.detail()
    );
}

/// P2 gate: memory.gba must PASS (mirrors + video STRB on bus write path).
#[test]
fn jsmolka_memory_suite_passes() {
    let outcome = run_test_rom(&MEMORY);
    assert_eq!(
        outcome.label(),
        "PASS",
        "jsmolka/memory: expected PASS, got {} ({})",
        outcome.label(),
        outcome.detail()
    );
}

/// P1 arm gate: arm.gba must PASS (includes PC+12 for register-specified shifts).
#[test]
fn jsmolka_arm_suite_passes() {
    let outcome = run_test_rom(&ARM);
    assert_eq!(
        outcome.label(),
        "PASS",
        "jsmolka/arm: expected PASS, got {} ({})",
        outcome.label(),
        outcome.detail()
    );
}

/// Opt-in full matrix printout (arm+thumb+memory).
///
/// `cargo test -p graycart-gba --test roms -- --ignored --nocapture`
#[test]
#[ignore = "full jsmolka matrix printout — also covered by default CI suite asserts"]
fn jsmolka_arm_thumb_memory_matrix() {
    eprintln!();
    eprintln!("{:<40} result", "ROM");
    let mut counts = [0usize; 5];
    for rom in JSMOLKA_MUST {
        let outcome = run_test_rom(rom);
        print_row(rom.id, &outcome);
        match &outcome {
            Outcome::Pass => counts[0] += 1,
            Outcome::Fail(_) => counts[1] += 1,
            Outcome::Timeout => counts[2] += 1,
            Outcome::Unsupported(_) => counts[3] += 1,
            Outcome::Skipped(_) => counts[4] += 1,
        }
        assert_ne!(
            outcome.label(),
            "SKIPPED",
            "{} still SKIPPED — loader/oracle regression",
            rom.id
        );
    }
    eprintln!();
    eprintln!(
        "summary: PASS={} FAIL={} TIMEOUT={} UNSUPPORTED={} SKIPPED={}",
        counts[0], counts[1], counts[2], counts[3], counts[4]
    );
    eprintln!("pass criteria: idle + DISPCNT Mode4|BG2 + r12==0 → PASS; r12==N → FAIL N;");
    eprintln!("  IWRAM fail digits on fail path only; LCD glyphs secondary; else TIMEOUT.");
}
