//! Windows — rect windows and OBJ window enable masks.
//!
//! Cited: GBATEK LCD Video Controller, windows.
//! <https://problemkaputt.de/gbatek.htm>

#[cfg(test)]
mod tests;

/// Which layers and color effects are allowed at one screen pixel.
pub struct WindowMask {
    pub bg: [bool; 4],
    pub obj: bool,
    pub blend: bool,
}

const WIN0H: usize = 0x40;
const WIN1H: usize = 0x42;
const WIN0V: usize = 0x44;
const WIN1V: usize = 0x46;
const WININ: usize = 0x48;
const WINOUT: usize = 0x4A;

/// Layer / blend enable for screen pixel `(x, y)`.
///
/// `in_obj_window` is true when this pixel is an opaque pixel of a window-mode
/// sprite. It is ignored unless DISPCNT bit 15 is set.
pub fn window_mask(x: usize, y: usize, dispcnt: u16, io: &[u8], in_obj_window: bool) -> WindowMask {
    let win0 = dispcnt & (1 << 13) != 0;
    let win1 = dispcnt & (1 << 14) != 0;
    let objwin = dispcnt & (1 << 15) != 0;

    if !win0 && !win1 && !objwin {
        return WindowMask {
            bg: [true; 4],
            obj: true,
            blend: true,
        };
    }

    let winin = read_u16_le(io, WININ);
    let winout = read_u16_le(io, WINOUT);

    if win0 && inside_rect(x, y, io, WIN0H, WIN0V) {
        return mask_from_bits(winin & 0x3F);
    }
    if win1 && inside_rect(x, y, io, WIN1H, WIN1V) {
        return mask_from_bits((winin >> 8) & 0x3F);
    }
    if objwin && in_obj_window {
        return mask_from_bits((winout >> 8) & 0x3F);
    }
    mask_from_bits(winout & 0x3F)
}

fn mask_from_bits(bits: u16) -> WindowMask {
    WindowMask {
        bg: [
            bits & (1 << 0) != 0,
            bits & (1 << 1) != 0,
            bits & (1 << 2) != 0,
            bits & (1 << 3) != 0,
        ],
        obj: bits & (1 << 4) != 0,
        blend: bits & (1 << 5) != 0,
    }
}

/// GBATEK page rule: X1 > X2 or Y1 > Y2 means that axis is empty (no wrap).
///
/// WIN*H: bits 0–7 = X2 (exclusive), bits 8–15 = X1 (inclusive).
/// WIN*V: bits 0–7 = Y2 (exclusive), bits 8–15 = Y1 (inclusive).
fn inside_rect(x: usize, y: usize, io: &[u8], h_off: usize, v_off: usize) -> bool {
    let h = read_u16_le(io, h_off);
    let v = read_u16_le(io, v_off);
    let x2 = (h & 0xFF) as usize;
    let x1 = ((h >> 8) & 0xFF) as usize;
    let y2 = (v & 0xFF) as usize;
    let y1 = ((v >> 8) & 0xFF) as usize;

    if x1 > x2 || y1 > y2 {
        return false;
    }
    x >= x1 && x < x2 && y >= y1 && y < y2
}

fn read_u16_le(bytes: &[u8], offset: usize) -> u16 {
    let lo = u16::from(bytes.get(offset).copied().unwrap_or(0));
    let hi = u16::from(bytes.get(offset + 1).copied().unwrap_or(0));
    lo | (hi << 8)
}
