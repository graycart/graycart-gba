//! ARM execute helpers over the shared [`crate::cpu::Cpu`] register API.
//!
//! Cited: GBATEK -- ARM CPU Register Set / pipeline PC skew
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI0210C -- programmer's model
//! Note: R15 reads add +8 (ARM); PC writes go through `Regs::set_pc` / `branch_exchange`.

use crate::cpu::Cpu;

/// Read GPR with ARM pipeline skew on R15 (`pc + 8`).
///
/// Prefer aligning with [`crate::cpu::Pipeline::r15_read`] once the step loop
/// keeps `regs.pc` and `pipeline.fetch_pc` in sync; until then architectural
/// `Regs::pc` + 8 matches the documented ARM skew.
#[inline]
pub fn read_reg(cpu: &Cpu, idx: u8) -> u32 {
    if idx == 15 {
        cpu.regs.pc().wrapping_add(8)
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
