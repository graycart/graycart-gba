//! Window enable masks — WIN0 / WIN1 / OBJWIN.
//!
//! Cited: GBATEK — Window Feature
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md` §6
//! Note: functional regions; not cycle-perfect.

use super::obj::obj_window_covers;
use super::regs::LcdRegs;

/// Which window region applies at (x,y). Priority: WIN0 > WIN1 > OBJWIN > outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinRegion {
    Win0,
    Win1,
    ObjWin,
    Outside,
}

/// Layer enable bits inside a window control nibble: BG0–3, OBJ, blend.
#[derive(Debug, Clone, Copy)]
pub struct WinEnables {
    pub bg: [bool; 4],
    pub obj: bool,
    pub blend: bool,
}

impl WinEnables {
    #[must_use]
    pub fn from_nibble(n: u16) -> Self {
        Self {
            bg: [n & 1 != 0, n & 2 != 0, n & 4 != 0, n & 8 != 0],
            obj: n & 0x10 != 0,
            blend: n & 0x20 != 0,
        }
    }

    #[must_use]
    pub fn all_on() -> Self {
        Self {
            bg: [true; 4],
            obj: true,
            blend: true,
        }
    }
}

#[must_use]
pub fn windows_active(regs: &LcdRegs) -> bool {
    // WIN0 / WIN1 / OBJWIN master enables.
    regs.layer_enable(13) || regs.layer_enable(14) || regs.layer_enable(15)
}

#[must_use]
fn in_win_x(x: u16, win_h: u16) -> bool {
    let x1 = win_h >> 8;
    let mut x2 = win_h & 0xFF;
    if x2 > 240 || x1 > x2 {
        x2 = 240;
    }
    x >= x1 && x < x2
}

#[must_use]
fn in_win_y(y: u16, win_v: u16) -> bool {
    let y1 = win_v >> 8;
    let mut y2 = win_v & 0xFF;
    if y2 > 160 || y1 > y2 {
        y2 = 160;
    }
    y >= y1 && y < y2
}

#[must_use]
pub fn region_at(regs: &LcdRegs, x: u16, y: u16, oam: &[u8], vram: &[u8]) -> WinRegion {
    if regs.layer_enable(13) && in_win_x(x, regs.win0_h) && in_win_y(y, regs.win0_v) {
        return WinRegion::Win0;
    }
    if regs.layer_enable(14) && in_win_x(x, regs.win1_h) && in_win_y(y, regs.win1_v) {
        return WinRegion::Win1;
    }
    if regs.layer_enable(15) && obj_window_covers(regs, x, y, oam, vram) {
        return WinRegion::ObjWin;
    }
    WinRegion::Outside
}

#[must_use]
pub fn enables_at(regs: &LcdRegs, x: u16, y: u16, oam: &[u8], vram: &[u8]) -> WinEnables {
    if !windows_active(regs) {
        return WinEnables::all_on();
    }
    match region_at(regs, x, y, oam, vram) {
        WinRegion::Win0 => WinEnables::from_nibble(regs.winin & 0x3F),
        WinRegion::Win1 => WinEnables::from_nibble((regs.winin >> 8) & 0x3F),
        WinRegion::ObjWin => WinEnables::from_nibble((regs.winout >> 8) & 0x3F),
        WinRegion::Outside => WinEnables::from_nibble(regs.winout & 0x3F),
    }
}
