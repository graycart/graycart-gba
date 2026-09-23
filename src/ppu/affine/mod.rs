//! Affine (rotation/scaling) backgrounds — BG2/BG3 in modes 1–2.
//!
//! Cited: GBATEK LCD Video Controller, affine backgrounds.
//! <https://problemkaputt.de/gbatek.htm>

#[cfg(test)]
mod tests;

/// One composited background pixel (color + draw priority).
pub struct BgPixel {
    pub color: u16,
    pub priority: u8,
}

/// Affine BG pixel. `pa`,`pb`,`pc`,`pd` are the 8.8 signed matrix registers.
/// `x0`,`y0` are the 32-bit reference-point registers (signed, 8 fractional bits).
/// `x`,`y` are screen pixels. Returns `None` for transparent index 0 or out of the
/// map when wrap is off.
#[allow(clippy::too_many_arguments)]
pub fn affine_bg_pixel(
    x: i32,
    y: i32,
    bgcnt: u16,
    pa: i16,
    pb: i16,
    pc: i16,
    pd: i16,
    x0: i32,
    y0: i32,
    pal: &[u8],
    vram: &[u8],
) -> Option<BgPixel> {
    let priority = (bgcnt & 0b11) as u8;
    let char_base = ((bgcnt >> 2) & 0b11) as usize * 0x4000;
    let screen_base = ((bgcnt >> 8) & 0b1_1111) as usize * 0x800;
    let wrap = bgcnt & (1 << 13) != 0;
    let size = match (bgcnt >> 14) & 0b11 {
        0 => 128,
        1 => 256,
        2 => 512,
        _ => 1024,
    };

    // Texture coords in 8.8 fixed point (signed).
    let tx = x0 + x * i32::from(pa) + y * i32::from(pb);
    let ty = y0 + x * i32::from(pc) + y * i32::from(pd);
    let mut tex_x = tx >> 8;
    let mut tex_y = ty >> 8;

    if wrap {
        let mask = size - 1;
        tex_x &= mask;
        tex_y &= mask;
    } else if tex_x < 0 || tex_y < 0 || tex_x >= size || tex_y >= size {
        return None;
    }

    let map_w = size / 8;
    let tile_x = (tex_x / 8) as usize;
    let tile_y = (tex_y / 8) as usize;
    let pix_x = (tex_x & 7) as usize;
    let pix_y = (tex_y & 7) as usize;

    let map_off = screen_base + tile_y * map_w as usize + tile_x;
    let tile = vram.get(map_off).copied().unwrap_or(0) as usize;

    // Affine BGs are always 8bpp: 64 bytes per tile.
    let tile_off = char_base + tile * 64 + pix_y * 8 + pix_x;
    let index = vram.get(tile_off).copied().unwrap_or(0) as usize;
    if index == 0 {
        return None;
    }

    let color = read_u16_le(pal, index * 2) & 0x7FFF;
    Some(BgPixel { color, priority })
}

fn read_u16_le(bytes: &[u8], offset: usize) -> u16 {
    let lo = u16::from(bytes.get(offset).copied().unwrap_or(0));
    let hi = u16::from(bytes.get(offset + 1).copied().unwrap_or(0));
    lo | (hi << 8)
}
