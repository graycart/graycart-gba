//! ARM7TDMI exception entry helpers (vectors, LR/SPSR/CPSR plan).
//!
//! Cited: GBATEK — ARM CPU Exceptions
//!   https://problemkaputt.de/gbatek.htm#armcpuexceptions
//! Cited: ARM7TDMI TRM (DDI0210C) §2.8 — exception entry / return
//!   https://developer.arm.com/documentation/ddi0210/c/
//! Note: pure planning stubs for P1; register banking applied by regs owner.

/// GBA exception vector base (BIOS ROM at low addresses).
pub const VECTOR_BASE: u32 = 0x0000_0000;

/// User IRQ handler pointer the BIOS stub branches to (IWRAM).
pub const GBA_IRQ_HANDLER_PTR: u32 = 0x0300_7FFC;

/// CPSR bit: IRQ disable.
pub const CPSR_I: u32 = 1 << 7;
/// CPSR bit: FIQ disable.
pub const CPSR_F: u32 = 1 << 6;
/// CPSR bit: Thumb state (T=1).
pub const CPSR_T: u32 = 1 << 5;
/// CPSR mode field mask (M4–M0).
pub const CPSR_MODE_MASK: u32 = 0x1F;

/// Architectural mode bits written on exception entry (ARMv4T).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExceptionModeBits {
    User = 0x10,
    Fiq = 0x11,
    Irq = 0x12,
    Supervisor = 0x13,
    Abort = 0x17,
    Undefined = 0x1B,
    System = 0x1F,
}

/// Exception kinds with GBA vector offsets and entry policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExceptionKind {
    Reset,
    Undefined,
    Swi,
    PrefetchAbort,
    DataAbort,
    Irq,
    Fiq,
}

impl ExceptionKind {
    /// Vector address = [`VECTOR_BASE`] + offset.
    pub const fn vector_offset(self) -> u32 {
        match self {
            Self::Reset => 0x00,
            Self::Undefined => 0x04,
            Self::Swi => 0x08,
            Self::PrefetchAbort => 0x0C,
            Self::DataAbort => 0x10,
            // 0x14 is reserved (Address Exceeds 26-bit) on classic ARM; unused on GBA.
            Self::Irq => 0x18,
            Self::Fiq => 0x1C,
        }
    }

    pub const fn vector(self) -> u32 {
        VECTOR_BASE.wrapping_add(self.vector_offset())
    }

    pub const fn mode_bits(self) -> ExceptionModeBits {
        match self {
            Self::Reset | Self::Swi => ExceptionModeBits::Supervisor,
            Self::Undefined => ExceptionModeBits::Undefined,
            Self::PrefetchAbort | Self::DataAbort => ExceptionModeBits::Abort,
            Self::Irq => ExceptionModeBits::Irq,
            Self::Fiq => ExceptionModeBits::Fiq,
        }
    }

    /// All exceptions set I=1; Reset and FIQ also set F=1.
    pub const fn sets_f(self) -> bool {
        matches!(self, Self::Reset | Self::Fiq)
    }

    /// Recommended return instruction (documentation / tests).
    pub const fn return_op(self) -> &'static str {
        match self {
            Self::Reset => "n/a",
            Self::Undefined | Self::Swi => "MOVS PC, R14",
            Self::PrefetchAbort | Self::Irq | Self::Fiq => "SUBS PC, R14, #4",
            Self::DataAbort => "SUBS PC, R14, #8",
        }
    }

    /// Link-register value written to `R14_<mode>` on entry.
    ///
    /// `pc` semantics (ARM ARM / GBATEK):
    /// - `Undefined` / `Swi`: address of the trapping instruction
    /// - `PrefetchAbort` / `DataAbort`: address of the aborted instruction
    /// - `Irq` / `Fiq`: address of the instruction to resume (next to execute)
    /// - `Reset`: unused (returns 0)
    ///
    /// Thumb uses the same *formulas* as ARM for aborts/IRQ/FIQ (`+4` / `+8`);
    /// SWI/Undef advance by the instruction size so `MOVS PC,R14` resumes correctly.
    pub fn link_register(self, pc: u32, thumb: bool) -> u32 {
        let insn_size = if thumb { 2 } else { 4 };
        match self {
            Self::Reset => 0,
            Self::Undefined | Self::Swi => pc.wrapping_add(insn_size),
            Self::PrefetchAbort | Self::Irq | Self::Fiq => pc.wrapping_add(4),
            Self::DataAbort => pc.wrapping_add(8),
        }
    }
}

/// Pure result of exception entry — apply via banked regs when available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExceptionEntryPlan {
    pub kind: ExceptionKind,
    /// Value for `R14_<new mode>`.
    pub lr: u32,
    /// Snapshot of CPSR before entry (`SPSR_<new mode>`).
    pub spsr: u32,
    /// CPSR after entry (ARM state, new mode, I/F policy).
    pub new_cpsr: u32,
    /// Forced fetch PC (vector).
    pub pc: u32,
}

impl ExceptionEntryPlan {
    /// Mode field of [`Self::new_cpsr`].
    pub fn mode_bits(self) -> u8 {
        (self.new_cpsr & CPSR_MODE_MASK) as u8
    }

    pub fn thumb_state(self) -> bool {
        self.new_cpsr & CPSR_T != 0
    }

    pub fn irq_disabled(self) -> bool {
        self.new_cpsr & CPSR_I != 0
    }

    pub fn fiq_disabled(self) -> bool {
        self.new_cpsr & CPSR_F != 0
    }
}

/// Plan exception entry without mutating register banks.
///
/// Always forces ARM state (`T=0`). Leaves NZCV and other non-control CPSR bits
/// intact. Does **not** flush the pipeline — caller should
/// [`crate::cpu::pipeline::Pipeline::flush`] after applying PC.
pub fn plan_entry(kind: ExceptionKind, cpsr: u32, pc: u32) -> ExceptionEntryPlan {
    let thumb = cpsr & CPSR_T != 0;
    let lr = kind.link_register(pc, thumb);
    let mut new_cpsr = cpsr;
    new_cpsr &= !CPSR_T;
    new_cpsr = (new_cpsr & !CPSR_MODE_MASK) | u32::from(kind.mode_bits() as u8);
    new_cpsr |= CPSR_I;
    if kind.sets_f() {
        new_cpsr |= CPSR_F;
    }
    ExceptionEntryPlan {
        kind,
        lr,
        spsr: cpsr,
        new_cpsr,
        pc: kind.vector(),
    }
}

/// GBATEK / ARM cycle table: exception / SWI entry ≈ **2S+1N** (pipeline refill).
/// Waitstate pricing is a bus concern (P2+); this is a named stub for later timing.
pub const EXCEPTION_ENTRY_BUS_HINT: &str = "2S+1N";
