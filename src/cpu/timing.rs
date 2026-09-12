//! Instruction cycle pricing helpers (P8 waitstate / pipeline advance).
//!
//! Cited: GBATEK — ARM CPU Cycle Times / Gamepak Waitstates
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM7TDMI TRM (DDI0210C) — N/S/I cycle kinds
//! Cross-check: research `docs/graycart-gba/01-cpu-arm7tdmi.md` §4.
//! Note: coarse per-insn quantum (not mid-insn preempt). Prefetch hits and
//! Disable Bug feed the fetch kind. Pricing peeks the buffer; pipeline fetch
//! owns drain via [`PrefetchBuffer::try_fetch_half`].

use crate::bus::disable_bug::{
    arm_likely_i_cycles_no_pc, force_next_fetch_nonseq, thumb_likely_i_cycles_no_pc,
    DisableBugInput,
};
use crate::bus::wait::{AccessKind, AccessSize, RomWindow, WaitTables};
use crate::bus::PrefetchBuffer;
use crate::cpu::IsaState;

/// Inputs for pricing one retired instruction quantum.
#[derive(Debug, Clone, Copy)]
pub struct TimingInput {
    pub isa: IsaState,
    /// Address of the opcode that just executed (Decode PC).
    pub code_addr: u32,
    pub opcode_raw: u32,
    /// True when the insn branched / wrote PC / took exception.
    pub pc_changed: bool,
    pub prefetch_enabled: bool,
    /// Force next fetch N (latched Disable Bug / post-DMA).
    pub force_fetch_n: bool,
}

/// Result of pricing one instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InsnCycles {
    /// Total system cycles to advance subsystems.
    pub total: u32,
    /// Internal cycles portion (I) — eligible for prefetch fill.
    pub internal: u32,
    /// Non-ROM data/code cycles that still free the cart bus for prefetch.
    pub cart_idle: u32,
    /// Whether Disable Bug should force the *next* opcode fetch as N.
    pub latch_disable_bug: bool,
    /// Code fetch access kind used for this insn's fetch pricing.
    pub fetch_kind: AccessKind,
}

/// Price a coarse instruction quantum: code fetch (+ optional I) using wait tables.
pub fn price_insn(input: TimingInput, tables: WaitTables, prefetch: &PrefetchBuffer) -> InsnCycles {
    let code_in_rom = RomWindow::from_addr(input.code_addr).is_some();
    let had_i = match input.isa {
        IsaState::Arm => arm_likely_i_cycles_no_pc(input.opcode_raw),
        IsaState::Thumb => thumb_likely_i_cycles_no_pc(input.opcode_raw as u16),
    };
    let bug = DisableBugInput {
        prefetch_enabled: input.prefetch_enabled,
        code_in_rom,
        had_internal_cycles: had_i,
        pc_changed: input.pc_changed,
    };
    let latch_disable_bug = force_next_fetch_nonseq(bug);

    let fetch_kind = if input.force_fetch_n {
        AccessKind::Nonseq
    } else {
        AccessKind::Seq
    };

    let (fetch_cycles, cart_idle_from_fetch) =
        if let Some(window) = RomWindow::from_addr(input.code_addr) {
            match input.isa {
                IsaState::Thumb => {
                    price_rom_half(input.code_addr, window, fetch_kind, tables, prefetch)
                }
                IsaState::Arm => {
                    let (c0, idle0) =
                        price_rom_half(input.code_addr, window, fetch_kind, tables, prefetch);
                    let (c1, idle1) = price_rom_half(
                        input.code_addr.wrapping_add(2),
                        window,
                        AccessKind::Seq,
                        tables,
                        prefetch,
                    );
                    (c0 + c1, idle0 + idle1)
                }
            }
        } else {
            let size = match input.isa {
                IsaState::Arm => AccessSize::Word,
                IsaState::Thumb => AccessSize::Half,
            };
            let c = if (input.code_addr >> 24) == 0x02 {
                tables.ewram_cycles(size)
            } else {
                WaitTables::fixed_fast_cycles(size)
            };
            (c, c)
        };

    let internal = if had_i { 1 } else { 0 };
    let total = fetch_cycles.saturating_add(internal).max(1);
    let cart_idle = cart_idle_from_fetch.saturating_add(internal);

    InsnCycles {
        total,
        internal,
        cart_idle,
        latch_disable_bug,
        fetch_kind,
    }
}

fn price_rom_half(
    addr: u32,
    window: RomWindow,
    kind: AccessKind,
    tables: WaitTables,
    prefetch: &PrefetchBuffer,
) -> (u32, u32) {
    if prefetch.enabled() && prefetch.peek_half(addr).is_some() {
        return (PrefetchBuffer::hit_cycles(), 0);
    }
    let cycles = tables.rom_half_cycles_at(addr, window, kind);
    (cycles, 0)
}
