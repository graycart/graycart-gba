//! Scanline compositor — modes 0–5 + OBJ — P4.
//!
//! Cited: GBATEK — LCD Video / Layer Priority
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md` §12
//! Note: functional priority + blend; not cycle/dot accurate.

use super::bg::{affine_tile_pixel, text_pixel, BgPixel};
use super::bitmap::bitmap_pixel;
use super::blend::{apply_blend, green_swap_line};
use super::obj::obj_pixel_at;
use super::regs::LcdRegs;
use super::window::enables_at;

#[inline]
fn backdrop(palette: &[u8]) -> u16 {
    let lo = u16::from(*palette.first().unwrap_or(&0));
    let hi = u16::from(*palette.get(1).unwrap_or(&0));
    lo | (hi << 8)
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
        let win = enables_at(regs, x, line);

        // Collect BG candidates enabled for this mode + window.
        let mut layers: [(u8, u16, bool); 5] = [(3, 0, true); 5]; // prio, color, transparent
                                                                  // index 0–3 = BG, 4 = OBJ placeholder filled below

        let put_bg = |layers: &mut [(u8, u16, bool); 5], bg: usize, pix: BgPixel, win_on: bool| {
            if !win_on || pix.transparent {
                return;
            }
            layers[bg] = (pix.priority, pix.color, false);
        };

        match mode {
            0 => {
                for bg in 0..4 {
                    if regs.layer_enable(8 + bg as u16) && win.bg[bg] {
                        let pix = text_pixel(regs, bg, x, line, vram, palette);
                        put_bg(&mut layers, bg, pix, true);
                    }
                }
            }
            1 => {
                for bg in 0..2 {
                    if regs.layer_enable(8 + bg as u16) && win.bg[bg] {
                        let pix = text_pixel(regs, bg, x, line, vram, palette);
                        put_bg(&mut layers, bg, pix, true);
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
                    put_bg(&mut layers, 2, pix, true);
                }
            }
            2 => {
                for bg in 2..4 {
                    if regs.layer_enable(8 + bg as u16) && win.bg[bg] {
                        let r = if bg == 2 { bg2_ref } else { bg3_ref };
                        let pix =
                            affine_tile_pixel(regs, bg, i32::from(x), vram, palette, r.0, r.1);
                        put_bg(&mut layers, bg, pix, true);
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
                put_bg(&mut layers, 2, pix, true);
            }
            _ => {}
        }

        let obj = if win.obj {
            obj_pixel_at(regs, x, line, oam, vram, palette)
        } else {
            super::obj::ObjPixel::NONE
        };

        // Pick top-most by priority (0 highest). OBJ wins ties vs BG.
        // Build sorted candidates: (prio, is_obj, color)
        let mut top: Option<(u8, bool, u16)> = None;
        let mut second: Option<u16> = None;

        let consider = |top: &mut Option<(u8, bool, u16)>,
                        second: &mut Option<u16>,
                        prio: u8,
                        is_obj: bool,
                        color: u16| {
            match *top {
                None => *top = Some((prio, is_obj, color)),
                Some((tp, _, tc)) => {
                    let wins = prio < tp || (prio == tp && is_obj);
                    if wins {
                        *second = Some(tc);
                        *top = Some((prio, is_obj, color));
                    } else if second.is_none() {
                        *second = Some(color);
                    }
                }
            }
        };

        for (p, c, tr) in &layers {
            if !*tr {
                consider(&mut top, &mut second, *p, false, *c);
            }
        }
        if !obj.transparent {
            consider(&mut top, &mut second, obj.priority, true, obj.color);
        }

        let (pixel, blend_top_is_obj_semi) = match top {
            Some((_, is_obj, c)) => (c, is_obj && obj.semi_transparent),
            None => (bd, false),
        };

        let out_c = if blend_top_is_obj_semi {
            // Semi-transparent OBJ forces alpha vs next.
            apply_blend(regs, pixel, second.or(Some(bd)), win.blend)
        } else {
            apply_blend(regs, pixel, second, win.blend)
        };
        out[usize::from(x)] = out_c;
    }

    green_swap_line(out, regs.greenswap & 1 != 0);
}
