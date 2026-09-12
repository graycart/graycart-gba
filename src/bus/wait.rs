//! Bus waitstate tables and N/S cycle accounting helpers.
//!
//! Cited: GBATEK -- Gamepak Waitstates / ARM Cycle Times
//!   https://problemkaputt.de/gbatek-gba-system-control.htm
//!   https://problemkaputt.de/gbatek.htm (ARM CPU cycle times)
//! Cross-check: research `docs/graycart-gba/02-memory-bus-dma.md` §5.
//! Note: prefetch fill/drain is out of scope (P8); only WAITCNT bit stub elsewhere.

use super::waitcnt::WaitCnt;

/// Non-sequential (N) vs sequential (S) bus access classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessKind {
    /// First / unrelated address — “1st access”.
    Nonseq,
    /// Continues previous stream — “2nd access”.
    Seq,
}

/// Game Pak ROM waitstate window (`08` / `0A` / `0C`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RomWindow {
    /// `08000000–09FFFFFF` (WS0).
    Ws0,
    /// `0A000000–0BFFFFFF` (WS1).
    Ws1,
    /// `0C000000–0DFFFFFF` (WS2).
    Ws2,
}

impl RomWindow {
    /// Decode ROM waitstate window from a CPU address (`addr >> 24`).
    ///
    /// Returns `None` for non-ROM regions.
    #[inline]
    pub const fn from_addr(addr: u32) -> Option<Self> {
        match addr >> 24 {
            0x08 | 0x09 => Some(Self::Ws0),
            0x0A | 0x0B => Some(Self::Ws1),
            0x0C | 0x0D => Some(Self::Ws2),
            _ => None,
        }
    }
}

/// Size of a single bus beat used for sequential classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessSize {
    Byte = 1,
    Half = 2,
    Word = 4,
}

impl AccessSize {
    #[inline]
    pub const fn bytes(self) -> u32 {
        self as u32
    }
}

/// 128 KiB Game Pak ROM block size (forced-N boundary stride).
pub const ROM_FORCE_N_BLOCK: u32 = 0x2_0000;

/// Default EWRAM waitstates from IMC `4000800h` bits 24–27 = `0Dh` (2 waits).
pub const EWRAM_DEFAULT_WAITS: u8 = 2;

/// Actual access time = `1 + waitstates` (GBATEK).
#[inline]
pub const fn cycles_for_waits(waitstates: u8) -> u32 {
    1 + waitstates as u32
}

/// True when `addr` is the first halfword of a 128 KiB Game Pak ROM block.
///
/// Even mid-burst, that beat is forced **N** (e.g. `LDMIA` crossing `8020000h`).
#[inline]
pub const fn rom_force_nonseq(addr: u32) -> bool {
    (addr & (ROM_FORCE_N_BLOCK - 1)) == 0
}

/// Classify the next access as N or S given the previous beat.
///
/// Sequential when the next address continues `prev + size` in the same region
/// nibble band and is not a forced-N ROM boundary. Callers supply region equality.
#[inline]
pub fn classify_access(
    prev_addr: u32,
    prev_size: AccessSize,
    next_addr: u32,
    same_region: bool,
) -> AccessKind {
    if !same_region {
        return AccessKind::Nonseq;
    }
    if RomWindow::from_addr(next_addr).is_some() && rom_force_nonseq(next_addr) {
        return AccessKind::Nonseq;
    }
    if next_addr == prev_addr.wrapping_add(prev_size.bytes()) {
        AccessKind::Seq
    } else {
        AccessKind::Nonseq
    }
}

/// Resolved N/S waitstate counts used by the bus arbiter.
///
/// Fixed internal regions use power-on defaults; ROM/SRAM come from [`WaitCnt`].
/// EWRAM waits default to IMC `0Dh` (2) until IMC is modeled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitTables {
    /// SRAM / Flash backup window N waitstates.
    pub sram_waits: u8,
    pub ws0_n: u8,
    pub ws0_s: u8,
    pub ws1_n: u8,
    pub ws1_s: u8,
    pub ws2_n: u8,
    pub ws2_s: u8,
    /// EWRAM (on-board WRAM) waitstates (IMC; default 2 → 3/3/6 cycles).
    pub ewram_waits: u8,
}

impl Default for WaitTables {
    fn default() -> Self {
        Self::power_on()
    }
}

impl WaitTables {
    /// Power-on tables: WAITCNT `0000h` + default EWRAM waits.
    ///
    /// ROM 8/16/32 default **5/5/8**; SRAM **5** cycles (4 waits).
    #[inline]
    pub const fn power_on() -> Self {
        Self::from_waitcnt(WaitCnt::power_on())
    }

