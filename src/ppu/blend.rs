//! Color special effects (alpha / brightness) — P4 functional + target layers.
//!
//! Cited: GBATEK — Color Special Effects
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md` §7
//! Note: semi-transparent OBJ forces alpha + 1st-target (overrides BLDCNT 4 / 6–7).

use super::regs::LcdRegs;

/// BLDCNT layer indices: BG0–3, OBJ, Backdrop.
pub const LAYER_BG0: u8 = 0;
pub const LAYER_BG1: u8 = 1;
pub const LAYER_BG2: u8 = 2;
pub const LAYER_BG3: u8 = 3;
pub const LAYER_OBJ: u8 = 4;
pub const LAYER_BD: u8 = 5;

#[inline]
fn clamp5(v: i32) -> u16 {
    v.clamp(0, 31) as u16
}

#[inline]
pub fn rgb555_channels(c: u16) -> (u16, u16, u16) {
    (c & 0x1F, (c >> 5) & 0x1F, (c >> 10) & 0x1F)
}

#[inline]
pub fn pack_rgb555(r: u16, g: u16, b: u16) -> u16 {
    (r & 0x1F) | ((g & 0x1F) << 5) | ((b & 0x1F) << 10)
}

#[inline]
fn is_1st_target(bldcnt: u16, layer: u8) -> bool {
    bldcnt & (1 << layer) != 0
}

#[inline]
fn is_2nd_target(bldcnt: u16, layer: u8) -> bool {
    bldcnt & (1 << (8 + layer)) != 0
}

/// Apply BLDCNT when `blend_ok` (window allows).
///
/// `force_alpha`: semi-transparent OBJ — always 1st target + alpha mode.
#[must_use]
pub fn apply_blend(
    regs: &LcdRegs,
    top: u16,
    top_layer: u8,
    bottom: Option<(u16, u8)>,
    blend_ok: bool,
    force_alpha: bool,
) -> u16 {
    if !blend_ok {
        return top;
    }
    let mut mode = (regs.bldcnt >> 6) & 3;
    if force_alpha {
        mode = 1;
    }
    if mode == 0 {
        return top;
    }
    if !force_alpha && !is_1st_target(regs.bldcnt, top_layer) {
        return top;
    }

    let (r1, g1, b1) = rgb555_channels(top);
    match mode {
        1 => {
            let Some((bot, bot_layer)) = bottom else {
                return top;
            };
            if !is_2nd_target(regs.bldcnt, bot_layer) {
                return top;
            }
            let eva = (regs.bldalpha & 0x1F).min(16);
            let evb = ((regs.bldalpha >> 8) & 0x1F).min(16);
            let (r2, g2, b2) = rgb555_channels(bot);
            pack_rgb555(
                clamp5((i32::from(r1) * i32::from(eva) + i32::from(r2) * i32::from(evb)) / 16),
                clamp5((i32::from(g1) * i32::from(eva) + i32::from(g2) * i32::from(evb)) / 16),
                clamp5((i32::from(b1) * i32::from(eva) + i32::from(b2) * i32::from(evb)) / 16),
            )
        }
        2 => {
            let evy = (regs.bldy & 0x1F).min(16);
            pack_rgb555(
                clamp5(i32::from(r1) + (31 - i32::from(r1)) * i32::from(evy) / 16),
                clamp5(i32::from(g1) + (31 - i32::from(g1)) * i32::from(evy) / 16),
                clamp5(i32::from(b1) + (31 - i32::from(b1)) * i32::from(evy) / 16),
            )
        }
        3 => {
            let evy = (regs.bldy & 0x1F).min(16);
            pack_rgb555(
                clamp5(i32::from(r1) - i32::from(r1) * i32::from(evy) / 16),
                clamp5(i32::from(g1) - i32::from(g1) * i32::from(evy) / 16),
                clamp5(i32::from(b1) - i32::from(b1) * i32::from(evy) / 16),
            )
        }
        _ => top,
    }
}

/// Optional green-swap on adjacent pixel pairs (DISPCNT-adjacent undocumented).
pub fn green_swap_line(line: &mut [u16; 240], enable: bool) {
    if !enable {
        return;
    }
    for i in (0..240).step_by(2) {
        let a = line[i];
        let b = line[i + 1];
        let (ra, ga, ba) = rgb555_channels(a);
        let (rb, gb, bb) = rgb555_channels(b);
        line[i] = pack_rgb555(ra, gb, ba);
        line[i + 1] = pack_rgb555(rb, ga, bb);
    }
}
