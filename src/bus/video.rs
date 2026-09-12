//! Video memory write policy: STRB quirks and legal write sizes.
//!
//! Cited: GBATEK -- GBA Unpredictable Things (8-bit writes to video memory)
//!   https://problemkaputt.de/gbatek-gba-unpredictable-things.htm
//! Cited: GBATEK -- LCD VRAM Overview (BG vs OBJ VRAM by BG mode)
//!   https://problemkaputt.de/gbatek.htm
//! Cross-check: research `docs/graycart-gba/02-memory-bus-dma.md` §§2.2, 4.
//! Note: mirrors / region decode belong to the region stream; callers pass
//! already-decoded targets or physical VRAM offsets in `0..region::VRAM_SIZE`.

use super::region::VRAM_SIZE;
use super::AccessSize;

/// OBJ VRAM base offset for tile BG modes 0–2 (`06010000`).
pub const OBJ_VRAM_BASE_TILE: u32 = 0x1_0000;
/// OBJ VRAM base offset for bitmap BG modes 3–5 (`06014000`).
pub const OBJ_VRAM_BASE_BITMAP: u32 = 0x1_4000;

/// Video memory target for write-size / STRB policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoTarget {
    /// BG/OBJ palette RAM (`05000000`, 1 KiB).
    Palette,
    /// Object attribute memory (`07000000`, 1 KiB).
    Oam,
    /// Background portion of VRAM (mode-dependent).
    BgVram,
    /// Object tile portion of VRAM (mode-dependent).
    ObjVram,
}

/// Outcome of applying video write rules for a CPU store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoWriteAction {
    /// Store at the requested width (16- or 32-bit).
    Store,
    /// STRB to BG VRAM / Palette: write `data × 0x0101` as one halfword.
    ExpandByteToHalfword {
        /// Halfword to store at the halfword-aligned address.
        halfword: u16,
    },
    /// STRB to OBJ VRAM / OAM: write discarded.
    Ignore,
    /// Width not legal for this target (e.g. caller bug).
    Reject,
}

/// Expand an 8-bit STRB datum to the duplicated halfword (`data × 0x0101`).
#[inline]
pub const fn expand_strb_byte(data: u8) -> u16 {
    let b = data as u16;
    b | (b << 8)
}

/// OBJ VRAM start offset within the 96 KiB physical window for DISPCNT BG mode.
///
/// Modes 0–2 → `0x10000`; modes 3–5 → `0x14000`. Modes 6–7 are undefined on
/// hardware; this helper treats them like bitmap (3–5) so callers can still
/// classify — mark accuracy TBD if a title ever programs those bits.
#[inline]
pub const fn obj_vram_base(dispcnt_bg_mode: u8) -> u32 {
    if dispcnt_bg_mode <= 2 {
        OBJ_VRAM_BASE_TILE
    } else {
        OBJ_VRAM_BASE_BITMAP
    }
}

/// Classify a physical VRAM offset (`0..VRAM_SIZE`) as BG vs OBJ for `dispcnt_bg_mode`.
///
/// Returns `None` if `offset` is outside the 96 KiB window.
#[inline]
pub const fn classify_vram_offset(offset: u32, dispcnt_bg_mode: u8) -> Option<VideoTarget> {
    if offset as usize >= VRAM_SIZE {
        return None;
    }
    if offset >= obj_vram_base(dispcnt_bg_mode) {
        Some(VideoTarget::ObjVram)
    } else {
        Some(VideoTarget::BgVram)
    }
}

/// True when a non-byte CPU write width is legal for video memory.
///
/// Palette, OAM, and VRAM accept **16/32 only**; 8-bit stores use [`resolve_strb`]
/// instead of a true byte write.
#[inline]
pub const fn write_size_ok(_target: VideoTarget, size: AccessSize) -> bool {
    // All video targets share the same legal widths (16/32). `_target` is kept
    // so callers can pass the decoded destination without a separate check.
    matches!(size, AccessSize::Half | AccessSize::Word)
}

/// Resolve a non-byte write: Store if size is legal, else Reject.
#[inline]
pub const fn resolve_wide_write(target: VideoTarget, size: AccessSize) -> VideoWriteAction {
    if write_size_ok(target, size) {
        VideoWriteAction::Store
    } else {
        VideoWriteAction::Reject
    }
}

/// Resolve an 8-bit (STRB) write to video memory.
///
/// - OBJ VRAM + OAM → [`VideoWriteAction::Ignore`]
/// - BG VRAM + Palette → [`VideoWriteAction::ExpandByteToHalfword`] with `data × 0x0101`
#[inline]
pub const fn resolve_strb(target: VideoTarget, data: u8) -> VideoWriteAction {
    match target {
        VideoTarget::ObjVram | VideoTarget::Oam => VideoWriteAction::Ignore,
        VideoTarget::BgVram | VideoTarget::Palette => VideoWriteAction::ExpandByteToHalfword {
            halfword: expand_strb_byte(data),
        },
    }
}

/// Resolve any CPU write size against video write policy.
#[inline]
pub const fn resolve_video_write(
    target: VideoTarget,
    size: AccessSize,
    data_lsb: u8,
) -> VideoWriteAction {
    match size {
        AccessSize::Byte => resolve_strb(target, data_lsb),
        AccessSize::Half | AccessSize::Word => resolve_wide_write(target, size),
    }
}
