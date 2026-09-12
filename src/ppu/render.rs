//! Scanline compositor — modes 0–5 + OBJ — blend targets + windows.
//!
//! Cited: GBATEK — LCD Video / Layer Priority / Color Special Effects
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md` §12
//! Note: functional priority + blend; not cycle/dot accurate.

use super::bg::{affine_tile_pixel, text_pixel, BgPixel};
use super::bitmap::bitmap_pixel;
use super::blend::{
    apply_blend, green_swap_line, LAYER_BD, LAYER_BG0, LAYER_BG2, LAYER_BG3, LAYER_OBJ,
};
use super::obj::obj_pixel_at;
use super::regs::LcdRegs;
use super::window::enables_at;

#[inline]
fn backdrop(palette: &[u8]) -> u16 {
    let lo = u16::from(*palette.first().unwrap_or(&0));
    let hi = u16::from(*palette.get(1).unwrap_or(&0));
    lo | (hi << 8)
}

#[derive(Clone, Copy)]
struct Cand {
    prio: u8,
    is_obj: bool,
    layer: u8,
    color: u16,
    semi: bool,
}

impl Cand {
    #[inline]
    fn beats(self, other: Self) -> bool {
        self.prio < other.prio || (self.prio == other.prio && self.is_obj && !other.is_obj)
    }
}

/// Render one visible scanline into `out` (240 RGB555 pixels).
#[allow(clippy::too_many_arguments)] // compositor needs mem + affine + out
pub fn render_scanline(
    regs: &LcdRegs,
    line: u16,
    vram: &[u8],
    palette: &[u8],
    oam: &[u8],
    bg2_ref: (i32, i32),
    bg3_ref: (i32, i32),
    out: &mut [u16; 240],
) {
    if regs.forced_blank() {
        out.fill(0x7FFF); // white
        return;
    }

    let mode = regs.bg_mode();
    let bd = backdrop(palette);

    for x in 0..240u16 {
        let win = enables_at(regs, x, line, oam, vram);

        let mut top: Option<Cand> = None;
        let mut second: Option<Cand> = None;

        let consider = |top: &mut Option<Cand>, second: &mut Option<Cand>, c: Cand| match *top {
            None => *top = Some(c),
            Some(t) if c.beats(t) => {
                *second = Some(t);
                *top = Some(c);
            }
            Some(_) => match *second {
                None => *second = Some(c),
                Some(s) if c.beats(s) => *second = Some(c),
                _ => {}
            },
        };

        let put_bg =
            |top: &mut Option<Cand>, second: &mut Option<Cand>, pix: BgPixel, layer: u8| {
                if pix.transparent {
                    return;
                }
                consider(
                    top,
                    second,
                    Cand {
                        prio: pix.priority,
                        is_obj: false,
                        layer,
                        color: pix.color,
                        semi: false,
                    },
                );
            };

        match mode {
            0 => {
                for bg in 0..4 {
                    if regs.layer_enable(8 + bg as u16) && win.bg[bg] {
                        let pix = text_pixel(regs, bg, x, line, vram, palette);
                        put_bg(&mut top, &mut second, pix, LAYER_BG0 + bg as u8);
                    }
                }
            }
            1 => {
                for bg in 0..2 {
                    if regs.layer_enable(8 + bg as u16) && win.bg[bg] {
                        let pix = text_pixel(regs, bg, x, line, vram, palette);
                        put_bg(&mut top, &mut second, pix, LAYER_BG0 + bg as u8);
                    }
                }
                if regs.layer_enable(10) && win.bg[2] {
                    let pix = affine_tile_pixel(
                        regs,
                        2,
                        i32::from(x),
                        vram,
                        palette,
                        bg2_ref.0,
                        bg2_ref.1,
                    );
                    put_bg(&mut top, &mut second, pix, LAYER_BG2);
                }
            }
            2 => {
                for bg in 2..4 {
                    if regs.layer_enable(8 + bg as u16) && win.bg[bg] {
                        let r = if bg == 2 { bg2_ref } else { bg3_ref };
                        let pix =
                            affine_tile_pixel(regs, bg, i32::from(x), vram, palette, r.0, r.1);
                        let layer = if bg == 2 { LAYER_BG2 } else { LAYER_BG3 };
                        put_bg(&mut top, &mut second, pix, layer);
                    }
                }
            }
            3..=5 if regs.layer_enable(10) && win.bg[2] => {
                let pix = bitmap_pixel(
                    regs,
                    mode,
                    i32::from(x),
                    vram,
                    palette,
                    bg2_ref.0,
                    bg2_ref.1,
                );
                put_bg(&mut top, &mut second, pix, LAYER_BG2);
            }
            _ => {}
        }

        if win.obj {
            let obj = obj_pixel_at(regs, x, line, oam, vram, palette);
            if !obj.transparent {
                consider(
                    &mut top,
                    &mut second,
                    Cand {
                        prio: obj.priority,
                        is_obj: true,
                        layer: LAYER_OBJ,
                        color: obj.color,
                        semi: obj.semi_transparent,
                    },
                );
            }
        }

        let (pixel, top_layer, force_alpha) = match top {
            Some(t) => (t.color, t.layer, t.semi),
            None => (bd, LAYER_BD, false),
        };
        let bottom = match second {
            Some(s) => Some((s.color, s.layer)),
            None if top_layer != LAYER_BD => Some((bd, LAYER_BD)),
            None => None,
        };

        out[usize::from(x)] = apply_blend(regs, pixel, top_layer, bottom, win.blend, force_alpha);
    }

    green_swap_line(out, regs.greenswap & 1 != 0);
}
