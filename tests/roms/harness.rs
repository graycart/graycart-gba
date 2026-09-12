//! Shared GBA conformance ROM harness (apparatus stub — no accuracy claim).
//!
//! Cited: graycart-gb `tests/roms/harness.rs` Outcome / launch-mode posture
//!   https://github.com/graycart/graycart-gb/blob/main/tests/roms/harness.rs
//! Cited: graycart-gba test apparatus §4 (harness API sketch)
//!   Project store: `docs/graycart-gba/11-test-apparatus.md`
//! Cited: jsmolka/gba-tests (MIT) — primary early ISA / memory suite identity
//!   https://github.com/jsmolka/gba-tests
//! Note: runner returns [`Outcome::Skipped`] / [`Outcome::Unsupported`] until
//! cart load + BiosHle step + oracle land. Do not treat matrix rows as PASS.

#![allow(dead_code)] // trait / helpers fill in as the machine boots ROMs

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

/// Result of one conformance ROM run.
///
/// [`Outcome::Skipped`] = apparatus / fixture / core not ready (this PR).
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

/// One named suite ROM under `tests/fixtures/`.
///
/// Default [`GbaTestRom::run`] is a stub: always [`Outcome::Skipped`]. Later PRs
/// load cart bytes, apply [`RomLaunchMode`], step a budget, and decode oracles
/// (jsmolka `r12` / Mode 4 / FB hash — see apparatus §4.3).
pub trait GbaTestRom {
    fn id(&self) -> &str;
    /// Path relative to `tests/fixtures/` (e.g. `jsmolka/arm/arm.gba`).
    fn relative_path(&self) -> &str;
    fn launch_mode(&self) -> RomLaunchMode;

    /// Execute when ROM bytes are available. Stub until the machine can boot.
    fn run(&self, _rom: &[u8]) -> Outcome {
        Outcome::Skipped(
            "GbaTestRom runner returns Skipped until cart load + BiosHle step + oracle".into(),
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
///
/// Absent `.gba` → [`Outcome::Skipped`]. Present bytes still yield Skipped from
/// the default stub runner until execute+oracle land — **no accuracy claim**.
pub fn run_test_rom(test: &dyn GbaTestRom) -> Outcome {
    match try_load_rom(test.relative_path()) {
        Err(outcome) => outcome,
        Ok(bytes) => {
            let _ = test.launch_mode();
            test.run(&bytes)
        }
    }
}
