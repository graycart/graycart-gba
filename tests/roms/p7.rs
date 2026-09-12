//! P7 Cart/BIOS/saves harness — jsmolka `bios` + `save/*` under BiosHle.
//!
//! Cited: jsmolka/gba-tests (MIT) — bios / save ROMs
//!   https://github.com/jsmolka/gba-tests
//! Cited: graycart-gba PHASES P7 · 06/07/11/12
//!   Project store: `docs/graycart-gba/PHASES.md`
//! Note: unit gates (G7-*) live under `src/cart/` + `src/bios/`.
//! Keep arm+thumb+memory green; never fake PASS.

use crate::harness::{
    fixture_path, run_jsmolka, run_test_rom, GbaTestRom, Outcome, RomLaunchMode, RunBudget,
};
use std::path::Path;

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
        // Save/bios suites can take longer than arm/thumb on soft erase loops.
        run_jsmolka(
            rom,
            RunBudget {
                max_steps: 200_000_000,
                idle_hold: 64,
            },
        )
    }
}

const BIOS: JsmolkaRom = JsmolkaRom {
    id: "jsmolka/bios",
    relative_path: "jsmolka/bios/bios.gba",
};
const SAVE_NONE: JsmolkaRom = JsmolkaRom {
    id: "jsmolka/save/none",
    relative_path: "jsmolka/save/none/none.gba",
};
const SAVE_SRAM: JsmolkaRom = JsmolkaRom {
    id: "jsmolka/save/sram",
    relative_path: "jsmolka/save/sram/sram.gba",
};
const SAVE_FLASH64: JsmolkaRom = JsmolkaRom {
    id: "jsmolka/save/flash64",
    relative_path: "jsmolka/save/flash64/flash64.gba",
};
const SAVE_FLASH128: JsmolkaRom = JsmolkaRom {
    id: "jsmolka/save/flash128",
    relative_path: "jsmolka/save/flash128/flash128.gba",
};

const P7_MUST: &[JsmolkaRom] = &[BIOS, SAVE_NONE, SAVE_SRAM, SAVE_FLASH64, SAVE_FLASH128];

#[test]
fn p7_fixture_paths_present() {
    for rom in P7_MUST {
        let path = fixture_path(rom.relative_path());
        assert!(
            path.is_file(),
            "missing P7 fixture {} — vendor MIT prebuilt",
            path.display()
        );
        assert!(
            path.metadata().map(|m| m.len() > 0).unwrap_or(false),
            "{} empty",
            path.display()
        );
        let readme = path.parent().unwrap().join("README.md");
        assert!(readme.is_file(), "missing {} path stub", readme.display());
    }
}

#[test]
fn jsmolka_bios_suite_passes() {
    let outcome = run_test_rom(&BIOS);
    eprintln!(
        "{:<40} {}  ({})",
        BIOS.id(),
        outcome.label(),
        outcome.detail()
    );
    assert!(
        outcome.is_pass(),
        "G7-bios-rom: expected PASS, got {} ({})",
        outcome.label(),
        outcome.detail()
    );
}

#[test]
fn jsmolka_save_none_passes() {
    let outcome = run_test_rom(&SAVE_NONE);
    eprintln!(
        "{:<40} {}  ({})",
        SAVE_NONE.id(),
        outcome.label(),
        outcome.detail()
    );
    assert!(
        outcome.is_pass(),
        "G7-save-rom none: expected PASS, got {} ({})",
        outcome.label(),
        outcome.detail()
    );
}

#[test]
fn jsmolka_save_sram_passes() {
    let outcome = run_test_rom(&SAVE_SRAM);
    eprintln!(
        "{:<40} {}  ({})",
        SAVE_SRAM.id(),
        outcome.label(),
        outcome.detail()
    );
    assert!(
        outcome.is_pass(),
        "G7-save-rom sram: expected PASS, got {} ({})",
        outcome.label(),
        outcome.detail()
    );
}

#[test]
fn jsmolka_save_flash64_passes() {
    let outcome = run_test_rom(&SAVE_FLASH64);
    eprintln!(
        "{:<40} {}  ({})",
        SAVE_FLASH64.id(),
        outcome.label(),
        outcome.detail()
    );
    assert!(
        outcome.is_pass(),
        "G7-save-rom flash64: expected PASS, got {} ({})",
        outcome.label(),
        outcome.detail()
    );
}

#[test]
fn jsmolka_save_flash128_passes() {
    let outcome = run_test_rom(&SAVE_FLASH128);
    eprintln!(
        "{:<40} {}  ({})",
        SAVE_FLASH128.id(),
        outcome.label(),
        outcome.detail()
    );
    assert!(
        outcome.is_pass(),
        "G7-save-rom flash128: expected PASS, got {} ({})",
        outcome.label(),
        outcome.detail()
    );
}

/// Stretch: LLE boot needs user BIOS — honest ignore.
#[test]
#[ignore = "G7-lle-stretch: requires user-supplied gba_bios.bin (never in git)"]
fn g7_lle_stretch_requires_user_bios() {
    assert!(!Path::new("gba_bios.bin").is_file());
    let outcome = Outcome::Unsupported("no gba_bios.bin".into());
    assert_eq!(outcome.label(), "UNSUPPORTED");
}
