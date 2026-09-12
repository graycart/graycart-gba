//! BIOS LLE map + HLE SWI table (BiosHle for suite soft-boot).
//!
//! Module layout from graycart-gba implementation plan §2.2.
//! Behavior: see research `docs/graycart-gba/06-cart-bios-saves.md`.
//!
//! Cited: GBATEK — BIOS Functions / Unpredictable Things (BIOS protect)
//!   https://problemkaputt.de/gbatek.htm
//! Note: user-supplied BIOS only for LLE — never commit BIOS images.

pub mod hle;
pub mod lle;
pub mod protect;

#[cfg(test)]
mod tests_hle;
#[cfg(test)]
mod tests_lle;

pub use protect::{
    HLE_IRQ_RETURN, IRQ_HANDLER_PTR, LATCH_AFTER_IRQ, LATCH_AFTER_SWI, LATCH_DURING_IRQ,
    LATCH_SOFT_RESET,
};

/// BIOS glue: tracks whether soft-boot HLE or mapped LLE firmware is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BiosMode {
    /// Soft entry at cart/multiboot; SWI Div HLE enabled in the step loop.
    #[default]
    Hle,
    /// Real `gba_bios.bin` mapped (user-provided).
    Lle,
}

/// BIOS owner. Image bytes (when LLE) live on [`crate::bus::Bus::bios`].
#[derive(Debug, Clone, Default)]
pub struct Bios {
    pub mode: BiosMode,
    /// Resume PC after HLE IRQ trampoline (`None` when idle).
    pub hle_irq_resume: Option<u32>,
    /// Saved r0–r3,r12 across BiosHle IRQ user ISR (BIOS wrapper semantics).
    pub hle_irq_regs: [u32; 5],
}

impl Bios {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
