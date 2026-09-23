//! Mosaic. Screen-aligned sample origins for BG and OBJ.
//!
//! Cited: GBATEK LCD Video Controller, mosaic.
//! <https://problemkaputt.de/gbatek.htm>
//!
//! `MOSAIC` is a halfword. A field value of `N` means a block of `N + 1`
//! pixels (`0` => 1 pixel, no visible mosaic).
//!
//! - BG horizontal: bits 0–3
//! - BG vertical: bits 4–7
//! - OBJ horizontal: bits 8–11
//! - OBJ vertical: bits 12–15
//!
//! This module only computes the top-left sample coordinate of a pixel's
//! mosaic block. It does not draw.

#[cfg(test)]
mod tests;

/// BG mosaic sample origin for screen pixel `(x, y)`.
///
/// Uses BG size fields in the low byte of `mosaic`.
pub fn bg_origin(x: usize, y: usize, mosaic: u16) -> (usize, usize) {
    let h = mosaic & 0xF;
    let v = (mosaic >> 4) & 0xF;
    origin(x, y, h, v)
}

/// OBJ mosaic sample origin for screen pixel `(x, y)`.
///
/// Uses OBJ size fields in the high byte of `mosaic`.
pub fn obj_origin(x: usize, y: usize, mosaic: u16) -> (usize, usize) {
    let h = (mosaic >> 8) & 0xF;
    let v = (mosaic >> 12) & 0xF;
    origin(x, y, h, v)
}

fn origin(x: usize, y: usize, h: u16, v: u16) -> (usize, usize) {
    let block_h = (h + 1) as usize;
    let block_v = (v + 1) as usize;
    (x - x % block_h, y - y % block_v)
}
