//! Synthetic commercial-title PPU gaps — blend targets, mosaic, affine OBJ,
//! OBJWIN, VRAM mirrors, window wrap (X1>X2 / Y1>Y2), BG char past 64K
//! (no commercial ROMs).
//!
//! Cited: GBATEK — Color Special Effects / Window / Mosaic / OBJ Affine / VRAM
//!   https://problemkaputt.de/gbatek.htm
//! Cross-check: mGBA window wrap (`_breakWindow`), Tonc BG charblock limits
//! Research: Project store `docs/graycart-gba/03-ppu.md`

use super::bg::text_pixel;
use super::blend::{apply_blend, LAYER_BD, LAYER_BG0, LAYER_BG2, LAYER_OBJ};
use super::obj::obj_pixel_at;
use super::regs::LcdRegs;
use super::render::render_scanline;
use super::vram_fetch;
use super::window::{enables_at, region_at, WinRegion};

#[test]
fn blend_requires_1st_and_2nd_targets() {
    let mut regs = LcdRegs {
        bldcnt: (1 << 6) | (1 << LAYER_BG0) | (1 << (8 + LAYER_BD)), // alpha, BG0→BD
        bldalpha: 0x0010,                                            // EVA=16 EVB=0
        ..Default::default()
    };
    let top = 0x001F; // red
    let bot = 0x03E0; // green
                      // EVA=16 → keep top fully when EVB=0.
    let out = apply_blend(&regs, top, LAYER_BG0, Some((bot, LAYER_BD)), true, false);
    assert_eq!(out, top);

    // Top not a 1st target → no blend.
    regs.bldcnt = (1 << 6) | (1 << (8 + LAYER_BD));
    let out = apply_blend(&regs, top, LAYER_BG0, Some((bot, LAYER_BD)), true, false);
    assert_eq!(out, top);

    // 2nd not selected → no blend.
    regs.bldcnt = (1 << 6) | (1 << LAYER_BG0);
    let out = apply_blend(&regs, top, LAYER_BG0, Some((bot, LAYER_BD)), true, false);
    assert_eq!(out, top);
}

#[test]
fn blend_alpha_half_mix_with_backdrop() {
    let regs = LcdRegs {
        bldcnt: (1 << 6) | (1 << LAYER_BG0) | (1 << (8 + LAYER_BD)),
        bldalpha: 0x0808, // 8/16 + 8/16
        ..Default::default()
    };
    let out = apply_blend(
        &regs,
        0x001F,
        LAYER_BG0,
        Some((0x0000, LAYER_BD)),
        true,
        false,
    );
    assert_eq!(out & 0x1F, 0x0F);
}

#[test]
fn semi_transparent_obj_forces_alpha() {
    // BLDCNT says brighten, but semi-transparent OBJ must alpha-mix.
    let regs = LcdRegs {
        bldcnt: (2 << 6) | (1 << (8 + LAYER_BD)), // brighten; OBJ not listed as 1st
        bldalpha: 0x0808,
        bldy: 0x10,
        ..Default::default()
    };
    let out = apply_blend(
        &regs,
        0x001F,
        LAYER_OBJ,
        Some((0x0000, LAYER_BD)),
        true,
        true,
    );
    assert_eq!(out & 0x1F, 0x0F);
}

#[test]
fn text_mosaic_samples_block_ul() {
    let mut regs = LcdRegs {
        dispcnt: 1 << 8,
        mosaic: 0x0007, // BG H size−1 = 7 → 8px
        ..Default::default()
    };
    regs.bgcnt[0] = 0x0104 | (1 << 6); // mosaic on, charbase 1, screenbase 1
    let mut vram = vec![0u8; 96 * 1024];
    let mut palette = vec![0u8; 1024];
    palette[2] = 0x1F; // index 1 = red
                       // Map: tile 0 at (0,0)
    vram[0x800] = 0;
    vram[0x801] = 0;
    // Tile 0: left column opaque (nibble pattern 0x11 → both pixels index 1)
    for i in 0..32 {
        vram[0x4000 + i] = 0x11;
    }
    let a = text_pixel(&regs, 0, 0, 0, &vram, &palette);
    let b = text_pixel(&regs, 0, 7, 0, &vram, &palette);
    assert!(!a.transparent);
    assert_eq!(a.color, b.color);
}

