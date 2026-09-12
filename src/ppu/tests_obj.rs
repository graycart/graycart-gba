//! G4-obj unit gates.
//!
//! Cited: GBATEK — OBJ Attributes
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md` §5

use super::obj::obj_pixel_at;
use super::regs::LcdRegs;

#[test]
fn regular_8x8_obj_draws() {
    let regs = LcdRegs {
        dispcnt: 1 << 12,
        ..Default::default()
    };
    let mut oam = vec![0u8; 1024];
    let mut vram = vec![0u8; 96 * 1024];
    let mut palette = vec![0u8; 1024];
    palette[0x200 + 2] = 0x1F;
    oam[0] = 0;
    oam[1] = 0;
    oam[2] = 0;
    oam[3] = 0;
    oam[4] = 0;
    oam[5] = 0;
    for i in 0..32 {
        vram[0x10000 + i] = 0x11;
    }
    let pix = obj_pixel_at(&regs, 0, 0, &oam, &vram, &palette);
    assert!(!pix.transparent);
    assert_eq!(pix.color & 0x1F, 0x1F);
}

#[test]
fn obj_disabled_when_layer_off() {
    let regs = LcdRegs::default();
    let pix = obj_pixel_at(&regs, 0, 0, &[0; 1024], &[0; 96 * 1024], &[0; 1024]);
    assert!(pix.transparent);
}

#[test]
fn bitmap_mode_rejects_low_tiles() {
    let regs = LcdRegs {
        dispcnt: 3 | (1 << 12),
        ..Default::default()
    };
    let mut oam = vec![0u8; 1024];
    oam[4] = 0;
    oam[5] = 0;
    let pix = obj_pixel_at(&regs, 0, 0, &oam, &[0; 96 * 1024], &[0; 1024]);
    assert!(pix.transparent);
}
