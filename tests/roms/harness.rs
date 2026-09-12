//! Shared GBA conformance ROM harness (load + BiosHle step + oracles).
//!
//! Cited: graycart-gb `tests/roms/harness.rs` Outcome / launch-mode posture
//!   https://github.com/graycart/graycart-gb/blob/main/tests/roms/harness.rs
//! Cited: graycart-gba test apparatus §4 (harness API) / §4.3 (jsmolka oracles)
//!   Project store: `docs/graycart-gba/11-test-apparatus.md`
//! Cited: jsmolka/gba-tests (MIT) — `m_test_eval` / idle / Mode 4 text
//!   https://github.com/jsmolka/gba-tests
//! Note: outcomes are real PASS/FAIL/TIMEOUT once ROMs load — never fake green.
//! Suites stay `#[ignore]` until CPU coverage can complete them.

#![allow(dead_code)] // helpers used by suite matrices + unit smoke

use graycart_gba::{Gba, RomLaunchMode as CoreLaunch};
use std::path::{Path, PathBuf};

/// How a GBA fixture is launched (BIOS / cart entry).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomLaunchMode {
    /// Soft entry ~`0x08000000` with HLE SWI (default homebrew / jsmolka).
    BiosHle,
    /// Real `gba_bios.bin` mapped (user-provided, never in git).
    BiosLle,
    /// Multiboot entry at `0x02000000`.
    Multiboot,
}

impl RomLaunchMode {
    fn to_core(self) -> CoreLaunch {
        match self {
            Self::BiosHle => CoreLaunch::BiosHle,
            Self::BiosLle => CoreLaunch::BiosLle,
            Self::Multiboot => CoreLaunch::Multiboot,
        }
    }
}

/// Result of one conformance ROM run.
///
/// [`Outcome::Skipped`] = fixture absent or suite not wired.
/// [`Outcome::Unsupported`] = known-missing capability (e.g. BIOS LLE absent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail(String),
    Timeout,
    Unsupported(String),
    Skipped(String),
}

impl Outcome {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail(_) => "FAIL",
            Self::Timeout => "TIMEOUT",
            Self::Unsupported(_) => "UNSUPPORTED",
            Self::Skipped(_) => "SKIPPED",
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            Self::Pass | Self::Timeout => "",
            Self::Fail(s) | Self::Unsupported(s) | Self::Skipped(s) => s,
        }
    }

    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass)
    }
}

/// Instruction budget for a headless suite run (crude cycle proxy).
#[derive(Debug, Clone, Copy)]
pub struct RunBudget {
    pub max_steps: u64,
    /// Consecutive identical Decode PCs before treating the ROM as idle.
    pub idle_hold: u32,
}

impl Default for RunBudget {
    fn default() -> Self {
        Self {
            // Enough for arm.gba on a working core; still times out if hung in tests.
            max_steps: 50_000_000,
            idle_hold: 64,
        }
    }
}

/// One named suite ROM under `tests/fixtures/`.
pub trait GbaTestRom {
    fn id(&self) -> &str;
    /// Path relative to `tests/fixtures/` (e.g. `jsmolka/arm/arm.gba`).
    fn relative_path(&self) -> &str;
    fn launch_mode(&self) -> RomLaunchMode;

    /// Execute when ROM bytes are available. Default: Skipped (unimplemented suite).
    fn run(&self, _rom: &[u8]) -> Outcome {
        Outcome::Skipped(
            "GbaTestRom::run not overridden for this suite (no load+oracle yet)".into(),
        )
    }
}

/// Absolute-from-crate-root fixture path for `relative` under `tests/fixtures/`.
pub fn fixture_path(relative: &str) -> PathBuf {
    Path::new("tests/fixtures").join(relative)
}

/// Read ROM bytes, or [`Outcome::Skipped`] if the file is absent (not vendored yet).
pub fn try_load_rom(relative: &str) -> Result<Vec<u8>, Outcome> {
    let path = fixture_path(relative);
    if !path.is_file() {
        return Err(Outcome::Skipped(format!(
            "ROM absent (not vendored): {}",
            path.display()
        )));
    }
    std::fs::read(&path)
        .map_err(|e| Outcome::Unsupported(format!("failed to read {}: {e}", path.display())))
}

/// Load fixture (if present) and call [`GbaTestRom::run`].
pub fn run_test_rom(test: &dyn GbaTestRom) -> Outcome {
    match try_load_rom(test.relative_path()) {
        Err(outcome) => outcome,
        Ok(bytes) => test.run(&bytes),
    }
}

