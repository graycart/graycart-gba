//! CGB RGB555 → RGBA8888 for NativeCgb host presentation.

use graycart::{SCREEN_HEIGHT, SCREEN_WIDTH};

/// Expand a 5-bit channel to 8-bit (`(c << 3) | (c >> 2)`).
fn expand5(c: u8) -> u8 {
    (c << 3) | (c >> 2)
}

/// Convert a CGB RGB555 color (bit 15 ignored) to RGBA8888.
pub fn rgb555_to_rgba(color: u16) -> [u8; 4] {
    let r = expand5((color & 0x1F) as u8);
    let g = expand5(((color >> 5) & 0x1F) as u8);
    let b = expand5(((color >> 10) & 0x1F) as u8);
    [r, g, b, 255]
}

/// Pack CRAM little-endian bytes (low then high) into a `u16`.
#[cfg(test)]
pub fn rgb555_le_bytes_to_u16(lo: u8, hi: u8) -> u16 {
    u16::from_le_bytes([lo, hi])
}

/// Fill a 160×144 RGBA buffer from RGB555 pixels. Does not touch Shade palettes.
pub fn rgb555_framebuffer_to_rgba(pixels: &[u16], out: &mut [u8]) {
    debug_assert_eq!(pixels.len(), SCREEN_WIDTH * SCREEN_HEIGHT);
    debug_assert_eq!(out.len(), SCREEN_WIDTH * SCREEN_HEIGHT * 4);
    for (dst, &color) in out.as_chunks_mut::<4>().0.iter_mut().zip(pixels) {
        dst.copy_from_slice(&rgb555_to_rgba(color));
    }
}

#[cfg(test)]
mod tests;
