//! P11 DMG suite gates — Blargg cpu_instrs / dmg_sound + Mooneye acceptance.
//!
//! Cited: PHASES P11 · 08 §3.12 · 09 §9
//!   Project store: `docs/graycart-gba/PHASES.md`
//! Cited: graycart-gb `tests/roms/blargg.rs` / `mooneye.rs` (same ROM lists)
//!   https://github.com/graycart/graycart-gb
//! Note: presence default; full matrices `#[ignore]`. Keep jsmolka green.

use crate::compat_harness::{run_blargg, run_mooneye, Outcome};
use graycart_gba::compat::CompatSilicon;
use std::path::{Path, PathBuf};

const BLARGG_ROOT: &str = "tests/fixtures/blargg";
const MOONEYE_ROOT: &str = "tests/fixtures/mooneye";
const STEP_LIMIT: u32 = 50_000_000;
const SOUND_STEP_LIMIT: u32 = 20_000_000;
const MOONEYE_STEP_LIMIT: u32 = 5_000_000;

/// Agreed P11 DMG Blargg cpu_instrs threshold (11 individual + all-in-one).
pub const G11_DMG_CPU_THRESHOLD: usize = 12;

const CPU_INSTRS_INDIVIDUAL: &[&str] = &[
    "cpu_instrs/individual/01-special.gb",
    "cpu_instrs/individual/02-interrupts.gb",
    "cpu_instrs/individual/03-op sp,hl.gb",
    "cpu_instrs/individual/04-op r,imm.gb",
    "cpu_instrs/individual/05-op rp.gb",
    "cpu_instrs/individual/06-ld r,r.gb",
    "cpu_instrs/individual/07-jr,jp,call,ret,rst.gb",
    "cpu_instrs/individual/08-misc instrs.gb",
    "cpu_instrs/individual/09-op r,r.gb",
    "cpu_instrs/individual/10-bit ops.gb",
    "cpu_instrs/individual/11-op a,(hl).gb",
];

const DMG_SOUND_SINGLES: &[&str] = &[
    "dmg_sound/rom_singles/01-registers.gb",
    "dmg_sound/rom_singles/02-len ctr.gb",
    "dmg_sound/rom_singles/03-trigger.gb",
    "dmg_sound/rom_singles/04-sweep.gb",
    "dmg_sound/rom_singles/05-sweep details.gb",
    "dmg_sound/rom_singles/06-overflow on trigger.gb",
    "dmg_sound/rom_singles/07-len sweep period sync.gb",
    "dmg_sound/rom_singles/08-len ctr during power.gb",
    "dmg_sound/rom_singles/09-wave read while on.gb",
    "dmg_sound/rom_singles/10-wave trigger while on.gb",
    "dmg_sound/rom_singles/11-regs after power.gb",
    "dmg_sound/rom_singles/12-wave write while on.gb",
];

