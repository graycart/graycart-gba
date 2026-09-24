//! Host-side framebuffer → RGBA8888 (no window / GPU).

use graycart::{Framebuffer, SCREEN_HEIGHT, SCREEN_WIDTH, Shade};

const GBA_WIDTH: usize = 240;
const GBA_HEIGHT: usize = 160;

const CLASSIC_DMG_RGBA: [[u8; 4]; 4] = [
    [0x9B, 0xBC, 0x0F, 255],
    [0x8B, 0xAC, 0x0F, 255],
    [0x30, 0x62, 0x30, 255],
    [0x0F, 0x38, 0x0F, 255],
];

fn expand5(c: u8) -> u8 {
    (c << 3) | (c >> 2)
}

/// GBA-hosted CGB 5-bit channel curve: `round(31 * (i/31)^1.7)` for `i` in 0..=31.
/// Table so 0→0, 16→10, 31→31 are exact without runtime `powf`.
const CGB_CHANNEL: [u8; 32] = [
    0, 0, 0, 1, 1, 1, 2, 2, 3, 4, 5, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15, 16, 17, 19, 20, 22, 23, 25,
    26, 28, 29, 31,
];

/// Map a 5-bit CGB channel through the GBA brightness curve (indexes above 31 clamp to 31).
pub fn cgb_channel(c5: u8) -> u8 {
    CGB_CHANNEL[usize::from(c5.min(31))]
}

/// GBA BGR555 (bits 0–4 blue, 5–9 green, 10–14 red) → RGBA8888.
/// Does not apply [`cgb_channel`]; native GBA stays linear expand.
pub fn bgr555_to_rgba(color: u16) -> [u8; 4] {
    let b = expand5((color & 0x1F) as u8);
    let g = expand5(((color >> 5) & 0x1F) as u8);
    let r = expand5(((color >> 10) & 0x1F) as u8);
    [r, g, b, 255]
}

fn rgb555_to_rgba(color: u16) -> [u8; 4] {
    let r = expand5(cgb_channel((color & 0x1F) as u8));
    let g = expand5(cgb_channel(((color >> 5) & 0x1F) as u8));
    let b = expand5(cgb_channel(((color >> 10) & 0x1F) as u8));
    [r, g, b, 255]
}

/// Classic DMG shade palette (index 0 lightest → 3 darkest).
pub fn shade_to_rgba(shade: Shade) -> [u8; 4] {
    CLASSIC_DMG_RGBA[shade.index() as usize]
}

/// 240×160 GBA pixels → RGBA. Writes `min(pixels.len(), 240×160, out.len()/4)` pixels.
pub fn gba_framebuffer_to_rgba(pixels: &[u16], out: &mut [u8]) {
    let cap = out.len() / 4;
    let limit = pixels.len().min(GBA_WIDTH * GBA_HEIGHT).min(cap);
    for (dst, &color) in out
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .take(limit)
        .zip(pixels.iter().take(limit))
    {
        dst.copy_from_slice(&bgr555_to_rgba(color));
    }
}

/// 160×144 SM83 framebuffer → RGBA. Writes `min(160×144, out.len()/4)` pixels.
pub fn sm83_framebuffer_to_rgba(fb: &Framebuffer, out: &mut [u8]) {
    let cap = out.len() / 4;
    let limit = (SCREEN_WIDTH * SCREEN_HEIGHT).min(cap);
    if fb.presents_cgb_color() {
        let rgb = fb.rgb555_pixels();
        for (dst, &color) in out
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .take(limit)
            .zip(rgb.iter().take(limit))
        {
            dst.copy_from_slice(&rgb555_to_rgba(color));
        }
    } else {
        let shades = fb.pixels();
        for (dst, &shade) in out
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .take(limit)
            .zip(shades.iter().take(limit))
        {
            dst.copy_from_slice(&shade_to_rgba(shade));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cgb_mid_channel_is_darker_than_linear() {
        assert_eq!(cgb_channel(0), 0);
        assert_eq!(cgb_channel(31), 31);
        assert_eq!(cgb_channel(16), 10);
        for i in 0..31 {
            assert!(
                cgb_channel(i) <= cgb_channel(i + 1),
                "cgb_channel not monotonic at {i}"
            );
        }
    }

    #[test]
    fn sm83_cgb_mid_channel_uses_curve_not_linear() {
        let mut fb = Framebuffer::new();
        fb.set_presents_cgb_color(true);
        // RGB555 red = 16 (linear expand would be 132; curve maps 16→10 → expand 82).
        fb.set_cgb_pixel(0, 0, Shade::Darkest, 0x0010);
        let mut out = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
        sm83_framebuffer_to_rgba(&fb, &mut out);
        assert_eq!(&out[0..4], &[82, 0, 0, 255]);
        assert_ne!(&out[0..4], &[132, 0, 0, 255]);
    }

    #[test]
    fn gba_bgr555_mid_blue_stays_linear() {
        // BGR555 blue = 16 → linear expand 132, not the CGB curve.
        assert_eq!(bgr555_to_rgba(0x0010), [0, 0, 132, 255]);
    }

    #[test]
    fn bgr555_white_is_full_rgba() {
        assert_eq!(bgr555_to_rgba(0x7FFF), [255, 255, 255, 255]);
    }

    #[test]
    fn bgr555_red_channel() {
        assert_eq!(bgr555_to_rgba(0x7C00), [255, 0, 0, 255]);
    }

    #[test]
    fn bgr555_blue_channel() {
        assert_eq!(bgr555_to_rgba(0x001F), [0, 0, 255, 255]);
    }

    #[test]
    fn classic_dmg_shade_corners() {
        assert_eq!(shade_to_rgba(Shade::Lightest), [0x9B, 0xBC, 0x0F, 255]);
        assert_eq!(shade_to_rgba(Shade::Darkest), [0x0F, 0x38, 0x0F, 255]);
    }

    #[test]
    fn sm83_dmg_framebuffer_pixel_zero() {
        let fb = Framebuffer::new();
        assert!(!fb.presents_cgb_color());
        let mut out = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
        sm83_framebuffer_to_rgba(&fb, &mut out);
        assert_eq!(&out[0..4], &[0x9B, 0xBC, 0x0F, 255]);
    }

    #[test]
    fn sm83_cgb_rgb555_at_pixel_zero() {
        let mut fb = Framebuffer::new();
        fb.set_presents_cgb_color(true);
        fb.set_cgb_pixel(0, 0, Shade::Darkest, 0x001F);
        let mut out = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
        sm83_framebuffer_to_rgba(&fb, &mut out);
        assert_eq!(&out[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn gba_framebuffer_respects_short_output() {
        let pixels = [0x7FFFu16];
        let mut out = [0u8; 8];
        gba_framebuffer_to_rgba(&pixels, &mut out);
        assert_eq!(&out[0..4], &[255, 255, 255, 255]);
        assert_eq!(&out[4..8], &[0, 0, 0, 0]);
    }

    #[test]
    fn sm83_framebuffer_respects_short_output() {
        let fb = Framebuffer::new();
        let mut out = [0u8; 4];
        sm83_framebuffer_to_rgba(&fb, &mut out);
        assert_eq!(&out, &[0x9B, 0xBC, 0x0F, 255]);
    }
}
