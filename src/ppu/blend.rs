//! Color special effects (alpha / brightness) — P4 functional.
//!
//! Cited: GBATEK — Color Special Effects
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md` §7

use super::regs::LcdRegs;

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

/// Apply BLDCNT effect when `blend_ok` (window allows). Top is 1st target pixel.
#[must_use]
pub fn apply_blend(regs: &LcdRegs, top: u16, bottom: Option<u16>, blend_ok: bool) -> u16 {
    if !blend_ok {
        return top;
    }
    let mode = (regs.bldcnt >> 6) & 3;
    let (r1, g1, b1) = rgb555_channels(top);
    match mode {
        1 => {
            // Alpha: need 2nd target.
            let Some(bot) = bottom else {
                return top;
            };
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
