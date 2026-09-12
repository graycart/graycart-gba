//! Open-bus and BIOS-protect **placeholders** (not cycle-exact).
//!
//! Cited: GBATEK -- GBA Unpredictable Things (BIOS protect / unused memory)
//!   https://problemkaputt.de/gbatek-gba-unpredictable-things.htm
//! Cross-check: research `docs/graycart-gba/02-memory-bus-dma.md` §4; TBD B-07.
//!
//! **TBD — do not invent:** ARM/Thumb pipeline open-bus formulas, IWRAM
//! OldLO/OldHI model variants, mid-instruction DMA MDR latches, or any
//! claim of cycle-exact unused-memory reads. Callers must treat
//! [`OpenBusKind::UnusedMemory`] / [`OpenBusKind::UnusedIo`] results as
//! stubs until hardware-consensus wiring lands.

/// Categories of open-bus / protect behavior the bus may eventually apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpenBusKind {
    /// BIOS read while PC is outside BIOS — last fetched BIOS opcode.
    BiosProtect,
    /// Unused address (e.g. `00004000–01FFFFFF`, `10000000+`) — CPU pipeline residue.
    ///
    /// TBD: ARM ≈ `[PC+8]`; Thumb depends on region/alignment (B-07).
    UnusedMemory,
    /// Unused / write-only I/O fragment — full-word open bus or unread half = 0.
    ///
    /// TBD until I/O decode marks readable halves.
    UnusedIo,
    /// Empty Game Pak ROM — shared addr/data lines (`(addr/2) & 0xFFFF`).
    EmptyCartRom,
}

/// Latch state for BIOS-protect / future open-bus consumers.
///
/// Pipeline residue for unused-memory open bus is **not** modeled here yet
/// (TBD). Only the BIOS last-opcode latch is stored as a wiring hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OpenBusState {
    /// Last successfully fetched BIOS opcode (32-bit pipeline residue).
    ///
    /// Updated by BIOS fetch paths when those exist; default `0` at power-on
    /// until the BIOS stream wires real fetches.
    pub last_bios_opcode: u32,
}

impl Default for OpenBusState {
    #[inline]
    fn default() -> Self {
        Self {
            last_bios_opcode: 0,
        }
    }
}

impl OpenBusState {
    #[inline]
    pub const fn new() -> Self {
        Self {
            last_bios_opcode: 0,
        }
    }

    /// Record a successfully fetched BIOS opcode (caller-gated to BIOS PC).
    #[inline]
    pub fn note_bios_fetch(&mut self, opcode: u32) {
        self.last_bios_opcode = opcode;
    }
}

/// BIOS region end (exclusive): `00000000–00003FFF`.
pub const BIOS_END: u32 = 0x4000;

/// True when `pc` is inside the BIOS address window (pre-mirror).
#[inline]
pub const fn pc_in_bios(pc: u32) -> bool {
    pc < BIOS_END
}

/// Placeholder BIOS-protect read.
///
/// When `pc` is **outside** BIOS, hardware returns the last successfully
/// fetched BIOS opcode. When `pc` is inside BIOS, the caller should perform
/// a normal BIOS ROM read instead of calling this helper.
///
/// TBD: exact SoftReset / IRQ / SWI residue examples once BIOS fetch is wired.
#[inline]
pub fn bios_protect_read(pc: u32, state: &OpenBusState) -> Option<u32> {
    if pc_in_bios(pc) {
        None
    } else {
        Some(state.last_bios_opcode)
    }
}

/// Placeholder unused-memory open bus.
///
/// **TBD:** returns `None` — do not invent ARM `[PC+8]` / Thumb OldLO/OldHI
/// values here. Wire after pipeline + region consensus (doc §4, B-07).
#[inline]
pub fn unused_memory_open_bus(
    _kind: OpenBusKind,
    _pc: u32,
    _addr: u32,
    _thumb: bool,
) -> Option<u32> {
    // TBD: cycle-exact open bus not implemented.
    None
}

/// Empty Game Pak ROM halfword pattern (documented; not TBD).
///
/// Shared address/data lines fill the region with `(addr / 2) & 0xFFFF`.
#[inline]
pub const fn empty_cart_rom_halfword(addr: u32) -> u16 {
    ((addr / 2) & 0xFFFF) as u16
}

/// Convenience: assemble a little-endian word from two empty-cart halfwords.
#[inline]
pub const fn empty_cart_rom_word(addr: u32) -> u32 {
    let lo = empty_cart_rom_halfword(addr) as u32;
    let hi = empty_cart_rom_halfword(addr.wrapping_add(2)) as u32;
    lo | (hi << 16)
}
