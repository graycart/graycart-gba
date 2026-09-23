//! Regular (non-affine) text backgrounds.
//!
//! Cited: GBATEK LCD Video Controller, backgrounds.
//! <https://problemkaputt.de/gbatek.htm>

#[cfg(test)]
mod tests;

/// Color of one text-background pixel, or None when the pixel is transparent (index 0).
/// `priority` is BGCNT bits 0-1.
pub struct BgPixel {
    pub color: u16,
    pub priority: u8,
}

/// Sample one screen pixel from a text (regular) background.
///
/// `x`/`y` are screen coordinates. `hofs`/`vofs` are BGHOFS/BGVOFS (low 9 bits).
/// `pal` is palette RAM; `vram` is VRAM.
pub fn text_bg_pixel(
    x: usize,
    y: usize,
    bgcnt: u16,
    hofs: u16,
    vofs: u16,
    pal: &[u8],
    vram: &[u8],
) -> Option<BgPixel> {
    let priority = (bgcnt & 0b11) as u8;
    let char_base = ((bgcnt >> 2) & 0b11) as usize * 0x4000;
    let is_8bpp = bgcnt & (1 << 7) != 0;
    let screen_base = ((bgcnt >> 8) & 0b1_1111) as usize * 0x800;
    let size = (bgcnt >> 14) & 0b11;

    let (x_mask, y_mask) = match size {
        0 => (0xFFu16, 0xFFu16),   // 256×256
        1 => (0x1FFu16, 0xFFu16),  // 512×256
        2 => (0xFFu16, 0x1FFu16),  // 256×512
        _ => (0x1FFu16, 0x1FFu16), // 512×512
    };

    let px = (hofs & 0x1FF).wrapping_add(x as u16) & x_mask;
    let py = (vofs & 0x1FF).wrapping_add(y as u16) & y_mask;

    let mut tile_x = (px / 8) as usize;
    let mut tile_y = (py / 8) as usize;
    let mut fine_x = (px % 8) as usize;
    let mut fine_y = (py % 8) as usize;

    // Select which 32×32 screen block for wide/tall maps.
    let block = match size {
        1 => {
            // 512×256: second horizontal block is next 0x800.
            if tile_x >= 32 {
                tile_x -= 32;
                1
            } else {
                0
            }
        }
        2 => {
            // 256×512: second vertical block follows.
            if tile_y >= 32 {
                tile_y -= 32;
                1
            } else {
                0
            }
        }
        3 => {
            // 512×512: TL=0, TR=1, BL=2, BR=3.
            let mut b = 0usize;
            if tile_x >= 32 {
                tile_x -= 32;
                b += 1;
            }
            if tile_y >= 32 {
                tile_y -= 32;
                b += 2;
            }
            b
        }
        _ => 0,
    };

    let map_off = screen_base + block * 0x800 + (tile_y * 32 + tile_x) * 2;
    let entry = read_u16_le(vram, map_off);
    let tile_num = (entry & 0x3FF) as usize;
    let hflip = entry & (1 << 10) != 0;
    let vflip = entry & (1 << 11) != 0;
    let pal_bank = ((entry >> 12) & 0xF) as usize;

    if hflip {
        fine_x = 7 - fine_x;
    }
    if vflip {
        fine_y = 7 - fine_y;
    }

    let color = if is_8bpp {
        let tile_off = char_base + tile_num * 64;
        let row_off = tile_off + fine_y * 8;
        let index = vram.get(row_off + fine_x).copied().unwrap_or(0) as usize;
        if index == 0 {
            return None;
        }
        read_u16_le(pal, index * 2) & 0x7FFF
    } else {
        let tile_off = char_base + tile_num * 32;
        let row_off = tile_off + fine_y * 4;
        let byte = vram.get(row_off + fine_x / 2).copied().unwrap_or(0);
        let index = if fine_x.is_multiple_of(2) {
            (byte & 0x0F) as usize
        } else {
            (byte >> 4) as usize
        };
        if index == 0 {
            return None;
        }
        read_u16_le(pal, (pal_bank * 16 + index) * 2) & 0x7FFF
    };

    Some(BgPixel { color, priority })
}

fn read_u16_le(bytes: &[u8], offset: usize) -> u16 {
    let lo = u16::from(bytes.get(offset).copied().unwrap_or(0));
    let hi = u16::from(bytes.get(offset + 1).copied().unwrap_or(0));
    lo | (hi << 8)
}
