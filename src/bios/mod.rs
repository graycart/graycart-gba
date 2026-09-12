//! BIOS LLE map + HLE SWI table (minimal BiosHle for suite soft-boot).
//!
//! Module layout from graycart-gba implementation plan §2.2.
//! Behavior: see research `docs/graycart-gba/06-cart-bios-saves.md`.
//!
//! Cited: GBATEK — BIOS Functions (Div SWI 0x06)
//!   https://problemkaputt.de/gbatek.htm
//! Note: user-supplied BIOS only for LLE — never commit BIOS images. Div HLE
//! lives in [`crate::cpu::step`] for the instruction loop; this type records
//! launch posture for the machine orchestrator.

/// BIOS glue: tracks whether soft-boot HLE or mapped LLE firmware is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BiosMode {
    /// Soft entry at cart/multiboot; SWI Div HLE enabled in the step loop.
    #[default]
    Hle,
    /// Real `gba_bios.bin` mapped (user-provided).
    Lle,
}

/// Stub BIOS owner. Image bytes (when LLE) live on [`crate::bus::Bus::bios`].
#[derive(Debug, Clone, Default)]
pub struct Bios {
    pub mode: BiosMode,
}
