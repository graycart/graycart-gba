//! BIOS protect latch constants (jsmolka `bios` residues) + HLE helpers.
//!
//! Cited: GBATEK — Unpredictable Things (BIOS protect)
//!   https://problemkaputt.de/gbatek-gba-unpredictable-things.htm
//! Cited: jsmolka/gba-tests `bios/bios.asm` (MIT) — expected residues
//!   https://github.com/jsmolka/gba-tests
//! Research: Project store `docs/graycart-gba/06-cart-bios-saves.md` §3.3

/// SoftReset / startup last-fetched BIOS opcode (`[00DCh+8]`).
pub const LATCH_SOFT_RESET: u32 = 0xE129_F000;
/// After SWI return (`[0188h+8]`).
pub const LATCH_AFTER_SWI: u32 = 0xE3A0_2004;
/// During IRQ user ISR (`[0134h+8]`).
pub const LATCH_DURING_IRQ: u32 = 0xE25E_F004;
/// After IRQ return (`[013Ch+8]`).
pub const LATCH_AFTER_IRQ: u32 = 0xE55E_C002;

/// User IRQ handler pointer the BIOS stub branches to.
pub const IRQ_HANDLER_PTR: u32 = 0x0300_7FFC;

/// HLE IRQ return sentinel (ARM `SUBS PC, LR, #4` epilogue lands here conceptually).
///
/// When PC redirects here under BiosHle, glue restores CPSR from SPSR and sets
/// [`LATCH_AFTER_IRQ`].
pub const HLE_IRQ_RETURN: u32 = 0x0000_0138;
