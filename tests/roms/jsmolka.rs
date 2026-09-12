//! jsmolka/gba-tests ignored matrix (apparatus only — no accuracy claim).
//!
//! Cited: jsmolka/gba-tests (MIT) — arm / thumb / memory gate ROMs
//!   https://github.com/jsmolka/gba-tests
//! Cited: graycart-gba test gates / apparatus (P1–P2 musts)
//!   Project store: `docs/graycart-gba/12-test-gates.md`
//!   Project store: `docs/graycart-gba/11-test-apparatus.md`
//! Note: default CI never runs these (`#[ignore]`). Rows stay SKIPPED until
//! ROMs are vendored (workstream C) and the runner can load+oracle (D/E).

use crate::harness::{fixture_path, run_test_rom, GbaTestRom, Outcome, RomLaunchMode};
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
        // Soft-boot at cart base; HLE Div SWI needed later for fail-digit path.
        RomLaunchMode::BiosHle
    }
}

/// P1/P2 must rows (binaries not committed — path stubs + LICENSE only).
const JSMOLKA_MUST: &[JsmolkaRom] = &[
    JsmolkaRom {
        id: "jsmolka/arm",
        relative_path: "jsmolka/arm/arm.gba",
    },
    JsmolkaRom {
        id: "jsmolka/thumb",
        relative_path: "jsmolka/thumb/thumb.gba",
    },
    JsmolkaRom {
        id: "jsmolka/memory",
        relative_path: "jsmolka/memory/memory.gba",
    },
];

/// Placeholder suite identities (also not vendored; matrix documents the spine).
const PLACEHOLDER_SUITES: &[(&str, &str)] = &[
    ("mgba-suite/suite.gba", "tests/fixtures/mgba-suite"),
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
        // `.gba` binaries are optional until workstream C; absence → SKIPPED in runner.
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

#[test]
fn jsmolka_runner_skipped_without_roms() {
    for rom in JSMOLKA_MUST {
        let outcome = run_test_rom(rom);
        assert_eq!(
            outcome.label(),
            "SKIPPED",
            "{} expected SKIPPED (absent ROM or stub runner), got {} ({})",
            rom.id,
            outcome.label(),
            outcome.detail()
        );
        assert!(
            !outcome.is_pass(),
            "{} must not claim PASS before execute+oracle",
            rom.id
        );
    }
}

/// Opt-in matrix: prints SKIPPED rows until ROMs + runner land.
///
/// Run locally: `cargo test -p graycart-gba --test roms -- --ignored --nocapture`
#[test]
#[ignore = "jsmolka arm/thumb/memory matrix; apparatus stub — no accuracy claim"]
fn jsmolka_arm_thumb_memory_matrix() {
    eprintln!();
    eprintln!("{:<40} result", "ROM");
    let mut counts = [0usize; 5]; // pass fail timeout unsupported skipped
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
        // Soft contract for this PR: never claim PASS.
        assert!(
            !outcome.is_pass(),
            "{} returned PASS before runner/oracle — unexpected",
            rom.id
        );
    }
    eprintln!();
    eprintln!(
        "summary: PASS={} FAIL={} TIMEOUT={} UNSUPPORTED={} SKIPPED={}",
        counts[0], counts[1], counts[2], counts[3], counts[4]
    );
    eprintln!("(placeholders: mgba-suite + fuzzarm LICENSE stubs only — not scored)");
}
