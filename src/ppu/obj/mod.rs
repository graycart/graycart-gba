//! OBJ (sprites) — normal (non-affine) sprites.
//!
//! Cited: GBATEK LCD Video Controller, OBJ.
//! <https://problemkaputt.de/gbatek.htm>

#[cfg(test)]
mod tests;

/// A single opaque sprite sample at a screen pixel.
pub struct SpritePixel {
    pub color: u16,
    pub priority: u8,
    /// True when ATTR0 bits 10–11 equal 1 (semi-transparent OBJ).
    pub semi: bool,
}

const OAM_ENTRIES: usize = 128;
const OAM_ENTRY_SIZE: usize = 8;
const SCREEN_W: i32 = 240;
const X_WRAP: i32 = 512;

/// Number of the 128 OAM entries that are not disabled and not OBJ-window.
///
/// Affine sprites still count. Disabled is ATTR0 bit 9 when bit 8 is clear.
/// OBJ window is ATTR0 bits 10–11 == 2.
pub fn sprite_count(oam: &[u8]) -> u32 {
    let mut count = 0u32;
    for i in 0..OAM_ENTRIES {
        let attr0 = read_u16_le(oam, i * OAM_ENTRY_SIZE);
        if obj_disabled(attr0) {
            continue;
        }
        if obj_mode(attr0) == 2 {
            continue;
        }
        count += 1;
    }
    count
}

/// Front-most drawable sprite at screen pixel `(x, y)`.
///
/// Skips disabled, affine, OBJ-window, and prohibited (mode 3) entries.
/// Lower priority wins. Same priority: lower OAM index wins.
/// `one_d` is DISPCNT bit 6 (1D mapping). OBJ tiles live at VRAM offset
/// `0x10000` when `bitmap_mode` is false, and `0x14000` when true.
/// `mosaic` is the MOSAIC I/O halfword; ATTR0 bit 12 enables per-sprite mosaic.
#[allow(clippy::too_many_arguments)]
pub fn sprite_pixel(
    x: usize,
    y: usize,
    oam: &[u8],
    pal: &[u8],
    vram: &[u8],
    one_d: bool,
    bitmap_mode: bool,
    mosaic: u16,
) -> Option<SpritePixel> {
    let obj_base = if bitmap_mode { 0x14000 } else { 0x10000 };
    let mut best: Option<SpritePixel> = None;

    for i in 0..OAM_ENTRIES {
        let off = i * OAM_ENTRY_SIZE;
        let attr0 = read_u16_le(oam, off);
        let attr1 = read_u16_le(oam, off + 2);
        let attr2 = read_u16_le(oam, off + 4);

        if obj_disabled(attr0) || obj_affine(attr0) {
            continue;
        }
        let mode = obj_mode(attr0);
        // OBJ window and prohibited — not drawn.
        if mode == 2 || mode == 3 {
            continue;
        }

        let shape = (attr0 >> 14) & 0b11;
        let size = (attr1 >> 14) & 0b11;
        let (width, height) = sprite_size(shape, size);

        let oam_y = (attr0 & 0xFF) as usize;
        // Hit-test at the real screen pixel.
        let hit_y = (y.wrapping_sub(oam_y)) & 0xFF;
        if hit_y >= height {
            continue;
        }

        let oam_x = attr1 & 0x1FF;
        let Some(_hit_x) = sprite_local_x(x, oam_x, width) else {
            continue;
        };

        // Texel sample: mosaic uses the block origin as the screen coordinate.
        let (sample_x, sample_y) = if attr0 & (1 << 12) != 0 {
            super::mosaic::obj_origin(x, y, mosaic)
        } else {
            (x, y)
        };
        let local_y = (sample_y.wrapping_sub(oam_y)) & 0xFF;
        if local_y >= height {
            continue;
        }
        let Some(local_x) = sprite_local_x(sample_x, oam_x, width) else {
            continue;
        };

        let hflip = attr1 & (1 << 12) != 0;
        let vflip = attr1 & (1 << 13) != 0;
        let mut tx = local_x;
        let mut ty = local_y;
        if hflip {
            tx = width - 1 - tx;
        }
        if vflip {
            ty = height - 1 - ty;
        }

        let bpp8 = attr0 & (1 << 13) != 0;
        let tile_base = (attr2 & 0x3FF) as usize;
        let priority = ((attr2 >> 10) & 0b11) as u8;
        let pal_bank = ((attr2 >> 12) & 0xF) as usize;
        let semi = mode == 1;

        let Some(color) = sample_sprite_color(
            tx, ty, width, bpp8, tile_base, pal_bank, one_d, obj_base, pal, vram,
        ) else {
            continue;
        };

        let candidate = SpritePixel {
            color,
            priority,
            semi,
        };
        match &best {
            None => best = Some(candidate),
            Some(cur) if candidate.priority < cur.priority => best = Some(candidate),
            // Same or worse priority: keep earlier (lower) OAM index.
            _ => {}
        }
    }

    best
}

