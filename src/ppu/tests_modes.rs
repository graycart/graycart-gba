//! G4-modes unit gates (bitmap + text smoke).
//!
//! Cited: GBATEK — BG Modes / Bitmap BG
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md` §2–3

use super::bg::text_pixel;
use super::bitmap::bitmap_pixel;
use super::regs::LcdRegs;
use super::render::render_scanline;
use super::Ppu;
use crate::bus::Bus;
use crate::irq::Irq;

#[test]
fn mode4_identity_samples_vram() {
    let regs = LcdRegs {
        dispcnt: 4 | (1 << 10),
        bg2_pa: 0x0100,
        bg2_pd: 0x0100,
        ..Default::default()
    };
    let mut vram = vec![0u8; 96 * 1024];
    let mut palette = vec![0u8; 1024];
    palette[2] = 0x1F;
    palette[3] = 0x00;
    vram[0] = 1;
    let pix = bitmap_pixel(&regs, 4, 0, &vram, &palette, 0, 0);
    assert!(!pix.transparent);
    assert_eq!(pix.color & 0x1F, 0x1F);
}

#[test]
fn mode3_direct_color() {
    let regs = LcdRegs {
        dispcnt: 3 | (1 << 10),
        bg2_pa: 0x0100,
        bg2_pd: 0x0100,
        ..Default::default()
    };
    let mut vram = vec![0u8; 96 * 1024];
    vram[0] = 0xE0;
    vram[1] = 0x03;
    let pix = bitmap_pixel(&regs, 3, 0, &vram, &[], 0, 0);
    assert!(!pix.transparent);
    assert_eq!(pix.color, 0x03E0);
}

#[test]
fn forced_blank_is_white() {
    let regs = LcdRegs {
        dispcnt: 1 << 7,
        ..Default::default()
    };
    let mut out = [0u16; 240];
    render_scanline(&regs, 0, &[], &[], &[], (0, 0), (0, 0), &mut out);
    assert!(out.iter().all(|&c| c == 0x7FFF));
}

#[test]
fn mode0_text_tile_smoke() {
    let mut regs = LcdRegs {
        dispcnt: 1 << 8,
        ..Default::default()
    };
    regs.bgcnt[0] = 0x0104;
    let mut vram = vec![0u8; 96 * 1024];
    let mut palette = vec![0u8; 1024];
    palette[2] = 0x1F;
    vram[0x800] = 0;
    vram[0x801] = 0;
    for i in 0..32 {
        vram[0x4000 + i] = 0x11;
    }
    let pix = text_pixel(&regs, 0, 0, 0, &vram, &palette);
    assert!(!pix.transparent);
    assert_eq!(pix.color & 0x1F, 0x1F);
}

#[test]
fn ppu_step_renders_mode4_line() {
    let mut ppu = Ppu::new();
    let mut bus = Bus::default();
    let mut irq = Irq::default();
    ppu.regs.dispcnt = 4 | (1 << 10);
    ppu.regs.bg2_pa = 0x0100;
    ppu.regs.bg2_pd = 0x0100;
    bus.palette[2] = 0x1F;
    bus.vram[0] = 1;
    ppu.step(super::timing::HDRAW_CYCLES, &bus, &mut irq);
    assert_ne!(ppu.fb[0], 0);
}
