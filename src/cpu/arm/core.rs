//! ARM execute helpers over the shared [`crate::cpu::Cpu`] register API.
//!
//! Cited: GBATEK -- ARM CPU Register Set / pipeline PC skew
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI0210C -- programmer's model / data-processing operands
//! Note: R15 reads add +8 (ARM) normally; register-specified shifts use +12
//! (extra I-cycle). PC writes go through `Regs::set_pc` / `branch_exchange`.

use crate::cpu::Cpu;

/// ARM pipeline skew for a normal R15 operand read (`instr + 8`).
pub const PC_SKEW_ARM: u32 = 8;

/// ARM pipeline skew when R15 is Rn/Rm with a **register-specified** shift
/// (`instr + 12`). The extra internal cycle for `SHIFT(Rs)` advances PC one
/// more instruction before the operand is sampled.
///
/// Cited: ARM DDI0100 / DDI0210C data-processing operand rules; GBATEK ALU
/// cycle note (`+1I if SHIFT(Rs)`). Confirmed by jsmolka `arm` tests 224–225.
pub const PC_SKEW_ARM_REG_SHIFT: u32 = 12;

/// Read GPR with ARM pipeline skew on R15 (`pc + 8`).
///
/// Prefer aligning with [`crate::cpu::Pipeline::r15_read`] once the step loop
/// keeps `regs.pc` and `pipeline.fetch_pc` in sync; until then architectural
/// `Regs::pc` + 8 matches the documented ARM skew.
#[inline]
pub fn read_reg(cpu: &Cpu, idx: u8) -> u32 {
    read_reg_skew(cpu, idx, PC_SKEW_ARM)
}

/// Read GPR with an explicit R15 skew (8 for normal ARM ops, 12 for Rs-shifts).
#[inline]
pub fn read_reg_skew(cpu: &Cpu, idx: u8, pc_skew: u32) -> u32 {
    if idx == 15 {
        cpu.regs.pc().wrapping_add(pc_skew)
    } else {
        cpu.regs.get(idx)
    }
}

/// Write GPR; R15 uses force-aligned [`crate::cpu::Regs::set_pc`].
#[inline]
pub fn write_reg(cpu: &mut Cpu, idx: u8, value: u32) {
    if idx == 15 {
        cpu.regs.set_pc(value);
    } else {
        cpu.regs.set(idx, value);
    }
}
