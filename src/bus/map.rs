//! Address and slice helpers shared by the memory map.

use super::VRAM;
use crate::timing::Width;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Access {
    Fetch,
    Data,
}

pub(super) fn width_bytes(width: Width) -> u32 {
    match width {
        Width::Byte => 1,
        Width::Half => 2,
        Width::Word => 4,
    }
}

/// Align halfword/word data accesses, except the 8-bit SRAM/flash bus where the
/// unaligned address selects which lane is written (jsmolka save tests).
pub(super) fn align_data(addr: u32, size: u32) -> u32 {
    if matches!(addr >> 24, 0x0E | 0x0F) {
        return addr;
    }
    match size {
        2 => addr & !1,
        4 => addr & !3,
        _ => addr,
    }
}

pub(super) fn covers_byte(off: u32, size: u32, byte: u32) -> bool {
    off <= byte && off.saturating_add(size) > byte
}

pub(super) fn sound_covers(off: u32, size: u32) -> bool {
    for i in 0..size {
        let addr = off.wrapping_add(i);
        if (0x60..=0x7E).contains(&addr)
            || addr == 0x80
            || addr == 0x81
            || addr == 0x82
            || addr == 0x83
            || addr == 0x84
            || addr == 0x85
            || addr == 0x88
            || addr == 0x89
            || (0x90..=0x9F).contains(&addr)
            || (0xA0..=0xA7).contains(&addr)
        {
            return true;
        }
    }
    false
}

pub(super) fn sio_touches(off: u32, size: u32) -> bool {
    let start = off;
    let end = off.saturating_add(size.saturating_sub(1));
    // SIODATA32 / SIOMULTI0–3 / SIOCNT / SIOMLT_SEND / SIODATA8: 0x120–0x12B
    // RCNT: 0x134–0x135
    ranges_overlap(start, end, 0x120, 0x12B) || ranges_overlap(start, end, 0x134, 0x135)
}

fn ranges_overlap(a0: u32, a1: u32, b0: u32, b1: u32) -> bool {
    a0 <= b1 && b0 <= a1
}

pub(super) fn vram_off(addr: u32) -> usize {
    let off = (addr & 0x1_FFFF) as usize;
    if off < VRAM { off } else { off - 0x8000 }
}

/// Past-end Game Pak reads: each halfword is `((addr >> 1) & 0xFFFF)`.
pub(super) fn rom_past_end(addr: u32, size: u32) -> u32 {
    let half = |a: u32| (a >> 1) & 0xFFFF;
    match size {
        1 => {
            let hw = half(addr & !1);
            (hw >> (8 * (addr & 1))) & 0xFF
        }
        2 => half(addr),
        4 => {
            let base = addr & !3;
            let low = half(base);
            let high = half(base.wrapping_add(2));
            low | (high << 16)
        }
        _ => 0,
    }
}

pub(super) fn slice_load(mem: &[u8], off: usize, size: u32) -> u32 {
    let mut out = 0u32;
    for i in 0..size as usize {
        let byte = mem.get(off.wrapping_add(i)).copied().unwrap_or(0);
        out |= (byte as u32) << (8 * i);
    }
    out
}

pub(super) fn slice_store(mem: &mut [u8], off: usize, value: u32, size: u32) {
    for i in 0..size as usize {
        if let Some(slot) = mem.get_mut(off.wrapping_add(i)) {
            *slot = (value >> (8 * i)) as u8;
        }
    }
}
