//! Unit tests for instruction cycle pricing (G8-cycles).
//!
//! Cited: GBATEK — ARM CPU Cycle Times / Gamepak Waitstates

use crate::bus::wait::WaitTables;
use crate::bus::waitcnt::WaitCnt;
use crate::bus::PrefetchBuffer;
use crate::cpu::timing::{price_insn, TimingInput};
use crate::cpu::IsaState;

#[test]
fn iwram_thumb_is_one_cycle_fetch() {
    let pf = PrefetchBuffer::new();
    let tables = WaitTables::power_on();
    let c = price_insn(
        TimingInput {
            isa: IsaState::Thumb,
            code_addr: 0x0300_0000,
            opcode_raw: 0,
            pc_changed: false,
            prefetch_enabled: false,
            force_fetch_n: false,
        },
        tables,
        &pf,
    );
    assert_eq!(c.total, 1);
    assert!(c.cart_idle >= 1);
}

#[test]
fn rom_thumb_nonseq_uses_waitcnt() {
    let pf = PrefetchBuffer::new();
    let tables = WaitTables::from_waitcnt(WaitCnt::power_on());
    // Power-on WS0 N = 4 waits → 5 cycles.
    let c = price_insn(
        TimingInput {
            isa: IsaState::Thumb,
            code_addr: 0x0800_0000,
            opcode_raw: 0,
            pc_changed: false,
            prefetch_enabled: false,
            force_fetch_n: true,
        },
        tables,
        &pf,
    );
    assert_eq!(c.total, 5);
    assert!(!c.latch_disable_bug); // no I cycles on NOP raw
}

#[test]
fn prefetch_hit_is_one_cycle() {
    let mut pf = PrefetchBuffer::new();
    pf.set_enabled(true);
    let tables = WaitTables::from_waitcnt(WaitCnt::from_u16(0x4317));
    pf.restart(0x0800_0200, tables);
    pf.tick_idle(64, tables, |_| 0x46C0); // NOP
    let c = price_insn(
        TimingInput {
            isa: IsaState::Thumb,
            code_addr: 0x0800_0200,
            opcode_raw: 0x46C0,
            pc_changed: false,
            prefetch_enabled: true,
            force_fetch_n: false,
        },
        tables,
        &pf,
    );
    assert_eq!(c.total, 1);
}
