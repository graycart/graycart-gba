//! OBJ / sprite basics — P4.
//!
//! Cited: GBATEK — OBJ Attributes / OAM
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md` §5
//! Note: one-line OAM staging / per-line cycle budgets are stretch; live OAM read.
//! Affine OBJ uses identity sample into the clip rect (full PA–PD OBJ later).

use super::regs::LcdRegs;

/// One composited OBJ sample.
#[derive(Debug, Clone, Copy)]
pub struct ObjPixel {
    pub color: u16,
    pub priority: u8,
    pub semi_transparent: bool,
    pub transparent: bool,
}

impl ObjPixel {
    pub const NONE: Self = Self {
        color: 0,
        priority: 3,
        semi_transparent: false,
        transparent: true,
    };
}

#[inline]
fn read16(mem: &[u8], off: usize) -> u16 {
    let lo = u16::from(*mem.get(off).unwrap_or(&0));
    let hi = u16::from(*mem.get(off + 1).unwrap_or(&0));
    lo | (hi << 8)
}

fn obj_size(shape: u16, size: u16) -> (u16, u16) {
    match (shape, size) {
        (0, 0) => (8, 8),
        (0, 1) => (16, 16),
        (0, 2) => (32, 32),
        (0, 3) => (64, 64),
        (1, 0) => (16, 8),
        (1, 1) => (32, 8),
        (1, 2) => (32, 16),
        (1, 3) => (64, 32),
        (2, 0) => (8, 16),
        (2, 1) => (8, 32),
        (2, 2) => (16, 32),
        (2, 3) => (32, 64),
        _ => (8, 8),
    }
}

/// Find top-most OBJ pixel at (x,y). Lower OAM index wins among equal priority.
pub fn obj_pixel_at(
    regs: &LcdRegs,
    x: u16,
    y: u16,
    oam: &[u8],
    vram: &[u8],
    palette: &[u8],
) -> ObjPixel {
    if !regs.layer_enable(12) {
        return ObjPixel::NONE;
    }
    let mode = regs.bg_mode();
    let obj_vram_base: usize = if mode >= 3 { 0x14000 } else { 0x10000 };
    let map_1d = regs.obj_1d_mapping();

    let mut best = ObjPixel::NONE;
    let mut best_idx = 128u8;

    for i in 0..128u8 {
        let base = usize::from(i) * 8;
        let attr0 = read16(oam, base);
        let attr1 = read16(oam, base + 2);
        let attr2 = read16(oam, base + 4);

        let obj_mode = (attr0 >> 8) & 3;
        // bits 8–9 encode affine / disable / double — GBATEK:
        // 00 = normal, 01 = affine, 10 = disable, 11 = affine double
        let aff_bits = (attr0 >> 8) & 3;
        if aff_bits == 2 {
            continue; // disabled
        }
        // OBJ window mode is in bits 10–11 of attr0 (= 2).
        let render_mode = (attr0 >> 10) & 3;
        if render_mode == 2 {
            continue; // window OBJ — not drawn
        }

        let shape = (attr0 >> 14) & 3;
        let size_code = (attr1 >> 14) & 3;
        let (w, h) = obj_size(shape, size_code);
        let is_affine = aff_bits == 1 || aff_bits == 3;
        let double = aff_bits == 3;
        let (draw_w, draw_h) = if double { (w * 2, h * 2) } else { (w, h) };

        let obj_y = attr0 & 0xFF;
        let y_start = if obj_y > 160 {
            obj_y as i16 - 256
        } else {
            obj_y as i16
        };
        let y_i = y as i16;
        if y_i < y_start || y_i >= y_start + draw_h as i16 {
            continue;
        }
        let obj_x_raw = attr1 & 0x1FF;
        let x_start = if obj_x_raw > 240 {
            obj_x_raw as i16 - 512
        } else {
            obj_x_raw as i16
        };
        let x_i = x as i16;
        if x_i < x_start || x_i >= x_start + draw_w as i16 {
            continue;
        }

        let mut lx = (x_i - x_start) as u16;
        let mut ly = (y_i - y_start) as u16;
        let hflip = !is_affine && attr1 & (1 << 12) != 0;
        let vflip = !is_affine && attr1 & (1 << 13) != 0;
        if hflip {
            lx = draw_w - 1 - lx;
        }
        if vflip {
            ly = draw_h - 1 - ly;
        }
        if double {
            lx = lx * w / draw_w;
            ly = ly * h / draw_h;
        }

        let tile_base = attr2 & 0x3FF;
        let prio = ((attr2 >> 10) & 3) as u8;
        let pal_bank = (attr2 >> 12) & 0xF;
        let bpp8 = attr0 & (1 << 13) != 0;

        if !best.transparent && (prio > best.priority || (prio == best.priority && i >= best_idx)) {
            continue;
        }

        let tile_x = lx / 8;
        let tile_y = ly / 8;
        let fx = lx % 8;
        let fy = ly % 8;

        let stride = if bpp8 {
            // 8bpp: each tile is 2 numbered slots.
            (w / 8) * 2
        } else {
            w / 8
        };
        let mut tile_num = if map_1d {
            tile_base
                .wrapping_add(tile_y * stride)
                .wrapping_add(tile_x * if bpp8 { 2 } else { 1 })
        } else {
            tile_base
                .wrapping_add(tile_y * 32)
                .wrapping_add(tile_x * if bpp8 { 2 } else { 1 })
        };

        if mode >= 3 {
            if tile_num < 512 {
                continue;
            }
            tile_num -= 512;
        }

        let color = if bpp8 {
            let off = obj_vram_base
                + usize::from(tile_num / 2) * 64
                + usize::from(fy) * 8
                + usize::from(fx);
            let idx = *vram.get(off).unwrap_or(&0);
            if idx == 0 {
                continue;
            }
            read16(palette, 0x200 + usize::from(idx) * 2)
        } else {
            let off = obj_vram_base
                + usize::from(tile_num) * 32
                + usize::from(fy) * 4
                + usize::from(fx / 2);
            let byte = *vram.get(off).unwrap_or(&0);
            let nibble = if fx & 1 == 0 { byte & 0xF } else { byte >> 4 };
            if nibble == 0 {
                continue;
            }
            let idx = usize::from(pal_bank) * 16 + usize::from(nibble);
            read16(palette, 0x200 + idx * 2)
        };

        best = ObjPixel {
            color,
            priority: prio,
            semi_transparent: render_mode == 1,
            transparent: false,
        };
        best_idx = i;
        let _ = obj_mode;
    }
    best
}