/// Build a machine: cart load + launch-mode reset.
pub fn load_gba(rom: &[u8], mode: RomLaunchMode) -> Result<Gba, Outcome> {
    let mut gba = Gba::new();
    gba.load_rom(rom);
    match gba.reset(mode.to_core()) {
        Ok(()) => Ok(gba),
        Err(e) => Err(Outcome::Unsupported(e)),
    }
}

/// Step until `oracle` returns Some, or budget exhausted → [`Outcome::Timeout`].
pub fn run_until(
    gba: &mut Gba,
    budget: RunBudget,
    mut oracle: impl FnMut(&Gba) -> Option<Outcome>,
) -> Outcome {
    let mut last_pc: Option<u32> = None;
    let mut same = 0u32;

    for _ in 0..budget.max_steps {
        gba.step_instruction();
        if let Some(o) = oracle(gba) {
            return o;
        }
        let pc = gba.decode_pc();
        if pc == last_pc && pc.is_some() {
            same = same.saturating_add(1);
            if same >= budget.idle_hold {
                if let Some(o) = jsmolka_idle_oracle(gba) {
                    return o;
                }
            }
        } else {
            same = 0;
            last_pc = pc;
        }
    }
    Outcome::Timeout
}

// --- jsmolka oracles (apparatus §4.3) ---------------------------------------

/// DISPCNT (`0x04000000`): Mode 4 + BG2 enable after `m_test_init` / `text_init`.
pub const JSMOLKA_DISPCNT_MODE4_BG2: u16 = 4 | (1 << 10);

/// Primary cooperative oracle: after idle, `r12 == 0` → PASS, else FAIL with test #.
///
/// jsmolka `m_exit` / aggregator leave the fail number in `r12`; `m_test_eval`
/// pushes/pops it so idle still sees the value. Pass path keeps `r12 == 0`.
pub fn jsmolka_r12_oracle(gba: &Gba) -> Outcome {
    let n = gba.r12();
    if n == 0 {
        Outcome::Pass
    } else {
        Outcome::Fail(format!("jsmolka failed test {n} (r12)"))
    }
}

/// Idle settle helper used by [`run_until`].
pub fn jsmolka_idle_oracle(gba: &Gba) -> Option<Outcome> {
    // Require Mode 4 + BG2 so we do not score a hang before `m_test_init`.
    let dispcnt = gba.io16(0);
    if dispcnt & JSMOLKA_DISPCNT_MODE4_BG2 != JSMOLKA_DISPCNT_MODE4_BG2 {
        return None;
    }
    Some(jsmolka_r12_oracle(gba))
}

/// Fail-digit scratch in IWRAM used by `m_test_eval` (hundreds/tens/ones words).
///
/// Not a pass magic — only populated on the fail path (after Div SWI). Exposed
/// for diagnostics when `r12 != 0`.
pub fn jsmolka_iwram_fail_digits(gba: &Gba) -> Option<(u32, u32, u32)> {
    let b0 = u32::from(gba.iwram8(0));
    let b1 = u32::from(gba.iwram8(1));
    let b2 = u32::from(gba.iwram8(2));
    let b3 = u32::from(gba.iwram8(3));
    let hundreds = b0 | (b1 << 8) | (b2 << 16) | (b3 << 24);
    let tens = u32::from(gba.iwram8(4))
        | (u32::from(gba.iwram8(5)) << 8)
        | (u32::from(gba.iwram8(6)) << 16)
        | (u32::from(gba.iwram8(7)) << 24);
    let ones = u32::from(gba.iwram8(8))
        | (u32::from(gba.iwram8(9)) << 8)
        | (u32::from(gba.iwram8(10)) << 16)
        | (u32::from(gba.iwram8(11)) << 24);
    if gba.r12() == 0 {
        None
    } else {
        Some((hundreds, tens, ones))
    }
}

/// Mode 4 LCD smoke: any non-zero VRAM byte near the text row (y≈76).
///
/// Glyphs are bitmaps (not ASCII). A full “All tests passed” string scan needs
/// glyph decode or a golden FB hash (apparatus §4.3 #2/#3) — deferred; r12 is
/// the authoritative PASS/FAIL.
pub fn jsmolka_lcd_text_drawn(gba: &Gba) -> bool {
    // Mode 4: 240×160 bytes; row 76 starts at 76*240 = 18240.
    let row = 76usize * 240;
    for off in row..row.saturating_add(240) {
        if gba.vram8(off) != 0 {
            return true;
        }
    }
    false
}

/// Run a jsmolka ROM under BiosHle with r12/idle oracle.
pub fn run_jsmolka(rom: &[u8], budget: RunBudget) -> Outcome {
    let mut gba = match load_gba(rom, RomLaunchMode::BiosHle) {
        Ok(g) => g,
        Err(o) => return o,
    };
    run_until(&mut gba, budget, |_| None)
}
