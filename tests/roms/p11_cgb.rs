//! P11 CGB suite gates — Blargg cgb_sound + Mooneye misc (FastCgb).
//!
//! Cited: PHASES P11 · 08 §3.12 · 09 §9
//!   Project store: `docs/graycart-gba/PHASES.md`
//! Cited: graycart-gb `tests/roms/blargg.rs` / `mooneye_cgb.rs`
//!   https://github.com/graycart/graycart-gb
//! Note: presence/classify default; matrices `#[ignore]`. Stretch: HDMA/KEY1.

use crate::compat_harness::{run_blargg, run_mooneye, Outcome};
use graycart_gba::compat::CompatSilicon;
use std::path::{Path, PathBuf};

const BLARGG_ROOT: &str = "tests/fixtures/blargg";
const MOONEYE_ROOT: &str = "tests/fixtures/mooneye";
const SOUND_STEP_LIMIT: u32 = 20_000_000;
const MOONEYE_STEP_LIMIT: u32 = 5_000_000;

const CGB_SOUND_SINGLES: &[&str] = &[
    "cgb_sound/rom_singles/01-registers.gb",
    "cgb_sound/rom_singles/02-len ctr.gb",
    "cgb_sound/rom_singles/03-trigger.gb",
    "cgb_sound/rom_singles/04-sweep.gb",
    "cgb_sound/rom_singles/05-sweep details.gb",
    "cgb_sound/rom_singles/06-overflow on trigger.gb",
    "cgb_sound/rom_singles/07-len sweep period sync.gb",
    "cgb_sound/rom_singles/08-len ctr during power.gb",
    "cgb_sound/rom_singles/09-wave read while on.gb",
    "cgb_sound/rom_singles/10-wave trigger while on.gb",
    "cgb_sound/rom_singles/11-regs after power.gb",
    "cgb_sound/rom_singles/12-wave.gb",
];

const CGB_MOONEYE: &[(&str, &str)] = &[
    ("misc/bits/unused_hwio-C.gb", "CGB unused HWIO"),
    ("misc/boot_hwio-C.gb", "CGB boot HWIO"),
    ("misc/boot_regs-cgb.gb", "CGB boot registers A=$11"),
    ("misc/ppu/vblank_stat_intr-C.gb", "CGB STAT/VBlank"),
];

fn blargg(rel: &str) -> PathBuf {
    Path::new(BLARGG_ROOT).join(rel)
}

fn mooneye(rel: &str) -> PathBuf {
    Path::new(MOONEYE_ROOT).join(rel)
}

fn print_row(name: &str, outcome: &Outcome) {
    let detail = outcome.detail();
    if detail.is_empty() {
        eprintln!("{:<48} {}", name, outcome.label());
    } else {
        eprintln!("{:<48} {}  ({detail})", name, outcome.label());
    }
}

#[test]
fn g11_cgb_sound_fixtures_present() {
    let all = blargg("cgb_sound/cgb_sound.gb");
    assert!(all.is_file(), "missing {}", all.display());
    for rel in CGB_SOUND_SINGLES {
        let p = blargg(rel);
        assert!(p.is_file(), "missing {}", p.display());
    }
}

#[test]
fn g11_cgb_mooneye_suite_classified() {
    eprintln!();
    eprintln!("{:<42} {:<28} status", "ROM", "area");
    let mut missing = 0usize;
    let mut present = 0usize;
    for (rel, area) in CGB_MOONEYE {
        let path = mooneye(rel);
        if path.is_file() {
            present += 1;
            eprintln!("{rel:<42} {area:<28} PRESENT");
        } else {
            missing += 1;
            eprintln!("{rel:<42} {area:<28} MISSING_FIXTURE");
        }
    }
    eprintln!(
        "classified: {} listed, {present} present, {missing} missing — no silent skip-as-pass",
        CGB_MOONEYE.len()
    );
    assert_eq!(
        missing, 0,
        "G11-fixtures: CGB Mooneye misc should be vendored"
    );
}

#[test]
fn g11_cgb_mode_unit_via_compat_machine() {
    // Smoke: FastCgb launch path compiles and steps (rom_only cart).
    use graycart::Cartridge;
    let c = Cartridge::rom_only(vec![0x00; 0x8000]);
    let bytes = c.rom_bytes().to_vec();
    let mut m = graycart_gba::compat::CompatMachine::new();
    m.set_silicon(CompatSilicon::FastCgb);
    m.load_rom_bytes(&bytes).expect("FastCgb load");
    assert_eq!(m.silicon(), CompatSilicon::FastCgb);
    m.run_frames(1).expect("frame");
}

#[test]
#[ignore = "P11 Blargg cgb_sound matrix (FastCgb) — run with --ignored --nocapture"]
fn g11_cgb_sound_matrix() {
    eprintln!();
    eprintln!("{:<48} result", "ROM");
    let mut fails = Vec::new();
    for rel in CGB_SOUND_SINGLES {
        let outcome = run_blargg(&blargg(rel), SOUND_STEP_LIMIT, CompatSilicon::FastCgb);
        print_row(rel, &outcome);
        if outcome != Outcome::Pass {
            fails.push(format!("{rel}: {} {}", outcome.label(), outcome.detail()));
        }
    }
    let all = "cgb_sound/cgb_sound.gb";
    let outcome = run_blargg(
        &blargg(all),
        SOUND_STEP_LIMIT.saturating_mul(2),
        CompatSilicon::FastCgb,
    );
    print_row(all, &outcome);
    if outcome != Outcome::Pass {
        fails.push(format!("{all}: {} {}", outcome.label(), outcome.detail()));
    }
    assert!(
        fails.is_empty(),
        "G11-cgb-sound failures:\n{}",
        fails.join("\n")
    );
}

#[test]
#[ignore = "P11 Mooneye CGB misc matrix — run with --ignored --nocapture"]
fn g11_cgb_mooneye_matrix() {
    eprintln!();
    eprintln!("{:<42} {:<28} result", "ROM", "area");
    let mut counts = [0usize; 4];
    for (rel, area) in CGB_MOONEYE {
        let outcome = run_mooneye(&mooneye(rel), MOONEYE_STEP_LIMIT, CompatSilicon::FastCgb);
        eprintln!(
            "{rel:<42} {area:<28} {} {}",
            outcome.label(),
            outcome.detail()
        );
        match outcome {
            Outcome::Pass => counts[0] += 1,
            Outcome::Fail(_) => counts[1] += 1,
            Outcome::Timeout => counts[2] += 1,
            Outcome::Unsupported(_) => counts[3] += 1,
        }
    }
    eprintln!(
        "PASS={} FAIL={} TIMEOUT={} UNSUPPORTED={}",
        counts[0], counts[1], counts[2], counts[3]
    );
}

#[test]
#[ignore = "P11 stretch — CGB HDMA / KEY1 edges not in agreed board"]
fn g11_stretch_cgb_hdma_key1() {
    // Placeholder so the stretch gate is named; do not treat as PASS.
    panic!("stretch only — no HDMA/KEY1 fixtures in P11 board");
}
