//! PPU framebuffer health heuristic gates (synthetic — no commercial ROMs).
//!
//! Cited: graycart-gb Debug Monitor health overview (spirit)
//!   https://github.com/graycart/graycart-gb (src/frontend/debug/ui/health.rs)
//! Cited: GBATEK — LCD dimensions / backdrop
//!   https://problemkaputt.de/gbatek.htm

use super::health::{
    analyze_framebuffer, backdrop_from_palette, blend_label, blend_mode, layer_enable_label,
    mosaic_active, objwin_active, FrameHealth, MONO_FRAME_PCT,
};
use super::regs::LcdRegs;
use super::{FB_HEIGHT, FB_WIDTH};

fn blank_fb(fill: u16) -> Vec<u16> {
    vec![fill; FB_WIDTH * FB_HEIGHT]
}

#[test]
fn all_black_framebuffer_trips_mono_heuristic() {
    let fb = blank_fb(0);
    let h = analyze_framebuffer(&fb, 0x7FFF);
    assert!(h.samples > 0);
    assert!(h.black_pct() >= MONO_FRAME_PCT);
    assert!(h.is_all_black());
    assert!(!h.is_backdrop_only());
}

#[test]
fn backdrop_only_non_black_trips_heuristic() {
    let bd = 0x001F; // non-zero blue
    let fb = blank_fb(bd);
    let h = analyze_framebuffer(&fb, bd);
    assert!(h.backdrop_pct() >= MONO_FRAME_PCT);
    assert!(h.is_backdrop_only());
    assert!(!h.is_all_black());
}

#[test]
fn mixed_frame_is_neither_mono_flag() {
    let mut fb = blank_fb(0);
    // Paint a stripe of non-black / non-backdrop pixels so black < 98%.
    for y in 0..FB_HEIGHT {
        for x in 0..(FB_WIDTH / 2) {
            fb[y * FB_WIDTH + x] = 0x03E0;
        }
    }
    let h = analyze_framebuffer(&fb, 0x7C00);
    assert!(!h.is_all_black());
    assert!(!h.is_backdrop_only());
    assert!(h.other > 0);
}

#[test]
fn short_buffer_returns_empty_health() {
    let h = analyze_framebuffer(&[0u16; 16], 0);
    assert_eq!(h, FrameHealth::default());
}

#[test]
fn backdrop_from_palette_reads_entry0() {
    let pal = [0x34u8, 0x12];
    assert_eq!(backdrop_from_palette(&pal), 0x1234);
    assert_eq!(backdrop_from_palette(&[]), 0);
}

#[test]
fn layer_blend_mosaic_objwin_labels() {
    let mut regs = LcdRegs {
        dispcnt: 0x1F00, // BG0–3 + OBJ
        ..LcdRegs::default()
    };
    assert_eq!(layer_enable_label(&regs), "BG0|BG1|BG2|BG3|OBJ");
    regs.bldcnt = 1 << 6; // alpha
    assert_eq!(blend_mode(&regs), 1);
    assert_eq!(blend_label(1), "alpha");
    regs.mosaic = 0x1111;
    assert!(mosaic_active(&regs));
    regs.dispcnt |= 1 << 15;
    assert!(objwin_active(&regs));
}
