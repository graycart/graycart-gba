//! Color special effects (BLDCNT / BLDALPHA / BLDY).
//!
//! Cited: GBATEK LCD Video Controller, color special effects.
//! <https://problemkaputt.de/gbatek.htm>
//!
//! Layer indices: 0=BG0, 1=BG1, 2=BG2, 3=BG3, 4=OBJ, 5=backdrop.
//! Effect modes (BLDCNT bits 6–7): 0=off, 1=alpha, 2=brighten, 3=darken.
//! Pixels are BGR555 in the low 15 bits; each 5-bit channel blends independently.

#[cfg(test)]
mod tests;

/// True when `layer` is selected as a 1st target (BLDCNT bits 0–5).
pub fn first_target(bldcnt: u16, layer: u8) -> bool {
    layer < 6 && (bldcnt & (1 << layer)) != 0
}

/// True when `layer` is selected as a 2nd target (BLDCNT bits 8–13).
pub fn second_target(bldcnt: u16, layer: u8) -> bool {
    layer < 6 && (bldcnt & (1 << (8 + layer))) != 0
}

/// Apply color special effects to `top`.
///
/// `force_alpha` is the semi-transparent OBJ case.
/// `bottom` is `Some` only when a 2nd-target pixel exists behind the top pixel.
pub fn blend(
    top: u16,
    bottom: Option<u16>,
    bldcnt: u16,
    bldalpha: u16,
    bldy: u16,
    force_alpha: bool,
) -> u16 {
    let effect = (bldcnt >> 6) & 0b11;

    if force_alpha || effect == 1 {
        return match bottom {
            Some(bot) => alpha(top, bot, bldalpha),
            None => top,
        };
    }

    match effect {
        0 => top,
        2 => brighten(top, bldy),
        3 => darken(top, bldy),
        _ => top,
    }
}

fn eva(bldalpha: u16) -> u16 {
    (bldalpha & 0x1F).min(16)
}

fn evb(bldalpha: u16) -> u16 {
    ((bldalpha >> 8) & 0x1F).min(16)
}

fn evy(bldy: u16) -> u16 {
    (bldy & 0x1F).min(16)
}

fn ch(color: u16, shift: u16) -> u16 {
    (color >> shift) & 0x1F
}

fn pack(r: u16, g: u16, b: u16) -> u16 {
    r | (g << 5) | (b << 10)
}

fn clamp5(v: u16) -> u16 {
    v.min(31)
}

/// Alpha: `(top * EVA + bottom * EVB) / 16` per channel.
fn alpha(top: u16, bottom: u16, bldalpha: u16) -> u16 {
    let a = eva(bldalpha);
    let b = evb(bldalpha);
    let blend_ch = |shift| clamp5((ch(top, shift) * a + ch(bottom, shift) * b) / 16);
    pack(blend_ch(0), blend_ch(5), blend_ch(10))
}

/// Brighten: `top + (31 - top) * EVY / 16` per channel.
fn brighten(top: u16, bldy: u16) -> u16 {
    let y = evy(bldy);
    let bright_ch = |shift| {
        let t = ch(top, shift);
        clamp5(t + (31 - t) * y / 16)
    };
    pack(bright_ch(0), bright_ch(5), bright_ch(10))
}

/// Darken: `top - top * EVY / 16` per channel.
fn darken(top: u16, bldy: u16) -> u16 {
    let y = evy(bldy);
    let dark_ch = |shift| {
        let t = ch(top, shift);
        t - t * y / 16
    };
    pack(dark_ch(0), dark_ch(5), dark_ch(10))
}
