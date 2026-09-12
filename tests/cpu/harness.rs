//! Shared CPU conformance harness stubs (graycart-gb Outcome posture).
//!
//! Cited: graycart-gb `tests/roms/harness.rs` Outcome enum
//!   https://github.com/graycart/graycart-gb
//! Cited: graycart-gba test strategy §5.1
//!   Project store: `docs/graycart-gba/07-test-strategy.md`

#![allow(dead_code)] // stubs exercised as the ROM runner fills in

/// How a GBA fixture is launched (BIOS / cart entry).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomLaunchMode {
    /// Soft entry ~`0x08000000` with HLE SWI (default homebrew).
    BiosHle,
    /// Real `gba_bios.bin` mapped (user-provided, never in git).
    BiosLle,
    /// Multiboot entry at `0x02000000`.
    Multiboot,
}

/// Result of one conformance ROM run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail(String),
    Timeout,
    Unsupported(String),
}

impl Outcome {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail(_) => "FAIL",
            Self::Timeout => "TIMEOUT",
            Self::Unsupported(_) => "UNSUPPORTED",
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            Self::Pass | Self::Timeout => "",
            Self::Fail(s) | Self::Unsupported(s) => s,
        }
    }
}
