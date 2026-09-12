//! RGB888 → RGBA8888 presentment helpers (host-only; core stays shade-agnostic).
//!
//! Cited: graycart-gba AGENTS.md (shade→RGB lives in the host)
//!   Project store: `docs/graycart-gba/AGENTS.md`
//! Note: GBA PPU already emits RGB888; host only expands alpha for egui textures.

use graycart_gba::ppu::{FB_HEIGHT, FB_WIDTH};

/// Visible LCD size (matches [`graycart_gba::ppu`]).
pub const SCREEN_WIDTH: usize = FB_WIDTH;
pub const SCREEN_HEIGHT: usize = FB_HEIGHT;

/// Default integer scale for the windowed host.
pub const SCALE: u32 = 3;

/// Expand packed RGB888 (`w*h*3`) into RGBA8888 (`w*h*4`) with opaque alpha.
pub fn rgb888_to_rgba(rgb: &[u8], rgba: &mut [u8]) {
    assert_eq!(rgb.len(), SCREEN_WIDTH * SCREEN_HEIGHT * 3);
    assert_eq!(rgba.len(), SCREEN_WIDTH * SCREEN_HEIGHT * 4);
    for (i, chunk) in rgb.as_chunks::<3>().0.iter().enumerate() {
        let o = i * 4;
        rgba[o] = chunk[0];
        rgba[o + 1] = chunk[1];
        rgba[o + 2] = chunk[2];
        rgba[o + 3] = 255;
    }
}

/// Allocate an RGBA buffer and fill from RGB888.
#[must_use]
pub fn rgb888_to_rgba_vec(rgb: &[u8]) -> Vec<u8> {
    let mut rgba = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
    rgb888_to_rgba(rgb, &mut rgba);
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb888_to_rgba_sets_opaque_alpha() {
        let mut rgb = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 3];
        rgb[0] = 10;
        rgb[1] = 20;
        rgb[2] = 30;
        let rgba = rgb888_to_rgba_vec(&rgb);
        assert_eq!(&rgba[0..4], &[10, 20, 30, 255]);
        assert_eq!(rgba.len(), SCREEN_WIDTH * SCREEN_HEIGHT * 4);
    }
}