#[test]
fn vram_mirror_maps_upper_32k_twice() {
    let mut vram = vec![0u8; 96 * 1024];
    vram[0x1_0000] = 0xAB;
    assert_eq!(vram_fetch::byte(&vram, 0x1_0000), 0xAB);
    assert_eq!(vram_fetch::byte(&vram, 0x1_8000), 0xAB);
    assert_eq!(vram_fetch::mirror_off(0x1_8000), 0x1_0000);
}

#[test]
fn affine_obj_identity_draws_center() {
    let regs = LcdRegs {
        dispcnt: 1 << 12,
        ..Default::default()
    };
    let mut oam = vec![0u8; 1024];
    let mut vram = vec![0u8; 96 * 1024];
    let mut palette = vec![0u8; 1024];
    palette[0x200 + 2] = 0x1F;
    // Attr0: Y=0, affine flag bit8, size 8x8
    oam[0] = 0;
    oam[1] = 0x01; // affine
    oam[2] = 0;
    oam[3] = 0; // X=0, affine group 0
    oam[4] = 0;
    oam[5] = 0;
    // PA=0x0100, PD=0x0100 identity in group 0
    oam[6] = 0x00;
    oam[7] = 0x01;
    oam[0x1E] = 0x00;
    oam[0x1F] = 0x01;
    for i in 0..32 {
        vram[0x10000 + i] = 0x11;
    }
    let pix = obj_pixel_at(&regs, 0, 0, &oam, &vram, &palette);
    assert!(!pix.transparent);
    assert_eq!(pix.color & 0x1F, 0x1F);
}

#[test]
fn objwin_region_uses_winout_high_nibble() {
    let regs = LcdRegs {
        // OBJ + OBJWIN; outside disables all; OBJWIN enables BG0+blend
        dispcnt: (1 << 12) | (1 << 15),
        winout: 0x21_00, // high: BG0+blend; low: nothing
        ..Default::default()
    };
    let mut oam = vec![0u8; 1024];
    let mut vram = vec![0u8; 96 * 1024];
    // Window-mode OBJ at (0,0) 8x8
    oam[0] = 0;
    oam[1] = 0x08; // mode = OBJ window (bits 10–11 = 2)
    oam[2] = 0;
    oam[3] = 0;
    oam[4] = 0;
    oam[5] = 0;
    for i in 0..32 {
        vram[0x10000 + i] = 0x11;
    }
    assert_eq!(region_at(&regs, 0, 0, &oam, &vram), WinRegion::ObjWin);
    let en = enables_at(&regs, 0, 0, &oam, &vram);
    assert!(en.bg[0]);
    assert!(en.blend);
    assert!(!en.obj);
    // Outside mask empty
    let en_out = enables_at(&regs, 100, 100, &oam, &vram);
    assert!(!en_out.bg[0]);
}

#[test]
fn win1_mode0_scanline_composites_without_panic() {
    // FireRed-like DISPCNT: mode0 + 1D OBJ + BG2 + BG3 + OBJ + WIN1
    let mut regs = LcdRegs {
        dispcnt: 0x5C40,
        winin: 0x3F3F,
        winout: 0x001F,
        win1_h: 0x00F0, // full width
        win1_v: 0x00A0, // full height
        bldcnt: (1 << 6) | (1 << LAYER_BG2) | (1 << (8 + LAYER_BD)),
        bldalpha: 0x1000,
        ..Default::default()
    };
    regs.bgcnt[2] = 0x0104;
    regs.bgcnt[3] = 0x0108;
    let mut out = [0u16; 240];
    let vram = vec![0u8; 96 * 1024];
    let palette = vec![0u8; 1024];
    let oam = vec![0u8; 1024];
    render_scanline(&regs, 0, &vram, &palette, &oam, (0, 0), (0, 0), &mut out);
    // Backdrop only — should be zero, not panic / white.
    assert!(out.iter().all(|&c| c == 0));
}

