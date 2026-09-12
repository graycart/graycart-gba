//! ARM7TDMI CPU core (P1 bring-up).
//!
//! Cited: GBATEK -- ARM CPU Overview / Register Set / Flags / Exceptions
//!   https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI0210C (ARM7TDMI TRM r4p1) -- programmer's model, pipeline
//!   https://developer.arm.com/documentation/ddi0210/c/
//! Note: registers/modes + pipeline/exception stubs; ARM decode/execute in
//! [`arm`]; Thumb decode/execute in [`thumb`].
//!
//! Research: Project store `docs/graycart-gba/01-cpu-arm7tdmi.md`.

pub mod arm;
mod mode;
mod regs;
pub mod step;
pub mod thumb;
pub mod timing;

pub mod exception;
pub mod pipeline;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_timing;

pub use exception::{
    plan_entry, ExceptionEntryPlan, ExceptionKind, ExceptionModeBits, EXCEPTION_ENTRY_BUS_HINT,
    GBA_IRQ_HANDLER_PTR, VECTOR_BASE,
};
pub use mode::Mode;
pub use pipeline::{align_pc, IsaState, Pipeline, PipelineSlot};
pub use regs::{cpsr, Regs};
pub use step::{soft_boot, step, StepHle, StepOutcome};
pub use timing::{price_insn, InsnCycles, TimingInput};

/// ARM7TDMI core shell. ISA execute lives in [`arm`] / [`thumb`].
#[derive(Debug, Default)]
pub struct Cpu {
    pub regs: Regs,
    pub pipeline: Pipeline,
}

impl Cpu {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enter an exception: bank LR/SPSR, write CPSR, set PC, flush pipeline to vector.
    ///
    /// `pc` semantics match [`exception::ExceptionKind::link_register`].
    pub fn take_exception(&mut self, kind: ExceptionKind, pc: u32) -> ExceptionEntryPlan {
        let plan = plan_entry(kind, self.regs.cpsr(), pc);
        let mode = Mode::from_bits(u32::from(plan.mode_bits()))
            .expect("exception entry uses architectural mode bits");

        // Bank SPSR/LR for the *target* mode before CPSR mode switch is visible to
        // current-mode helpers; banking is dynamic so order is still documented.
        self.regs.set_spsr_of(mode, plan.spsr);
        self.regs.set_r14_mode(mode, plan.lr);
        self.regs.set_cpsr(plan.new_cpsr);
        self.regs.set_pc(plan.pc);
        self.pipeline.apply_exception_vector(plan.pc);
        plan
    }
}
