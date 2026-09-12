//! Prefetch Disable Bug helpers (WAITCNT.14 = 0).
//!
//! Cited: GBATEK — GBA GamePak Prefetch (Disable Bug)
//!   https://problemkaputt.de/gbatek-gba-gamepak-prefetch.htm
//! Cross-check: research `docs/graycart-gba/02-memory-bus-dma.md` §7.2.
//! Note: with prefetch off, ROM opcodes that have I cycles and do not change
//! R15 fetch the next opcode as 1N instead of 1S.

use super::wait::AccessKind;

/// Classification for the Prefetch Disable Bug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisableBugInput {
    /// WAITCNT bit 14 clear.
    pub prefetch_enabled: bool,
    /// Executing opcode address is in Game Pak ROM (`08`/`0A`/`0C` windows).
    pub code_in_rom: bool,
    /// Instruction had one or more internal (I) cycles.
    pub had_internal_cycles: bool,
    /// Instruction wrote R15 / branched / exception (PC changed).
    pub pc_changed: bool,
}

/// True when the next opcode fetch must be forced **N** (Disable Bug).
#[inline]
#[must_use]
pub const fn force_next_fetch_nonseq(input: DisableBugInput) -> bool {
    !input.prefetch_enabled && input.code_in_rom && input.had_internal_cycles && !input.pc_changed
}

/// Resolve the access kind for the next ROM opcode fetch after `input`.
#[inline]
#[must_use]
pub const fn next_opcode_access_kind(input: DisableBugInput, sequential: bool) -> AccessKind {
    if force_next_fetch_nonseq(input) {
        AccessKind::Nonseq
    } else if sequential {
        AccessKind::Seq
    } else {
        AccessKind::Nonseq
    }
}

/// ARM/Thumb opcodes that typically include I cycles without necessarily
/// changing R15 (GBATEK Disable Bug list, coarse classifier for P8 units).
///
/// This is a **heuristic** for cycle pricing / bug latch — not a full scheduler.
#[inline]
#[must_use]
pub const fn arm_likely_i_cycles_no_pc(raw: u32) -> bool {
    // Multiply: bits 27-22 = 000000 and bit7=1 bit4=1 (data-processing mul family)
    let mul = (raw & 0x0FC0_00F0) == 0x0000_0090;
    // LDR (single): 01x1_xxxx with L=1, not PC in Rd necessarily — treat as I-capable
    let ldr = (raw & 0x0C50_0000) == 0x0410_0000;
    // LDM/POP: 100x_xxx1
    let ldm = (raw & 0x0E10_0000) == 0x0810_0000;
    // SWP: 0001_0x00 .... 1001
    let swp = (raw & 0x0FB0_0FF0) == 0x0100_0090;
    // Data-proc with SHIFT(Rs): bit4=1, bit7=0
    let shift_rs = (raw & 0x0E00_0090) == 0x0000_0010;
    mul || ldr || ldm || swp || shift_rs
}

/// Thumb coarse I-cycle hint (loads / push-pop / long mul not in Thumb1).
#[inline]
#[must_use]
pub const fn thumb_likely_i_cycles_no_pc(raw: u16) -> bool {
    let op = raw >> 8;
    // LDR Rd,[Rb,#imm] 01101...
    let ldr_imm = (op & 0xF8) == 0x68;
    // LDR Rd,[PC,#imm] 01001...
    let ldr_pc = (op & 0xF8) == 0x48;
    // LDR Rd,[SP,#imm] 10011...
    let ldr_sp = (op & 0xF8) == 0x98;
    // LDMIA / POP 11001... / 1011_1x10
    let ldm = (op & 0xF8) == 0xC8;
    let pop = (op & 0xFE) == 0xBA;
    ldr_imm || ldr_pc || ldr_sp || ldm || pop
}