    /// Derive ROM/SRAM waits from WAITCNT; keep default EWRAM waits.
    #[inline]
    pub const fn from_waitcnt(wc: WaitCnt) -> Self {
        Self {
            sram_waits: wc.sram_waits(),
            ws0_n: wc.ws0_n_waits(),
            ws0_s: wc.ws0_s_waits(),
            ws1_n: wc.ws1_n_waits(),
            ws1_s: wc.ws1_s_waits(),
            ws2_n: wc.ws2_n_waits(),
            ws2_s: wc.ws2_s_waits(),
            ewram_waits: EWRAM_DEFAULT_WAITS,
        }
    }

    /// Mutate ROM/SRAM costs from WAITCNT; leave EWRAM untouched.
    #[inline]
    pub fn apply_waitcnt(&mut self, wc: WaitCnt) {
        self.sram_waits = wc.sram_waits();
        self.ws0_n = wc.ws0_n_waits();
        self.ws0_s = wc.ws0_s_waits();
        self.ws1_n = wc.ws1_n_waits();
        self.ws1_s = wc.ws1_s_waits();
        self.ws2_n = wc.ws2_n_waits();
        self.ws2_s = wc.ws2_s_waits();
    }

    /// N waitstates for a ROM window.
    #[inline]
    pub const fn rom_n_waits(self, window: RomWindow) -> u8 {
        match window {
            RomWindow::Ws0 => self.ws0_n,
            RomWindow::Ws1 => self.ws1_n,
            RomWindow::Ws2 => self.ws2_n,
        }
    }

    /// S waitstates for a ROM window.
    #[inline]
    pub const fn rom_s_waits(self, window: RomWindow) -> u8 {
        match window {
            RomWindow::Ws0 => self.ws0_s,
            RomWindow::Ws1 => self.ws1_s,
            RomWindow::Ws2 => self.ws2_s,
        }
    }

    /// Waitstates for one ROM halfword beat of the given kind.
    #[inline]
    pub const fn rom_half_waits(self, window: RomWindow, kind: AccessKind) -> u8 {
        match kind {
            AccessKind::Nonseq => self.rom_n_waits(window),
            AccessKind::Seq => self.rom_s_waits(window),
        }
    }

    /// Cycles for one ROM halfword (16-bit bus beat).
    #[inline]
    pub const fn rom_half_cycles(self, window: RomWindow, kind: AccessKind) -> u32 {
        cycles_for_waits(self.rom_half_waits(window, kind))
    }

    /// Cycles for a 32-bit ROM transfer (two halfword beats; second always S).
    #[inline]
    pub const fn rom_word_cycles(self, window: RomWindow, first: AccessKind) -> u32 {
        self.rom_half_cycles(window, first) + self.rom_half_cycles(window, AccessKind::Seq)
    }

    /// Cycles for an 8-bit SRAM/Flash backup access (N only; 8-bit bus).
    #[inline]
    pub const fn sram_byte_cycles(self) -> u32 {
        cycles_for_waits(self.sram_waits)
    }

    /// EWRAM cycles for 8/16/32-bit (16-bit bus → word = two beats).
    ///
    /// Default waits `2` → **3/3/6**.
    #[inline]
    pub const fn ewram_cycles(self, size: AccessSize) -> u32 {
        let beat = cycles_for_waits(self.ewram_waits);
        match size {
            AccessSize::Byte | AccessSize::Half => beat,
            AccessSize::Word => beat * 2,
        }
    }

    /// Fixed 1-cycle regions (BIOS / IWRAM / I/O / OAM per beat, no waitstates).
    #[inline]
    pub const fn fixed_fast_cycles(_size: AccessSize) -> u32 {
        1
    }

    /// Palette / VRAM default: 1 cycle per 16-bit beat (word = 2). PPU conflict +1 is separate.
    #[inline]
    pub const fn video_ram_cycles(size: AccessSize) -> u32 {
        match size {
            AccessSize::Byte | AccessSize::Half => 1,
            AccessSize::Word => 2,
        }
    }

    /// Price one beat after applying the 128 KiB ROM forced-N rule.
    #[inline]
    pub fn rom_half_cycles_at(self, addr: u32, window: RomWindow, kind: AccessKind) -> u32 {
        let kind = if rom_force_nonseq(addr) {
            AccessKind::Nonseq
        } else {
            kind
        };
        self.rom_half_cycles(window, kind)
    }
}