/// True when `(x, y)` is an opaque pixel of a non-affine OBJ-window sprite
/// (ATTR0 bits 10–11 == 2). Disabled and affine entries are ignored.
pub fn obj_window_hit(
    x: usize,
    y: usize,
    oam: &[u8],
    vram: &[u8],
    one_d: bool,
    bitmap_mode: bool,
) -> bool {
    let obj_base = if bitmap_mode { 0x14000 } else { 0x10000 };

    for i in 0..OAM_ENTRIES {
        let off = i * OAM_ENTRY_SIZE;
        let attr0 = read_u16_le(oam, off);
        let attr1 = read_u16_le(oam, off + 2);
        let attr2 = read_u16_le(oam, off + 4);

        if obj_affine(attr0) || obj_disabled(attr0) {
            continue;
        }
        if obj_mode(attr0) != 2 {
            continue;
        }

        let shape = (attr0 >> 14) & 0b11;
        let size = (attr1 >> 14) & 0b11;
        let (width, height) = sprite_size(shape, size);

        let oam_y = (attr0 & 0xFF) as usize;
        let local_y = (y.wrapping_sub(oam_y)) & 0xFF;
        if local_y >= height {
            continue;
        }

        let oam_x = attr1 & 0x1FF;
        let Some(local_x) = sprite_local_x(x, oam_x, width) else {
            continue;
        };

        let hflip = attr1 & (1 << 12) != 0;
        let vflip = attr1 & (1 << 13) != 0;
        let mut tx = local_x;
        let mut ty = local_y;
        if hflip {
            tx = width - 1 - tx;
        }
        if vflip {
            ty = height - 1 - ty;
        }

        let bpp8 = attr0 & (1 << 13) != 0;
        let tile_base = (attr2 & 0x3FF) as usize;
        let pal_bank = ((attr2 >> 12) & 0xF) as usize;

        // Palette values are unused; opaque means a non-zero tile index.
        let dummy_pal = [0u8; 0x400];
        if sample_sprite_color(
            tx, ty, width, bpp8, tile_base, pal_bank, one_d, obj_base, &dummy_pal, vram,
        )
        .is_some()
        {
            return true;
        }
    }

    false
}

fn obj_affine(attr0: u16) -> bool {
    attr0 & (1 << 8) != 0
}

/// ATTR0 bit 9 disables the OBJ only when bit 8 (affine) is clear.
fn obj_disabled(attr0: u16) -> bool {
    !obj_affine(attr0) && attr0 & (1 << 9) != 0
}

/// ATTR0 bits 10–11: 0 normal, 1 semi-transparent, 2 OBJ window, 3 prohibited.
fn obj_mode(attr0: u16) -> u16 {
    (attr0 >> 10) & 0b11
}

fn sprite_size(shape: u16, size: u16) -> (usize, usize) {
    match (shape, size) {
        // Square
        (0, 0) => (8, 8),
        (0, 1) => (16, 16),
        (0, 2) => (32, 32),
        (0, 3) => (64, 64),
        // Horizontal
        (1, 0) => (16, 8),
        (1, 1) => (32, 8),
        (1, 2) => (32, 16),
        (1, 3) => (64, 32),
        // Vertical
        (2, 0) => (8, 16),
        (2, 1) => (8, 32),
        (2, 2) => (16, 32),
        (2, 3) => (32, 64),
        // Prohibited shape: treat as 8×8.
        _ => (8, 8),
    }
}

/// Local X within the sprite, handling 9-bit wrap past the left edge.
fn sprite_local_x(screen_x: usize, oam_x: u16, width: usize) -> Option<usize> {
    let sx = screen_x as i32;
    let x = oam_x as i32;
    let mut delta = sx - x;
    if delta < 0 {
        // Sprite placed past the right edge can wrap onto the left.
        if x > SCREEN_W && x >= X_WRAP - width as i32 {
            delta = sx + (X_WRAP - x);
        } else {
            return None;
        }
    }
    if delta >= 0 && (delta as usize) < width {
        Some(delta as usize)
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn sample_sprite_color(
    px: usize,
    py: usize,
    width: usize,
    bpp8: bool,
    tile_base: usize,
    pal_bank: usize,
    one_d: bool,
    obj_base: usize,
    pal: &[u8],
    vram: &[u8],
) -> Option<u16> {
    let tile_x = px / 8;
    let tile_y = py / 8;
    let fine_x = px % 8;
    let fine_y = py % 8;
    let tiles_w = width / 8;

    let tile_num = if one_d {
        if bpp8 {
            tile_base + (tile_y * tiles_w + tile_x) * 2
        } else {
            tile_base + tile_y * tiles_w + tile_x
        }
    } else if bpp8 {
        tile_base + tile_x * 2 + tile_y * 32
    } else {
        tile_base + tile_x + tile_y * 32
    };

    // Tile numbers address VRAM in 32-byte units (8bpp tiles span two numbers).
    let tile_off = obj_base + tile_num * 32;

    let index = if bpp8 {
        let byte_off = tile_off + fine_y * 8 + fine_x;
        vram.get(byte_off).copied().unwrap_or(0)
    } else {
        let byte_off = tile_off + fine_y * 4 + fine_x / 2;
        let byte = vram.get(byte_off).copied().unwrap_or(0);
        if fine_x & 1 == 0 {
            byte & 0x0F
        } else {
            byte >> 4
        }
    };

    if index == 0 {
        return None;
    }

    let pal_index = if bpp8 {
        index as usize
    } else {
        pal_bank * 16 + index as usize
    };
    // OBJ palette starts at byte 0x200 of palette RAM.
    Some(read_u16_le(pal, 0x200 + pal_index * 2) & 0x7FFF)
}

fn read_u16_le(bytes: &[u8], offset: usize) -> u16 {
    let lo = u16::from(bytes.get(offset).copied().unwrap_or(0));
    let hi = u16::from(bytes.get(offset + 1).copied().unwrap_or(0));
    lo | (hi << 8)
}
