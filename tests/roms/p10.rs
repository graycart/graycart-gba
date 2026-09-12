//! P10 DMG/CGB compat bring-up gates — honest smoke + fixture hygiene.
//!
//! Cited: graycart-gba PHASES P10 · 08 §3.11 · 09 §9 · 10 reuse
//!   Project store: `docs/graycart-gba/PHASES.md`
//! Cited: Blargg via retrio/gb-test-roms (same culture as graycart-gb)
//!   https://github.com/retrio/gb-test-roms
//! Note: default CI skips when Blargg binaries absent; `#[ignore]` for long smoke.
//! Keep jsmolka arm/thumb/memory green.

use graycart_gba::compat::{CompatMachine, GRAYCART_DEP_LABEL, GRAYCART_GIT_REV};
use std::path::{Path, PathBuf};

/// Documented P10 exit: DMG smoke via compat wrapper; CI ROM-free when fixtures absent.
pub const P10_EXIT: &str =
    "P10: graycart wrap + WAITCNT/Mode-8 posture; Blargg cpu_instrs smoke when present";

const BLARGG_ROOT: &str = "tests/fixtures/blargg";
const SMOKE_ROM: &str = "cpu_instrs/individual/01-special.gb";
/// Generous step budget — matches graycart-gb blargg individual ROMs.
const STEP_LIMIT: u32 = 50_000_000;

fn fixture(rel: &str) -> PathBuf {
    Path::new(BLARGG_ROOT).join(rel)
}

#[test]
fn p10_exit_note() {
    assert!(P10_EXIT.contains("P10"));
    assert!(P10_EXIT.contains("graycart") || P10_EXIT.contains("compat"));
    eprintln!("G10-docs: {P10_EXIT}");
    eprintln!("G10-dep: {GRAYCART_DEP_LABEL} @ {GRAYCART_GIT_REV}");
}

#[test]
fn blargg_license_readme_present() {
    let license = Path::new(BLARGG_ROOT).join("LICENSE");
    let readme = Path::new(BLARGG_ROOT).join("README.md");
    assert!(
        license.is_file(),
        "missing {} — fixture LICENSE stub required (G10-fixtures)",
        license.display()
    );
    assert!(
        readme.is_file(),
        "missing {} — fixture README required (G10-fixtures)",
        readme.display()
    );
    let lic = std::fs::read_to_string(&license).expect("LICENSE");
    assert!(
        lic.to_ascii_lowercase().contains("blargg")
            || lic.contains("Shay Green")
            || lic.contains("retrio"),
        "LICENSE should attribute Blargg / retrio"
    );
}

#[test]
fn mooneye_license_stub_present() {
    let license = Path::new("tests/fixtures/mooneye/LICENSE");
    let readme = Path::new("tests/fixtures/mooneye/README.md");
    assert!(license.is_file(), "missing mooneye LICENSE stub");
    assert!(readme.is_file(), "missing mooneye README stub");
}

#[test]
fn compat_machine_api_smoke_without_rom() {
    // G10-api / G10-wrap: construct + run placeholder without fixtures.
    let mut m = CompatMachine::new();
    assert_eq!(m.system_id(), "gb-compat");
    m.run_frames(1).expect("placeholder frame");
    assert_eq!(m.framebuffer_shades().len(), 160 * 144);
    let _ = m.drain_audio();
}

#[test]
fn dmg_smoke_skips_when_fixture_absent() {
    // G10-smoke: never fail default CI for missing Blargg binaries.
    let path = fixture(SMOKE_ROM);
    if !path.is_file() {
        eprintln!(
            "G10-smoke: {} absent — SKIP OK (copy from retrio/gb-test-roms)",
            path.display()
        );
        return;
    }
    run_blargg_smoke(&path);
}

#[test]
#[ignore = "long Blargg cpu_instrs smoke — run locally when fixtures vendored"]
fn dmg_smoke_blargg_01_special_ignored() {
    let path = fixture(SMOKE_ROM);
    assert!(
        path.is_file(),
        "missing {} — vendor Blargg before --ignored",
        path.display()
    );
    run_blargg_smoke(&path);
}

fn run_blargg_smoke(path: &Path) {
    let mut m = CompatMachine::new();
    m.load_rom_path(path).expect("load smoke ROM");
    assert!(m.handoff().sm83_active);

    for i in 0..STEP_LIMIT {
        if let Some(pass) = blargg_done(&m) {
            assert!(pass, "Blargg reported Failed (step {i})");
            eprintln!("G10-smoke: PASS {} in {i} steps", path.display());
            return;
        }
        m.step_instruction()
            .unwrap_or_else(|e| panic!("step {i}: {e}"));
    }
    panic!(
        "TIMEOUT after {STEP_LIMIT} steps; serial={}",
        m.serial_text()
    );
}

/// Blargg serial / RAM oracle (same idea as graycart-gb tests/roms/harness.rs).
fn blargg_done(m: &CompatMachine) -> Option<bool> {
    let text = m.serial_text();
    if text.contains("Passed") {
        return Some(true);
    }
    if text.contains("Failed") {
        return Some(false);
    }
    if m.read8(0xA001) == 0xDE && m.read8(0xA002) == 0xB0 && m.read8(0xA003) == 0x61 {
        let status = m.read8(0xA000);
        if status == 0x80 {
            return None;
        }
        return Some(status == 0);
    }
    None
}

#[test]
fn conformance_mentions_p10() {
    let text = std::fs::read_to_string("docs/conformance.md").expect("conformance");
    assert!(
        text.contains("P10") || text.contains("compat"),
        "docs/conformance.md should record a P10 compat board"
    );
}

#[test]
fn crate_version_patch_after_p9() {
    let v = env!("CARGO_PKG_VERSION");
    assert!(
        v == "0.1.1" || v.starts_with("0.1."),
        "P10 expected patch on 0.1.x, got {v}"
    );
}