#[test]
fn win_x1_gt_x2_wraps_two_strips() {
    // Hardware / mGBA: X1>X2 → [X1,240) ∪ [0,X2), not “clamp X2=240 only”.
    let regs = LcdRegs {
        dispcnt: 1 << 13, // WIN0 only
        win0_h: 0xC8_28,  // X1=200, X2=40
        win0_v: 0x00_A0,  // full height
        winin: 0x0001,    // inside WIN0: BG0 only
        winout: 0x0000,   // outside: nothing
        ..Default::default()
    };
    let oam = [0u8; 1024];
    let vram = [0u8; 96 * 1024];
    assert_eq!(region_at(&regs, 0, 0, &oam, &vram), WinRegion::Win0);
    assert_eq!(region_at(&regs, 39, 0, &oam, &vram), WinRegion::Win0);
    assert_eq!(region_at(&regs, 40, 0, &oam, &vram), WinRegion::Outside);
    assert_eq!(region_at(&regs, 199, 0, &oam, &vram), WinRegion::Outside);
    assert_eq!(region_at(&regs, 200, 0, &oam, &vram), WinRegion::Win0);
    assert_eq!(region_at(&regs, 239, 0, &oam, &vram), WinRegion::Win0);
}

#[test]
fn win_y1_gt_y2_wraps_vertically() {
    let regs = LcdRegs {
        dispcnt: 1 << 13,
        win0_h: 0x00_F0,
        win0_v: 0x64_14, // Y1=100, Y2=20
        winin: 0x0001,
        winout: 0x0000,
        ..Default::default()
    };
    let oam = [0u8; 1024];
    let vram = [0u8; 96 * 1024];
    assert_eq!(region_at(&regs, 0, 0, &oam, &vram), WinRegion::Win0);
    assert_eq!(region_at(&regs, 0, 19, &oam, &vram), WinRegion::Win0);
    assert_eq!(region_at(&regs, 0, 20, &oam, &vram), WinRegion::Outside);
    assert_eq!(region_at(&regs, 0, 99, &oam, &vram), WinRegion::Outside);
    assert_eq!(region_at(&regs, 0, 100, &oam, &vram), WinRegion::Win0);
    assert_eq!(region_at(&regs, 0, 159, &oam, &vram), WinRegion::Win0);
}

#[test]
fn win0_over_win1_on_overlap_when_both_wrap() {
    // Mid-run FireRed-ish: both windows on; WIN0 wins on overlap.
    let regs = LcdRegs {
        dispcnt: (1 << 13) | (1 << 14),
        win0_h: 0xC8_28, // wrap strips
        win0_v: 0x00_A0,
        win1_h: 0x00_F0, // full width
        win1_v: 0x00_A0,
        winin: 0x1F_01, // WIN0: BG0; WIN1: BG0–3+OBJ
        winout: 0x0000,
        ..Default::default()
    };
    let oam = [0u8; 1024];
    let vram = [0u8; 96 * 1024];
    assert_eq!(region_at(&regs, 10, 0, &oam, &vram), WinRegion::Win0);
    assert_eq!(region_at(&regs, 100, 0, &oam, &vram), WinRegion::Win1);
    let en0 = enables_at(&regs, 10, 0, &oam, &vram);
    assert!(en0.bg[0] && !en0.obj);
    let en1 = enables_at(&regs, 100, 0, &oam, &vram);
    assert!(en1.obj);
}

#[test]
fn bg_char_fetch_past_64k_is_transparent() {
    // Tonc / hardware: BG tiles cannot read OBJ charblocks (VRAM ≥ 0x10000).
    let mut regs = LcdRegs {
        dispcnt: 1 << 8,
        ..Default::default()
    };
    // charbase 3 (0xC000) + tile 512*32 = 0x10000 for 4bpp.
    regs.bgcnt[0] = 0x000C; // charbase 3, screenbase 0
    let mut vram = vec![0u8; 96 * 1024];
    let mut palette = vec![0u8; 1024];
    palette[2] = 0x1F;
    // Map entry: tile 512 at (0,0)
    vram[0] = 0x00;
    vram[1] = 0x02; // tile = 0x200
                    // Opaque garbage in OBJ VRAM — hardware must not sample this as a BG tile.
    for i in 0..32 {
        vram[0x10000 + i] = 0x11;
    }
    let pix = text_pixel(&regs, 0, 0, 0, &vram, &palette);
    assert!(pix.transparent, "BG char past 64K must not sample OBJ VRAM");
}
