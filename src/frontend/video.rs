//! RGB888 → RGBA8888 presentment helpers (host-only; core stays shade-agnostic).
//!
//! Cited: graycart-gba AGENTS.md (shade→RGB lives in the host)
//!   Project store: `docs/graycart-gba/AGENTS.md`
//! Note: GBA PPU emits RGB888; GB compat uses bridge shade→RGB then this expand.

use graycart_gba::ppu::{FB_HEIGHT, FB_WIDTH};

/// Native GBA LCD size (matches [`graycart_gba::ppu`]).
pub const SCREEN_WIDTH: usize = FB_WIDTH;
pub const SCREEN_HEIGHT: usize = FB_HEIGHT;

/// DMG/CGB panel size (matches graycart `SCREEN_*`).
pub const GB_SCREEN_WIDTH: usize = 160;
pub const GB_SCREEN_HEIGHT: usize = 144;

/// Default integer scale for the windowed host.
pub const SCALE: u32 = 3;

/// Expand packed RGB888 (`w*h*3`) into RGBA8888 (`w*h*4`) with opaque alpha.
pub fn rgb888_to_rgba(rgb: &[u8], rgba: &mut [u8], width: usize, height: usize) {
    assert_eq!(rgb.len(), width * height * 3);
    assert_eq!(rgba.len(), width * height * 4);
    for (i, chunk) in rgb.as_chunks::<3>().0.iter().enumerate() {
        let o = i * 4;
        rgba[o] = chunk[0];
        rgba[o + 1] = chunk[1];
        rgba[o + 2] = chunk[2];
        rgba[o + 3] = 255;
    }
}

/// Allocate an RGBA buffer and fill from RGB888 at the given geometry.
#[must_use]
pub fn rgb888_to_rgba_vec(rgb: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut rgba = vec![0u8; width * height * 4];
    rgb888_to_rgba(rgb, &mut rgba, width, height);
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
        let rgba = rgb888_to_rgba_vec(&rgb, SCREEN_WIDTH, SCREEN_HEIGHT);
        assert_eq!(&rgba[0..4], &[10, 20, 30, 255]);
        assert_eq!(rgba.len(), SCREEN_WIDTH * SCREEN_HEIGHT * 4);
    }

    #[test]
    fn gb_geometry_constants() {
        assert_eq!(GB_SCREEN_WIDTH * GB_SCREEN_HEIGHT, 160 * 144);
    }
}