/// Mooneye acceptance board (same curated set as graycart-gb).
const MOONEYE_SUITE: &[&str] = &[
    "acceptance/bits/mem_oam.gb",
    "acceptance/bits/reg_f.gb",
    "acceptance/bits/unused_hwio-GS.gb",
    "acceptance/timer/tim00.gb",
    "acceptance/timer/tim01.gb",
    "acceptance/timer/tim10.gb",
    "acceptance/timer/tim11.gb",
    "acceptance/timer/tima_reload.gb",
    "acceptance/timer/div_write.gb",
    "acceptance/interrupts/ie_push.gb",
    "acceptance/oam_dma/basic.gb",
    "acceptance/oam_dma/reg_read.gb",
    "acceptance/oam_dma/sources-GS.gb",
    "acceptance/oam_dma_start.gb",
    "acceptance/oam_dma_timing.gb",
    "acceptance/oam_dma_restart.gb",
    "acceptance/halt_ime0_ei.gb",
    "acceptance/halt_ime1_timing.gb",
    "acceptance/ei_timing.gb",
    "acceptance/if_ie_registers.gb",
    "acceptance/intr_timing.gb",
    "acceptance/div_timing.gb",
    "acceptance/ppu/intr_2_oam_ok_timing.gb",
    "acceptance/ppu/intr_2_mode3_timing.gb",
    "acceptance/ppu/hblank_ly_scx_timing-GS.gb",
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
fn g11_dmg_cpu_fixtures_present() {
    let all = blargg("cpu_instrs/cpu_instrs.gb");
    assert!(
        all.is_file(),
        "missing {} — G11-fixtures (copy from graycart-gb / retrio)",
        all.display()
    );
    for rel in CPU_INSTRS_INDIVIDUAL {
        let p = blargg(rel);
        assert!(p.is_file(), "missing {}", p.display());
    }
}

#[test]
fn g11_dmg_sound_fixtures_present() {
    let all = blargg("dmg_sound/dmg_sound.gb");
    assert!(all.is_file(), "missing {} — G11-fixtures", all.display());
    for rel in DMG_SOUND_SINGLES {
        let p = blargg(rel);
        assert!(p.is_file(), "missing {}", p.display());
    }
}

#[test]
fn g11_dmg_mooneye_fixtures_present() {
    let mut missing = Vec::new();
    for rel in MOONEYE_SUITE {
        let p = mooneye(rel);
        if !p.is_file() {
            missing.push(p.display().to_string());
        }
    }
    assert!(
        missing.is_empty(),
        "missing Mooneye fixtures:\n{}",
        missing.join("\n")
    );
}

#[test]
fn g11_dmg_cpu_smoke_01_special_when_present() {
    let path = blargg(CPU_INSTRS_INDIVIDUAL[0]);
    if !path.is_file() {
        eprintln!("G11-dmg-cpu: {} absent — SKIP OK", path.display());
        return;
    }
    let o = run_blargg(&path, STEP_LIMIT, CompatSilicon::FastDmg);
    assert_eq!(
        o,
        Outcome::Pass,
        "{}: {} {}",
        path.display(),
        o.label(),
        o.detail()
    );
}

#[test]
#[ignore = "P11 Blargg cpu_instrs matrix — run with --ignored --nocapture"]
fn g11_dmg_cpu_instrs_matrix() {
    eprintln!();
    eprintln!("{:<48} result", "ROM");
    let mut passes = 0usize;
    let mut fails = Vec::new();
    for rel in CPU_INSTRS_INDIVIDUAL {
        let outcome = run_blargg(&blargg(rel), STEP_LIMIT, CompatSilicon::FastDmg);
        print_row(rel, &outcome);
        if outcome == Outcome::Pass {
            passes += 1;
        } else {
            fails.push(format!("{rel}: {} {}", outcome.label(), outcome.detail()));
        }
    }
    let all = "cpu_instrs/cpu_instrs.gb";
    let outcome = run_blargg(
        &blargg(all),
        STEP_LIMIT.saturating_mul(2),
        CompatSilicon::FastDmg,
    );
    print_row(all, &outcome);
    if outcome == Outcome::Pass {
        passes += 1;
    } else {
        fails.push(format!("{all}: {} {}", outcome.label(), outcome.detail()));
    }
    eprintln!("PASS={passes} / threshold={G11_DMG_CPU_THRESHOLD}");
    assert!(
        fails.is_empty() && passes >= G11_DMG_CPU_THRESHOLD,
        "G11-dmg-cpu failures:\n{}",
        fails.join("\n")
    );
}

#[test]
#[ignore = "P11 Blargg dmg_sound matrix — run with --ignored --nocapture"]
fn g11_dmg_sound_matrix() {
    eprintln!();
    eprintln!("{:<48} result", "ROM");
    let mut passes = 0usize;
    let mut fails = Vec::new();
    for rel in DMG_SOUND_SINGLES {
        let outcome = run_blargg(&blargg(rel), SOUND_STEP_LIMIT, CompatSilicon::FastDmg);
        print_row(rel, &outcome);
        if outcome == Outcome::Pass {
            passes += 1;
        } else {
            fails.push(format!("{rel}: {} {}", outcome.label(), outcome.detail()));
        }
    }
    let all = "dmg_sound/dmg_sound.gb";
    let outcome = run_blargg(
        &blargg(all),
        SOUND_STEP_LIMIT.saturating_mul(2),
        CompatSilicon::FastDmg,
    );
    print_row(all, &outcome);
    if outcome == Outcome::Pass {
        passes += 1;
    } else {
        fails.push(format!("{all}: {} {}", outcome.label(), outcome.detail()));
    }
    eprintln!("PASS={passes}");
    assert!(
        fails.is_empty(),
        "G11-dmg-sound failures:\n{}",
        fails.join("\n")
    );
}

#[test]
#[ignore = "P11 Mooneye acceptance matrix — run with --ignored --nocapture"]
fn g11_dmg_mooneye_matrix() {
    eprintln!();
    eprintln!("{:<48} result", "ROM");
    let mut counts = [0usize; 4];
    for rel in MOONEYE_SUITE {
        let outcome = run_mooneye(&mooneye(rel), MOONEYE_STEP_LIMIT, CompatSilicon::FastDmg);
        print_row(rel, &outcome);
        match outcome {
            Outcome::Pass => counts[0] += 1,
            Outcome::Fail(_) => counts[1] += 1,
            Outcome::Timeout => counts[2] += 1,
            Outcome::Unsupported(_) => counts[3] += 1,
        }
    }
    eprintln!(
        "summary: PASS={} FAIL={} TIMEOUT={} UNSUPPORTED={}",
        counts[0], counts[1], counts[2], counts[3]
    );
    assert_eq!(
        counts[3], 0,
        "G11-dmg-mooneye soft gate: no UNSUPPORTED (missing CPU/hardware)"
    );
}
