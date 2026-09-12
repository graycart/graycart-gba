//! Single-instruction CPU step over [`crate::bus::CpuMem`] (pipeline owner).
//!
//! Cited: ARM7TDMI TRM (DDI0210C) §1.1.1 — pipeline / PC skew
//!   https://developer.arm.com/documentation/ddi0210/c/
//! Cited: GBATEK — ARM CPU Overview / SWI
//!   https://problemkaputt.de/gbatek.htm
//! Note: crude 1-insn advance; waitstate-accurate scheduling is later. BiosHle
//! Div (SWI 0x06) is handled here so jsmolka fail-digit paths can settle.

use crate::bus::CpuMem;
use crate::cpu::arm::{self, ExecResult as ArmExec};
use crate::cpu::thumb::{self, ExecResult as ThumbExec, ThumbCore, ThumbMem};
use crate::cpu::{ExceptionKind, IsaState, Mode};

use super::Cpu;

/// Result of one architectural instruction retirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// Fell through (sequential PC).
    Ok,
    /// Branch / BX / exception return / PC write.
    Branched,
    /// SWI handled by BiosHle (Div, etc.) — PC advanced past the SWI.
    SwiHle,
    /// Took a real exception vector (no HLE handler).
    Exception(ExceptionKind),
}

/// Optional BiosHle hooks used while stepping (Div for jsmolka fail digits).
#[derive(Debug, Clone, Copy, Default)]
pub struct StepHle {
    /// When true, SWI `0x06` (Div) is emulated instead of vectoring to BIOS.
    pub bios_hle: bool,
}

impl StepHle {
    #[must_use]
    pub const fn bios_hle() -> Self {
        Self { bios_hle: true }
    }
}

/// Advance the CPU by one instruction from the Decode slot.
///
/// Refills the pipeline when flushed. Keeps [`Cpu::regs`] PC at the executing
/// instruction address for ARM (`R15` reads add +8 in execute) and at
/// `exec+4` for Thumb (matches Thumb unit harness).
pub fn step(cpu: &mut Cpu, bus: &mut impl CpuMem, hle: StepHle) -> StepOutcome {
    ensure_pipe(cpu, bus);

    let slot = match cpu.pipeline.decode {
        Some(s) => s,
        None => {
            // Empty media / zero ROM — treat as NOP advance failure surface.
            return StepOutcome::Ok;
        }
    };
    let isa = cpu.pipeline.isa;

    match isa {
        IsaState::Arm => step_arm(cpu, bus, hle, slot.addr, slot.raw),
        IsaState::Thumb => step_thumb(cpu, bus, hle, slot.addr, slot.raw as u16),
    }
}

fn ensure_pipe(cpu: &mut Cpu, bus: &mut impl CpuMem) {
    if cpu.pipeline.decode.is_none() {
        cpu.pipeline.refill(bus);
        sync_exec_pc(cpu);
    }
}

fn sync_exec_pc(cpu: &mut Cpu) {
    if let Some(d) = cpu.pipeline.decode {
        match cpu.pipeline.isa {
            IsaState::Arm => cpu.regs.set_pc(d.addr),
            // Thumb execute expects R15 == instr_pc + 4 on entry.
            IsaState::Thumb => cpu.regs.set_pc(d.addr.wrapping_add(4)),
        }
    }
}

fn advance_sequential(cpu: &mut Cpu, bus: &mut impl CpuMem) {
    cpu.pipeline.shift_decode();
    cpu.pipeline.fetch_into(bus);
    sync_exec_pc(cpu);
}

fn refill_after_branch(cpu: &mut Cpu, bus: &mut impl CpuMem) {
    let isa = IsaState::from_cpsr_t(cpu.regs.thumb());
    // ARM branch paths may already have redirected; Thumb only updates regs — always sync.
    cpu.pipeline.redirect(cpu.regs.pc(), isa);
    cpu.pipeline.refill(bus);
    sync_exec_pc(cpu);
}

fn step_arm(
    cpu: &mut Cpu,
    bus: &mut impl CpuMem,
    hle: StepHle,
    instr_pc: u32,
    raw: u32,
) -> StepOutcome {
    cpu.regs.set_pc(instr_pc);
    match arm::step(cpu, bus, raw) {
        ArmExec::Ok | ArmExec::CondFailed => {
            advance_sequential(cpu, bus);
            StepOutcome::Ok
        }
        ArmExec::Branched => {
            refill_after_branch(cpu, bus);
            StepOutcome::Branched
        }
        ArmExec::Exception(kind) => handle_exception(cpu, bus, hle, kind, instr_pc),
    }
}

