//! BIOS LLE map + HLE SWI table placeholder (P7).
//!
//! Module layout from graycart-gba implementation plan §2.2.
//! Behavior: see research `docs/graycart-gba/06-cart-bios-saves.md` (not implemented).
//! User-supplied BIOS only — never commit BIOS images.

/// Stub BIOS glue (`BiosHle` / `BiosLle` later).
#[derive(Debug, Default)]
pub struct Bios;
