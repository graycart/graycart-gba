//! GBA memory-region decode and bus widths.
//!
//! Cited: GBATEK — GBA Memory Map
//!   https://problemkaputt.de/gbatek-gba-memory-map.htm
//! Cross-check: gbadoc memory layout (mirror / size notes)
//!   https://gbadev.net/gbadoc/memory.html
//! Note: decode + widths only; waitstates / open-bus / video STRB are sibling modules.

/// Physical BIOS ROM size (16 KiB).
pub const BIOS_SIZE: usize = 16 * 1024;
/// On-board WRAM (EWRAM) size (256 KiB).
pub const EWRAM_SIZE: usize = 256 * 1024;
/// On-chip WRAM (IWRAM) size (32 KiB).
pub const IWRAM_SIZE: usize = 32 * 1024;
/// I/O register file size through undocumented IMC at `0x0400_0800`.
pub const IO_SIZE: usize = 0x804;
/// BG/OBJ palette RAM size (1 KiB).
pub const PALETTE_SIZE: usize = 1024;
/// VRAM physical size (96 KiB).
pub const VRAM_SIZE: usize = 96 * 1024;
/// OAM size (1 KiB).
pub const OAM_SIZE: usize = 1024;
/// Maximum Game Pak ROM window size per waitstate view (32 MiB).
pub const ROM_WINDOW_SIZE: usize = 32 * 1024 * 1024;
/// Game Pak SRAM / Flash backup window size (64 KiB).
pub const SRAM_WINDOW_SIZE: usize = 64 * 1024;

/// Decoded GBA memory region (high-level map).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    /// `00000000–00003FFF` — BIOS System ROM.
    Bios,
    /// `00004000–01FFFFFF` — unused (open bus; sibling).
    UnusedLow,
    /// `02xxxxxx` — EWRAM (on-board WRAM).
    Ewram,
    /// `03xxxxxx` — IWRAM (on-chip WRAM).
    Iwram,
    /// `04xxxxxx` — I/O registers (+ undocumented IMC word).
    Io,
    /// `05xxxxxx` — palette RAM.
    Palette,
    /// `06xxxxxx` — VRAM.
    Vram,
    /// `07xxxxxx` — OAM.
    Oam,
    /// `08–09xxxxxx` — Game Pak ROM waitstate 0.
    GamePakRomWs0,
    /// `0A–0Bxxxxxx` — Game Pak ROM waitstate 1.
    GamePakRomWs1,
    /// `0C–0Dxxxxxx` — Game Pak ROM waitstate 2.
    GamePakRomWs2,
    /// `0E–0Fxxxxxx` — Game Pak SRAM / Flash backup window.
    GamePakSram,
    /// `10000000+` — unused (upper nibble ignored on bus; open bus sibling).
    UnusedHigh,
}

/// Native data-bus width for a region (GBATEK access table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusWidth {
    /// 8-bit bus (Game Pak SRAM / Flash backup).
    Bits8,
    /// 16-bit bus (EWRAM, palette, VRAM, Game Pak ROM).
    Bits16,
    /// 32-bit bus (BIOS, IWRAM, I/O, OAM).
    Bits32,
}

impl Region {
    /// Native bus width for this region.
    ///
    /// Unused regions have no physical bus; callers should treat them as open-bus
    /// (sibling) rather than relying on [`BusWidth`].
    #[must_use]
    pub const fn bus_width(self) -> Option<BusWidth> {
        match self {
            Self::Bios | Self::Iwram | Self::Io | Self::Oam => Some(BusWidth::Bits32),
            Self::Ewram
            | Self::Palette
            | Self::Vram
            | Self::GamePakRomWs0
            | Self::GamePakRomWs1
            | Self::GamePakRomWs2 => Some(BusWidth::Bits16),
            Self::GamePakSram => Some(BusWidth::Bits8),
            Self::UnusedLow | Self::UnusedHigh => None,
        }
    }

    /// `true` when this region is one of the three Game Pak ROM waitstate views.
    #[must_use]
    pub const fn is_game_pak_rom(self) -> bool {
        matches!(
            self,
            Self::GamePakRomWs0 | Self::GamePakRomWs1 | Self::GamePakRomWs2
        )
    }

    /// Physical backing size in bytes, when the region has fixed on-chip/on-board RAM.
    #[must_use]
    pub const fn physical_size(self) -> Option<usize> {
        match self {
            Self::Bios => Some(BIOS_SIZE),
            Self::Ewram => Some(EWRAM_SIZE),
            Self::Iwram => Some(IWRAM_SIZE),
            Self::Io => Some(IO_SIZE),
            Self::Palette => Some(PALETTE_SIZE),
            Self::Vram => Some(VRAM_SIZE),
            Self::Oam => Some(OAM_SIZE),
            Self::GamePakSram => Some(SRAM_WINDOW_SIZE),
            Self::GamePakRomWs0 | Self::GamePakRomWs1 | Self::GamePakRomWs2 => {
                Some(ROM_WINDOW_SIZE)
            }
            Self::UnusedLow | Self::UnusedHigh => None,
        }
    }
}

/// Decode `addr` into a memory [`Region`].
///
/// Uses the top address byte (GBATEK map). BIOS vs unused-low is refined inside
/// `00xxxxxx`; Game Pak WS0–2 share the same ROM image with independent waits.
#[must_use]
pub fn decode(addr: u32) -> Region {
    match addr >> 24 {
        0x00 => {
            if addr < 0x0000_4000 {
                Region::Bios
            } else {
                Region::UnusedLow
            }
        }
        0x01 => Region::UnusedLow,
        0x02 => Region::Ewram,
        0x03 => Region::Iwram,
        0x04 => Region::Io,
        0x05 => Region::Palette,
        0x06 => Region::Vram,
        0x07 => Region::Oam,
        0x08 | 0x09 => Region::GamePakRomWs0,
        0x0A | 0x0B => Region::GamePakRomWs1,
        0x0C | 0x0D => Region::GamePakRomWs2,
        0x0E | 0x0F => Region::GamePakSram,
        _ => Region::UnusedHigh,
    }
}