fn step_thumb(
    cpu: &mut Cpu,
    bus: &mut impl CpuMem,
    hle: StepHle,
    instr_pc: u32,
    raw: u16,
) -> StepOutcome {
    cpu.regs.set_pc(instr_pc.wrapping_add(4));
    let instr = thumb::decode(raw);
    let result = {
        let mut view = ThumbView { cpu, bus };
        thumb::execute(&mut view, instr_pc, instr)
    };
    match result {
        ThumbExec::Continue => {
            // Architectural PC was skewed; sequential next halfword.
            cpu.regs.set_pc(instr_pc.wrapping_add(2));
            advance_sequential(cpu, bus);
            StepOutcome::Ok
        }
        ThumbExec::Branch => {
            refill_after_branch(cpu, bus);
            StepOutcome::Branched
        }
        ThumbExec::Swi => handle_exception(cpu, bus, hle, ExceptionKind::Swi, instr_pc),
        ThumbExec::Undef => handle_exception(cpu, bus, hle, ExceptionKind::Undefined, instr_pc),
    }
}

fn handle_exception(
    cpu: &mut Cpu,
    bus: &mut impl CpuMem,
    hle: StepHle,
    kind: ExceptionKind,
    instr_pc: u32,
) -> StepOutcome {
    if hle.bios_hle && kind == ExceptionKind::Swi && try_hle_swi(cpu, bus, instr_pc) {
        // Resume at next instruction (same as exception LR semantics).
        let next = if cpu.regs.thumb() {
            // CPSR.T still Thumb here — we have not taken the vector.
            instr_pc.wrapping_add(2)
        } else {
            instr_pc.wrapping_add(4)
        };
        let isa = IsaState::from_cpsr_t(cpu.regs.thumb());
        cpu.regs.set_pc(next);
        cpu.pipeline.redirect(next, isa);
        cpu.pipeline.refill(bus);
        sync_exec_pc(cpu);
        return StepOutcome::SwiHle;
    }

    cpu.take_exception(kind, instr_pc);
    cpu.pipeline.refill(bus);
    sync_exec_pc(cpu);
    StepOutcome::Exception(kind)
}

/// GBA BIOS SWI comment is in bits 16..=23 of the ARM imm24 / Thumb imm8 low byte.
fn swi_number(cpu: &Cpu, bus: &mut impl CpuMem, instr_pc: u32) -> u8 {
    if cpu.regs.thumb() {
        (bus.read16(instr_pc) & 0xFF) as u8
    } else {
        ((bus.read32(instr_pc) >> 16) & 0xFF) as u8
    }
}

fn try_hle_swi(cpu: &mut Cpu, bus: &mut impl CpuMem, instr_pc: u32) -> bool {
    match swi_number(cpu, bus, instr_pc) {
        0x06 => {
            // Div: r0=/ r1=% ; r3 = abs(quot). Cited: GBATEK BIOS Div.
            let num = cpu.regs.get(0) as i32;
            let den = cpu.regs.get(1) as i32;
            if den == 0 {
                // Hardware Div-by-zero is undefined; keep registers and succeed.
                return true;
            }
            let quot = num / den;
            let rem = num % den;
            cpu.regs.set(0, quot as u32);
            cpu.regs.set(1, rem as u32);
            cpu.regs.set(3, quot.unsigned_abs());
            true
        }
        // Soft-boot homebrew rarely needs more SWIs for arm/thumb/memory PASS path.
        _ => false,
    }
}

/// Combined Thumb view of CPU regs + bus memory.
struct ThumbView<'a, M: CpuMem> {
    cpu: &'a mut Cpu,
    bus: &'a mut M,
}

