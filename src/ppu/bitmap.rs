//! Bitmap backgrounds (modes 3–5) — P4.
//!
//! Cited: GBATEK — BG Modes Summary / Bitmap BG Modes
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md` §2–3.4

use super::affine::{latch_ref_8_8, sample_x, sample_y};
use super::bg::BgPixel;
use super::regs::LcdRegs;

#[inline]
fn read16(mem: &[u8], off: usize) -> u16 {
    let lo = u16::from(*mem.get(off).unwrap_or(&0));
    let hi = u16::from(*mem.get(off + 1).unwrap_or(&0));
    lo | (hi << 8)
}

/// Sample bitmap BG2 at screen pixel using affine refs (identity = 1:1).
pub fn bitmap_pixel(
    regs: &LcdRegs,
    mode: u16,
    x: i32,
    vram: &[u8],
    palette: &[u8],
    internal_x: i32,
    internal_y: i32,
) -> BgPixel {
    let prio = (regs.bgcnt[2] & 3) as u8;
    let tx = sample_x(internal_x, regs.bg2_pa, x);
    let ty = sample_y(internal_y, regs.bg2_pc, x);

    match mode {
        3 => {
            if !(0..240).contains(&tx) || !(0..160).contains(&ty) {
                return BgPixel::TRANSPARENT;
            }
            let off = (ty as usize) * 240 * 2 + (tx as usize) * 2;
            BgPixel {
                color: read16(vram, off),
                priority: prio,
                transparent: false,
            }
        }
        4 => {
            if !(0..240).contains(&tx) || !(0..160).contains(&ty) {
                return BgPixel::TRANSPARENT;
            }
            let base = if regs.frame_select() { 0xA000 } else { 0 };
            let off = base + (ty as usize) * 240 + (tx as usize);
            let idx = u16::from(*vram.get(off).unwrap_or(&0));
            if idx == 0 {
                return BgPixel {
                    color: 0,
                    priority: prio,
                    transparent: true,
                };
            }
            let color = read16(palette, usize::from(idx) * 2);
            BgPixel {
                color,
                priority: prio,
                transparent: false,
            }
        }
        5 => {
            // 160×128 RGB555, two frames.
            if !(0..160).contains(&tx) || !(0..128).contains(&ty) {
                return BgPixel::TRANSPARENT;
            }
            let base = if regs.frame_select() { 0xA000 } else { 0 };
            let off = base + (ty as usize) * 160 * 2 + (tx as usize) * 2;
            BgPixel {
                color: read16(vram, off),
                priority: prio,
                transparent: false,
            }
        }
        _ => BgPixel::TRANSPARENT,
    }
}

#[must_use]
pub fn latch_bg2(regs: &LcdRegs) -> (i32, i32) {
    (latch_ref_8_8(regs.bg2_x), latch_ref_8_8(regs.bg2_y))
}
