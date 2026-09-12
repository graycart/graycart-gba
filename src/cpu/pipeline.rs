//! ARM7TDMI three-stage pipeline (Fetch → Decode → Execute) helpers.
//!
//! Cited: ARM7TDMI TRM (DDI0210C) §1.1.1 — instruction pipeline / PC skew
//!   https://developer.arm.com/documentation/ddi0210/c/
//! Cited: GBATEK — ARM CPU Overview (ARM vs Thumb PC reads)
//!   https://problemkaputt.de/gbatek.htm
//! Note: explicit prefetch slots for open-bus / flush fidelity; execute body
//! owned by arm/thumb streams.

use crate::bus::CpuMem;

/// Instruction set state for fetch width / PC alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IsaState {
    #[default]
    Arm,
    Thumb,
}

impl IsaState {
    pub const fn is_thumb(self) -> bool {
        matches!(self, Self::Thumb)
    }

    pub const fn insn_size(self) -> u32 {
        match self {
            Self::Arm => 4,
            Self::Thumb => 2,
        }
    }

    /// Architectural R15 read skew relative to the *executing* instruction address.
    pub const fn pc_read_skew(self) -> u32 {
        match self {
            Self::Arm => 8,
            Self::Thumb => 4,
        }
    }

    pub const fn from_cpsr_t(thumb: bool) -> Self {
        if thumb {
            Self::Thumb
        } else {
            Self::Arm
        }
    }
}

/// One fetched opcode sitting in a pipeline slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineSlot {
    /// Address of this opcode in memory.
    pub addr: u32,
    /// ARM: full word. Thumb: halfword in the low 16 bits.
    pub raw: u32,
}

/// Explicit Fetch / Decode slots. Execute holds the insn currently running
/// outside this struct (arm/thumb owners).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pipeline {
    pub isa: IsaState,
    /// Address of the instruction being **fetched** (architectural PC / R15 base).
    pub fetch_pc: u32,
    pub decode: Option<PipelineSlot>,
    pub fetch: Option<PipelineSlot>,
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new(IsaState::Arm, 0)
    }
}

impl Pipeline {
    pub fn new(isa: IsaState, fetch_pc: u32) -> Self {
        Self {
            isa,
            fetch_pc: align_pc(fetch_pc, isa),
            decode: None,
            fetch: None,
        }
    }

    /// Value presented when an instruction reads R15 (PC).
    ///
    /// Equals [`Self::fetch_pc`] after alignment — two instructions ahead of execute
    /// when the pipe is full (`ARM: exec+8`, `Thumb: exec+4`).
    pub fn r15_read(&self) -> u32 {
        self.fetch_pc
    }

    /// Address of the instruction currently in Decode (about to execute next), if any.
    pub fn decode_pc(&self) -> Option<u32> {
        self.decode.map(|s| s.addr)
    }

    /// Implied execute address when the pipe is full: `fetch_pc - skew`.
    pub fn implied_exec_pc(&self) -> u32 {
        self.fetch_pc.wrapping_sub(self.isa.pc_read_skew())
    }

    /// Drop Decode/Fetch slots (branch, exception, BX, PC write).
    pub fn flush(&mut self) {
        self.decode = None;
        self.fetch = None;
    }

    /// Branch / exception PC write: align, switch ISA, flush, set fetch PC.
    pub fn redirect(&mut self, dest: u32, isa: IsaState) {
        self.isa = isa;
        self.fetch_pc = align_pc(dest, isa);
        self.flush();
    }

    /// Apply an [`crate::cpu::exception::ExceptionEntryPlan`] PC/ISA side-effect.
    pub fn apply_exception_vector(&mut self, vector: u32) {
        // Exceptions always enter ARM.
        self.redirect(vector, IsaState::Arm);
    }

    /// Advance PC after a successful fetch into the Fetch slot.
    pub fn advance_fetch_pc(&mut self) {
        self.fetch_pc = self.fetch_pc.wrapping_add(self.isa.insn_size());
    }

    /// Shift Fetch → Decode and clear Fetch (call after execute retires).
    pub fn shift_decode(&mut self) {
        self.decode = self.fetch.take();
    }

    /// Fetch one opcode from `mem` at [`Self::fetch_pc`] into the Fetch slot,
    /// then advance fetch PC. Does not touch Decode.
    pub fn fetch_into<M: CpuMem>(&mut self, mem: &mut M) {
        let addr = self.fetch_pc;
        let raw = match self.isa {
            IsaState::Arm => mem.read32(addr),
            IsaState::Thumb => u32::from(mem.read16(addr)),
        };
        self.fetch = Some(PipelineSlot { addr, raw });
        self.advance_fetch_pc();
    }

    /// Refill both slots after a flush (branch / exception). Leaves `fetch_pc`
    /// two instructions ahead of the first decoded opcode when complete.
    pub fn refill<M: CpuMem>(&mut self, mem: &mut M) {
        self.flush();
        self.fetch_into(mem);
        self.shift_decode();
        self.fetch_into(mem);
    }

    pub fn is_flushed(&self) -> bool {
        self.decode.is_none() && self.fetch.is_none()
    }
}

/// Force PC alignment for the destination ISA (ARM clears [1:0], Thumb clears [0]).
pub fn align_pc(pc: u32, isa: IsaState) -> u32 {
    match isa {
        IsaState::Arm => pc & !0b11,
        IsaState::Thumb => pc & !0b1,
    }
}
