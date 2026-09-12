//! Background fetch — text (Mode 0/1) + affine tile (Mode 1/2) — P4.
//!
//! Cited: GBATEK — BG Control / BG Text / BG Rotation
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md` §3
//! Note: mosaic = upper-left of block (size−1 fields); not cycle-perfect.

use super::affine::{latch_ref_8_8, sample_x, sample_y};
use super::regs::LcdRegs;
use super::vram_fetch;

/// Opaque RGB555 pixel or transparent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BgPixel {
    pub color: u16,
    pub priority: u8,
    pub transparent: bool,
}

impl BgPixel {
    pub const TRANSPARENT: Self = Self {
        color: 0,
        priority: 3,
        transparent: true,
    };
}

#[inline]
fn pal_color(palette: &[u8], index: usize) -> u16 {
    let lo = u16::from(*palette.get(index.saturating_mul(2)).unwrap_or(&0));
    let hi = u16::from(*palette.get(index.saturating_mul(2) + 1).unwrap_or(&0));
    lo | (hi << 8)
}

/// Screen size in tiles for text BGs.
fn text_screen_tiles(size: u16) -> (u16, u16) {
    match size & 3 {
        0 => (32, 32),
        1 => (64, 32),
        2 => (32, 64),
        _ => (64, 64),
    }
}

/// Char base / screen base helpers.
#[inline]
fn char_base(bgcnt: u16) -> usize {
    usize::from((bgcnt >> 2) & 3) * 0x4000
}

#[inline]
fn screen_base(bgcnt: u16) -> usize {
    usize::from((bgcnt >> 8) & 0x1F) * 0x800
}

#[inline]
fn is_8bpp(bgcnt: u16) -> bool {
    bgcnt & (1 << 7) != 0
}

#[inline]
fn priority(bgcnt: u16) -> u8 {
    (bgcnt & 3) as u8
}

#[inline]
fn mosaic_snap(coord: u16, size_minus_1: u16) -> u16 {
    let size = size_minus_1 + 1;
    coord - (coord % size)
}

/// Fetch one text-BG pixel at screen (x, y).
pub fn text_pixel(
    regs: &LcdRegs,
    bg: usize,
    x: u16,
    y: u16,
    vram: &[u8],
    palette: &[u8],
) -> BgPixel {
    let bgcnt = regs.bgcnt[bg];
    let prio = priority(bgcnt);
    let (mut sx, mut sy) = (x, y);
    if bgcnt & (1 << 6) != 0 {
        let mh = regs.mosaic & 0xF;
        let mv = (regs.mosaic >> 4) & 0xF;
        sx = mosaic_snap(sx, mh);
        sy = mosaic_snap(sy, mv);
    }
    let (sw, sh) = text_screen_tiles(bgcnt >> 14);
    let mut mx = (sx.wrapping_add(regs.bg_hofs[bg])) & 0x1FF;
    let mut my = (sy.wrapping_add(regs.bg_vofs[bg])) & 0x1FF;
    // Wrap within screen size in tiles*8.
    let pix_w = sw * 8;
    let pix_h = sh * 8;
    mx %= pix_w;
    my %= pix_h;

    let map_x = mx / 8;
    let map_y = my / 8;
    // Screenblock stitching for sizes 1–3.
    let (sb_x, sb_y, tx, ty) = match bgcnt >> 14 {
        0 => (0u16, 0u16, map_x, map_y),
        1 => (map_x / 32, 0, map_x % 32, map_y),
        2 => (0, map_y / 32, map_x, map_y % 32),
        _ => (map_x / 32, map_y / 32, map_x % 32, map_y % 32),
    };
    let screen_off = screen_base(bgcnt) + usize::from(sb_y * 2 + sb_x) * 0x800;
    let entry_off = screen_off + (usize::from(ty) * 32 + usize::from(tx)) * 2;
    let entry = vram_fetch::half(vram, entry_off);
    let tile = entry & 0x3FF;
    let hflip = entry & (1 << 10) != 0;
    let vflip = entry & (1 << 11) != 0;
    let pal_bank = (entry >> 12) & 0xF;

    let mut fx = mx % 8;
    let mut fy = my % 8;
    if hflip {
        fx = 7 - fx;
    }
    if vflip {
        fy = 7 - fy;
    }

    let char0 = char_base(bgcnt);
    if is_8bpp(bgcnt) {
        let tile_off = char0 + usize::from(tile) * 64 + usize::from(fy) * 8 + usize::from(fx);
        let idx = u16::from(vram_fetch::byte(vram, tile_off));
        if idx == 0 {
            return BgPixel {
                color: 0,
                priority: prio,
                transparent: true,
            };
        }
        BgPixel {
            color: pal_color(palette, usize::from(idx)),
            priority: prio,
            transparent: false,
        }
    } else {
        let tile_off = char0 + usize::from(tile) * 32 + usize::from(fy) * 4 + usize::from(fx / 2);
        let byte = vram_fetch::byte(vram, tile_off);
        let nibble = if fx & 1 == 0 { byte & 0xF } else { byte >> 4 };
        if nibble == 0 {
            return BgPixel {
                color: 0,
                priority: prio,
                transparent: true,
            };
        }
        let idx = usize::from(pal_bank) * 16 + usize::from(nibble);
        BgPixel {
            color: pal_color(palette, idx),
            priority: prio,
            transparent: false,
        }
    }
}

/// Affine tile BG (BG2/BG3 in modes 1–2): 8bpp map, wrap/transparent overflow.
pub fn affine_tile_pixel(
    regs: &LcdRegs,
    bg: usize,
    x: i32,
    vram: &[u8],
    palette: &[u8],
    internal_x: i32,
    internal_y: i32,
) -> BgPixel {
    let bgcnt = regs.bgcnt[bg];
    let prio = priority(bgcnt);
    let size = (bgcnt >> 14) & 3;
    let map_tiles: i32 = match size {
        0 => 16,
        1 => 32,
        2 => 64,
        _ => 128,
    };
    let (pa, pc) = if bg == 2 {
        (regs.bg2_pa, regs.bg2_pc)
    } else {
        (regs.bg3_pa, regs.bg3_pc)
    };
    // Mosaic snaps screen X before affine sample (coarse / common emulator model).
    let sx = if bgcnt & (1 << 6) != 0 {
        let mh = i32::from(regs.mosaic & 0xF) + 1;
        x - x.rem_euclid(mh)
    } else {
        x
    };
    let mut tx = sample_x(internal_x, pa, sx);
    let mut ty = sample_y(internal_y, pc, sx);
    let pix = map_tiles * 8;
    let wrap = bgcnt & (1 << 13) != 0;
    if tx < 0 || ty < 0 || tx >= pix || ty >= pix {
        if wrap {
            tx = tx.rem_euclid(pix);
            ty = ty.rem_euclid(pix);
        } else {
            return BgPixel::TRANSPARENT;
        }
    }
    let tile_x = (tx / 8) as u16;
    let tile_y = (ty / 8) as u16;
    let fx = (tx % 8) as u16;
    let fy = (ty % 8) as u16;
    let map_off = screen_base(bgcnt)
        + usize::from(tile_y) * usize::from(map_tiles as u16)
        + usize::from(tile_x);
    let tile = u16::from(vram_fetch::byte(vram, map_off));
    let char0 = char_base(bgcnt);
    let tile_off = char0 + usize::from(tile) * 64 + usize::from(fy) * 8 + usize::from(fx);
    let idx = u16::from(vram_fetch::byte(vram, tile_off));
    if idx == 0 {
        return BgPixel {
            color: 0,
            priority: prio,
            transparent: true,
        };
    }
    BgPixel {
        color: pal_color(palette, usize::from(idx)),
        priority: prio,
        transparent: false,
    }
}

/// Latch affine internal refs from registers (8.8 fixed).
#[must_use]
pub fn latch_affine_refs(regs: &LcdRegs, bg: usize) -> (i32, i32) {
    if bg == 2 {
        (latch_ref_8_8(regs.bg2_x), latch_ref_8_8(regs.bg2_y))
    } else {
        (latch_ref_8_8(regs.bg3_x), latch_ref_8_8(regs.bg3_y))
    }
}