impl<M: CpuMem> ThumbCore for ThumbView<'_, M> {
    fn reg(&self, n: u8) -> u32 {
        self.cpu.regs.get(n)
    }
    fn set_reg(&mut self, n: u8, val: u32) {
        if n == 15 {
            self.cpu.regs.set_pc(val);
        } else {
            self.cpu.regs.set(n, val);
        }
    }
    fn n(&self) -> bool {
        self.cpu.regs.n()
    }
    fn z(&self) -> bool {
        self.cpu.regs.z()
    }
    fn c(&self) -> bool {
        self.cpu.regs.c()
    }
    fn v(&self) -> bool {
        self.cpu.regs.v()
    }
    fn set_nzcv(&mut self, n: bool, z: bool, c: bool, v: bool) {
        self.cpu.regs.set_nzcv(n, z, c, v);
    }
    fn thumb_state(&self) -> bool {
        self.cpu.regs.thumb()
    }
    fn set_thumb_state(&mut self, thumb: bool) {
        self.cpu.regs.set_thumb(thumb);
    }
    fn raise_swi(&mut self, _imm8: u8) {
        // Signalled via [`ThumbExec::Swi`]; exception/HLE owned by [`step`].
    }
    fn raise_undef(&mut self) {
        // Signalled via [`ThumbExec::Undef`].
    }
}

impl<M: CpuMem> ThumbMem for ThumbView<'_, M> {
    fn read8(&mut self, addr: u32) -> u8 {
        self.bus.read8(addr)
    }
    fn write8(&mut self, addr: u32, val: u8) {
        self.bus.write8(addr, val);
    }
    fn read16(&mut self, addr: u32) -> u16 {
        self.bus.read16(addr)
    }
    fn write16(&mut self, addr: u32, val: u16) {
        self.bus.write16(addr, val);
    }
    fn read32(&mut self, addr: u32) -> u32 {
        self.bus.read32(addr)
    }
    fn write32(&mut self, addr: u32, val: u32) {
        self.bus.write32(addr, val);
    }
}

/// Soft-boot stack pointers (common post-BIOS handoff values).
///
/// Cited: GBATEK — Memory Map / BIOS after startup (secondary community consensus)
///   https://problemkaputt.de/gbatek.htm
pub mod soft_boot {
    use super::*;

    pub const CART_ENTRY: u32 = 0x0800_0000;
    pub const MULTIBOOT_ENTRY: u32 = 0x0200_0000;
    pub const SP_USR: u32 = 0x0300_7F00;
    pub const SP_IRQ: u32 = 0x0300_7FA0;
    pub const SP_SVC: u32 = 0x0300_7FE0;

    /// Apply System-mode soft entry at `entry` (ARM, IRQ/FIQ masked off for SVC stacks).
    pub fn apply(cpu: &mut Cpu, entry: u32) {
        cpu.regs = crate::cpu::Regs::new();
        cpu.regs.set_mode(Mode::Supervisor);
        cpu.regs.set_r13_mode(Mode::Supervisor, SP_SVC);
        cpu.regs.set_r13_mode(Mode::Irq, SP_IRQ);
        cpu.regs.set_mode(Mode::System);
        cpu.regs.set_r13_mode(Mode::System, SP_USR);
        // Clear I/F so homebrew IRQs could fire later; keep ARM state.
        let cpsr =
            (cpu.regs.cpsr() & !crate::cpu::cpsr::I & !crate::cpu::cpsr::F) | Mode::System.bits();
        cpu.regs.set_cpsr(cpsr);
        cpu.regs.set_pc(entry);
        cpu.pipeline.redirect(entry, IsaState::Arm);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::FlatRam;

    #[test]
    fn soft_boot_then_branch_header_shape() {
        // Minimal: ARM `B +0` at entry (self) — just proves refill + step.
        let mut cpu = Cpu::new();
        let mut mem = FlatRam::new(16);
        soft_boot::apply(&mut cpu, 0);
        mem.write32(0, 0xEAFF_FFFE); // B .
        let _ = step(&mut cpu, &mut mem, StepHle::default());
        assert_eq!(cpu.pipeline.decode.unwrap().addr, 0);
    }

    #[test]
    fn hle_div_swi() {
        let mut cpu = Cpu::new();
        let mut mem = FlatRam::new(16);
        soft_boot::apply(&mut cpu, 0);
        // GBA SWI number in bits 16..=23 → SWI 0x06 Div.
        mem.write32(0, 0xEF06_0000);
        cpu.regs.set(0, 1234);
        cpu.regs.set(1, 10);
        let o = step(&mut cpu, &mut mem, StepHle::bios_hle());
        assert_eq!(o, StepOutcome::SwiHle);
        assert_eq!(cpu.regs.get(0), 123);
        assert_eq!(cpu.regs.get(1), 4);
    }
}
