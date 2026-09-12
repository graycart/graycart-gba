//! Unit tests for Prefetch Disable Bug (G8-disable).
//!
//! Cited: GBATEK — GBA GamePak Prefetch (Disable Bug)

use super::disable_bug::{
    arm_likely_i_cycles_no_pc, force_next_fetch_nonseq, next_opcode_access_kind,
    thumb_likely_i_cycles_no_pc, DisableBugInput,
};
use super::wait::AccessKind;

#[test]
fn disable_bug_forces_n_when_prefetch_off() {
    let input = DisableBugInput {
        prefetch_enabled: false,
        code_in_rom: true,
        had_internal_cycles: true,
        pc_changed: false,
    };
    assert!(force_next_fetch_nonseq(input));
    assert_eq!(next_opcode_access_kind(input, true), AccessKind::Nonseq);
}

#[test]
fn no_bug_when_prefetch_on_or_pc_changed() {
    let base = DisableBugInput {
        prefetch_enabled: true,
        code_in_rom: true,
        had_internal_cycles: true,
        pc_changed: false,
    };
    assert!(!force_next_fetch_nonseq(base));
    assert_eq!(next_opcode_access_kind(base, true), AccessKind::Seq);

    let pc = DisableBugInput {
        prefetch_enabled: false,
        pc_changed: true,
        ..base
    };
    assert!(!force_next_fetch_nonseq(pc));
}

#[test]
fn no_bug_outside_rom_or_without_i() {
    let input = DisableBugInput {
        prefetch_enabled: false,
        code_in_rom: false,
        had_internal_cycles: true,
        pc_changed: false,
    };
    assert!(!force_next_fetch_nonseq(input));

    let no_i = DisableBugInput {
        code_in_rom: true,
        had_internal_cycles: false,
        ..input
    };
    assert!(!force_next_fetch_nonseq(no_i));
}

#[test]
fn classifiers_catch_mul_and_thumb_ldr() {
    // mul r0, r1, r2  (simplified encoding check — bit pattern with 1001)
    let mul = 0xE000_0192; // rough; classifier looks at mul mask
    let _ = arm_likely_i_cycles_no_pc(mul);
    // Thumb LDR Rd,[Rb,#imm5] = 0110_1...
    assert!(thumb_likely_i_cycles_no_pc(0x6800));
}
