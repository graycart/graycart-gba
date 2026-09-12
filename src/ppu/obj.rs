//! OBJ / sprite basics — regular + affine PA–PD.
//!
//! Cited: GBATEK — OBJ Attributes / OAM / Affine
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md` §5
//! Note: one-line OAM staging / per-line cycle budgets are stretch; live OAM read.

use super::regs::LcdRegs;
use super::vram_fetch;

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

#[inline]
fn mosaic_snap(coord: u16, size_minus_1: u16) -> u16 {
    let size = size_minus_1 + 1;
    coord - (coord % size)
}

fn affine_params(oam: &[u8], group: u16) -> (i16, i16, i16, i16) {
    let base = usize::from(group) * 32;
    (
        read16(oam, base + 6) as i16,
        read16(oam, base + 14) as i16,
        read16(oam, base + 22) as i16,
        read16(oam, base + 30) as i16,
    )
}

/// Returns `Some(color)` when opaque. Pass `palette = None` to test coverage only.
#[allow(clippy::too_many_arguments)]
fn sample_obj_color(
    mode: u16,
    map_1d: bool,
    obj_vram_base: usize,
    tile_base: u16,
    bpp8: bool,
    pal_bank: u16,
    w: u16,
    lx: u16,
    ly: u16,
    vram: &[u8],
    palette: Option<&[u8]>,
) -> Option<u16> {
    let tile_x = lx / 8;
    let tile_y = ly / 8;
    let fx = lx % 8;
    let fy = ly % 8;

    let stride = if bpp8 { (w / 8) * 2 } else { w / 8 };
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
            return None;
        }
        tile_num -= 512;
    }

    if bpp8 {
        let off =
            obj_vram_base + usize::from(tile_num / 2) * 64 + usize::from(fy) * 8 + usize::from(fx);
        let idx = vram_fetch::byte(vram, off);
        if idx == 0 {
            return None;
        }
        match palette {
            Some(pal) => Some(read16(pal, 0x200 + usize::from(idx) * 2)),
            None => Some(0),
        }
    } else {
        let off =
            obj_vram_base + usize::from(tile_num) * 32 + usize::from(fy) * 4 + usize::from(fx / 2);
        let byte = vram_fetch::byte(vram, off);
        let nibble = if fx & 1 == 0 { byte & 0xF } else { byte >> 4 };
        if nibble == 0 {
            return None;
        }
        match palette {
            Some(pal) => {
                let idx = usize::from(pal_bank) * 16 + usize::from(nibble);
                Some(read16(pal, 0x200 + idx * 2))
            }
            None => Some(0),
        }
    }
}

/// True when a non-transparent OBJ-window-mode sprite covers (x,y).
#[must_use]
pub fn obj_window_covers(regs: &LcdRegs, x: u16, y: u16, oam: &[u8], vram: &[u8]) -> bool {
    if !regs.layer_enable(12) || !regs.layer_enable(15) {
        return false;
    }
    let mode = regs.bg_mode();
    let obj_vram_base: usize = if mode >= 3 { 0x14000 } else { 0x10000 };
    let map_1d = regs.obj_1d_mapping();
    let mh = (regs.mosaic >> 8) & 0xF;
    let mv = (regs.mosaic >> 12) & 0xF;

    for i in 0..128u8 {
        let base = usize::from(i) * 8;
        let attr0 = read16(oam, base);
        let attr1 = read16(oam, base + 2);
        let attr2 = read16(oam, base + 4);

        let aff_bits = (attr0 >> 8) & 3;
        if aff_bits == 2 {
            continue;
        }
        let render_mode = (attr0 >> 10) & 3;
        if render_mode != 2 {
            continue;
        }

        let shape = (attr0 >> 14) & 3;
        let size_code = (attr1 >> 14) & 3;
        let (w, h) = obj_size(shape, size_code);
        let is_affine = aff_bits == 1 || aff_bits == 3;
        let double = aff_bits == 3;
        let (draw_w, draw_h) = if double { (w * 2, h * 2) } else { (w, h) };

        let mut sx = x;
        let mut sy = y;
        if attr0 & (1 << 12) != 0 {
            sx = mosaic_snap(sx, mh);
            sy = mosaic_snap(sy, mv);
        }

        let obj_y = attr0 & 0xFF;
        let y_start = if obj_y > 160 {
            obj_y as i16 - 256
        } else {
            obj_y as i16
        };
        let y_i = sy as i16;
        if y_i < y_start || y_i >= y_start + draw_h as i16 {
            continue;
        }
        let obj_x_raw = attr1 & 0x1FF;
        let x_start = if obj_x_raw > 240 {
            obj_x_raw as i16 - 512
        } else {
            obj_x_raw as i16
        };
        let x_i = sx as i16;
        if x_i < x_start || x_i >= x_start + draw_w as i16 {
            continue;
        }

        let tile_base = attr2 & 0x3FF;
        let pal_bank = (attr2 >> 12) & 0xF;
        let bpp8 = attr0 & (1 << 13) != 0;

        let local = if is_affine {
            let group = (attr1 >> 9) & 0x1F;
            let (pa, pb, pc, pd) = affine_params(oam, group);
            let sx_c = i32::from(x_i - x_start) - i32::from(draw_w) / 2;
            let sy_c = i32::from(y_i - y_start) - i32::from(draw_h) / 2;
            let tex_x = (i32::from(pa) * sx_c + i32::from(pb) * sy_c + (i32::from(w) << 7)) >> 8;
            let tex_y = (i32::from(pc) * sx_c + i32::from(pd) * sy_c + (i32::from(h) << 7)) >> 8;
            if tex_x < 0 || tex_y < 0 || tex_x >= i32::from(w) || tex_y >= i32::from(h) {
                continue;
            }
            (tex_x as u16, tex_y as u16)
        } else {
            let mut lx = (x_i - x_start) as u16;
            let mut ly = (y_i - y_start) as u16;
            let hflip = attr1 & (1 << 12) != 0;
            let vflip = attr1 & (1 << 13) != 0;
            if hflip {
                lx = draw_w - 1 - lx;
            }
            if vflip {
                ly = draw_h - 1 - ly;
            }
            (lx, ly)
        };

        if sample_obj_color(
            mode,
            map_1d,
            obj_vram_base,
            tile_base,
            bpp8,
            pal_bank,
            w,
            local.0,
            local.1,
            vram,
            None,
        )
        .is_some()
        {
            return true;
        }
    }
    false
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
    let mh = (regs.mosaic >> 8) & 0xF;
    let mv = (regs.mosaic >> 12) & 0xF;

    let mut best = ObjPixel::NONE;
    let mut best_idx = 128u8;

    for i in 0..128u8 {
        let base = usize::from(i) * 8;
        let attr0 = read16(oam, base);
        let attr1 = read16(oam, base + 2);
        let attr2 = read16(oam, base + 4);

        let aff_bits = (attr0 >> 8) & 3;
        if aff_bits == 2 {
            continue; // disabled
        }
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

        let mut sx = x;
        let mut sy = y;
        if attr0 & (1 << 12) != 0 {
            sx = mosaic_snap(sx, mh);
            sy = mosaic_snap(sy, mv);
        }

        let obj_y = attr0 & 0xFF;
        let y_start = if obj_y > 160 {
            obj_y as i16 - 256
        } else {
            obj_y as i16
        };
        let y_i = sy as i16;
        if y_i < y_start || y_i >= y_start + draw_h as i16 {
            continue;
        }
        let obj_x_raw = attr1 & 0x1FF;
        let x_start = if obj_x_raw > 240 {
            obj_x_raw as i16 - 512
        } else {
            obj_x_raw as i16
        };
        let x_i = sx as i16;
        if x_i < x_start || x_i >= x_start + draw_w as i16 {
            continue;
        }

        let tile_base = attr2 & 0x3FF;
        let prio = ((attr2 >> 10) & 3) as u8;
        let pal_bank = (attr2 >> 12) & 0xF;
        let bpp8 = attr0 & (1 << 13) != 0;

        if !best.transparent && (prio > best.priority || (prio == best.priority && i >= best_idx)) {
            continue;
        }

        let local = if is_affine {
            let group = (attr1 >> 9) & 0x1F;
            let (pa, pb, pc, pd) = affine_params(oam, group);
            let sx_c = i32::from(x_i - x_start) - i32::from(draw_w) / 2;
            let sy_c = i32::from(y_i - y_start) - i32::from(draw_h) / 2;
            let tex_x = (i32::from(pa) * sx_c + i32::from(pb) * sy_c + (i32::from(w) << 7)) >> 8;
            let tex_y = (i32::from(pc) * sx_c + i32::from(pd) * sy_c + (i32::from(h) << 7)) >> 8;
            if tex_x < 0 || tex_y < 0 || tex_x >= i32::from(w) || tex_y >= i32::from(h) {
                continue;
            }
            (tex_x as u16, tex_y as u16)
        } else {
            let mut lx = (x_i - x_start) as u16;
            let mut ly = (y_i - y_start) as u16;
            let hflip = attr1 & (1 << 12) != 0;
            let vflip = attr1 & (1 << 13) != 0;
            if hflip {
                lx = draw_w - 1 - lx;
            }
            if vflip {
                ly = draw_h - 1 - ly;
            }
            (lx, ly)
        };

        let Some(color) = sample_obj_color(
            mode,
            map_1d,
            obj_vram_base,
            tile_base,
            bpp8,
            pal_bank,
            w,
            local.0,
            local.1,
            vram,
            Some(palette),
        ) else {
            continue;
        };

        best = ObjPixel {
            color,
            priority: prio,
            semi_transparent: render_mode == 1,
            transparent: false,
        };
        best_idx = i;
    }
    best
}
