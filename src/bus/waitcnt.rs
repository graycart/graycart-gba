//! WAITCNT (`4000204h`) — Game Pak / SRAM waitstate control.
//!
//! Cited: GBATEK -- GBA System Control (WAITCNT)
//!   https://problemkaputt.de/gbatek-gba-system-control.htm
//! Cross-check: research `docs/graycart-gba/02-memory-bus-dma.md` §5.3.
//! Note: prefetch enable is bit storage only (no fill/drain FSM — P8).

use super::wait::{RomWindow, WaitTables};

/// Writable WAITCNT bits: 0–12 and 14. Bit 13 unused; bit 15 is cart-type RO.
const WRITABLE_MASK: u16 = 0x5FFF;

/// Power-on value (`0000h`).
pub const POWER_ON: u16 = 0x0000;

/// Common commercial cart setting: WS0 3,1; SRAM 8; WS2 8,8; prefetch on.
pub const COMMERCIAL_COMMON: u16 = 0x4317;

/// N-wait encoding `0..3` → waitstate counts `4, 3, 2, 8`.
const N_WAITS: [u8; 4] = [4, 3, 2, 8];

/// Game Pak waitstate control register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitCnt {
    raw: u16,
}

impl Default for WaitCnt {
    fn default() -> Self {
        Self::power_on()
    }
}

impl WaitCnt {
    /// Power-on reset (`0000h`).
    #[inline]
    pub const fn power_on() -> Self {
        Self { raw: POWER_ON }
    }

    /// Construct from a raw halfword (writable bits + RO cart-type bit).
    #[inline]
    pub const fn from_u16(value: u16) -> Self {
        Self {
            raw: value & (WRITABLE_MASK | 0x8000),
        }
    }

    /// Raw register value (including RO bit 15).
    #[inline]
    pub const fn raw(self) -> u16 {
        self.raw
    }

    /// CPU/MMIO write: updates writable fields; preserves cart-type RO (bit 15).
    #[inline]
    pub fn write(&mut self, value: u16) {
        self.raw = (value & WRITABLE_MASK) | (self.raw & 0x8000);
    }

    /// Set cart-type flag (hardware RO from cart pin; emulator/cart attach hook).
    #[inline]
    pub fn set_cart_type_cgb(&mut self, cgb: bool) {
        if cgb {
            self.raw |= 0x8000;
        } else {
            self.raw &= !0x8000;
        }
    }

    /// Bit 15: Game Pak type (`0` = GBA, `1` = CGB). Read-only from software.
    #[inline]
    pub const fn cart_type_cgb(self) -> bool {
        (self.raw & 0x8000) != 0
    }

    /// Bit 14: Game Pak prefetch enable — **stub** (no FSM until P8).
    #[inline]
    pub const fn prefetch_enable(self) -> bool {
        (self.raw & (1 << 14)) != 0
    }

    /// Stub setter for prefetch enable (bit 14 only; no buffer side effects).
    #[inline]
    pub fn set_prefetch_enable(&mut self, enable: bool) {
        if enable {
            self.raw |= 1 << 14;
        } else {
            self.raw &= !(1 << 14);
        }
    }

    /// Bits 11–12: PHI terminal output encoding (normally leave disabled).
    #[inline]
    pub const fn phi_out(self) -> u8 {
        ((self.raw >> 11) & 0b11) as u8
    }

    /// SRAM N waitstates from bits 0–1 (`4,3,2,8`).
    #[inline]
    pub const fn sram_waits(self) -> u8 {
        N_WAITS[(self.raw & 0b11) as usize]
    }

    /// WS0 first-access (N) waitstates from bits 2–3.
    #[inline]
    pub const fn ws0_n_waits(self) -> u8 {
        N_WAITS[((self.raw >> 2) & 0b11) as usize]
    }

    /// WS0 second-access (S) waitstates from bit 4 (`2` or `1`).
    #[inline]
    pub const fn ws0_s_waits(self) -> u8 {
        if (self.raw & (1 << 4)) != 0 {
            1
        } else {
            2
        }
    }

    /// WS1 first-access (N) waitstates from bits 5–6.
    #[inline]
    pub const fn ws1_n_waits(self) -> u8 {
        N_WAITS[((self.raw >> 5) & 0b11) as usize]
    }

    /// WS1 second-access (S) waitstates from bit 7 (`4` or `1`).
    #[inline]
    pub const fn ws1_s_waits(self) -> u8 {
        if (self.raw & (1 << 7)) != 0 {
            1
        } else {
            4
        }
    }

    /// WS2 first-access (N) waitstates from bits 8–9.
    #[inline]
    pub const fn ws2_n_waits(self) -> u8 {
        N_WAITS[((self.raw >> 8) & 0b11) as usize]
    }

    /// WS2 second-access (S) waitstates from bit 10 (`8` or `1`).
    #[inline]
    pub const fn ws2_s_waits(self) -> u8 {
        if (self.raw & (1 << 10)) != 0 {
            1
        } else {
            8
        }
    }

    /// N waitstates for a ROM waitstate window.
    #[inline]
    pub const fn rom_n_waits(self, window: RomWindow) -> u8 {
        match window {
            RomWindow::Ws0 => self.ws0_n_waits(),
            RomWindow::Ws1 => self.ws1_n_waits(),
            RomWindow::Ws2 => self.ws2_n_waits(),
        }
    }

    /// S waitstates for a ROM waitstate window.
    #[inline]
    pub const fn rom_s_waits(self, window: RomWindow) -> u8 {
        match window {
            RomWindow::Ws0 => self.ws0_s_waits(),
            RomWindow::Ws1 => self.ws1_s_waits(),
            RomWindow::Ws2 => self.ws2_s_waits(),
        }
    }

    /// Build cycle cost tables from this register (ROM/SRAM from WAITCNT).
    #[inline]
    pub fn to_tables(self) -> WaitTables {
        WaitTables::from_waitcnt(self)
    }
}
