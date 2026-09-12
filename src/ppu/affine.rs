//! Affine fixed-point helpers (BG2/BG3 + OBJ) — P4 functional.
//!
//! Cited: GBATEK — BG Rotation/Scaling
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md` §4
//! Note: mid-frame ref write updates internal immediately (Mode-7 style).
//! Internals stored as signed 8.8 (same as BGxX low precision).

/// Sign-extend 28-bit BGxX/BGxY write latch to i32 (keeps 8 frac bits).
#[inline]
pub fn latch_ref_8_8(raw: u32) -> i32 {
    ((raw as i32) << 4) >> 4
}

/// Sample transformed coordinates for screen pixel `x` on the current line.
/// `ix`/`iy` are line internals (8.8); `pa`/`pc` are 8.8.
#[inline]
pub fn sample_x(ix: i32, pa: i16, x: i32) -> i32 {
    (ix + i32::from(pa) * x) >> 8
}

#[inline]
pub fn sample_y(iy: i32, pc: i16, x: i32) -> i32 {
    (iy + i32::from(pc) * x) >> 8
}
