//! Interrupt controller: IE / IF (W1C) / IME + CPU IRQ line sample (P3).
//!
//! Cited: GBATEK — Interrupt Control / BIOS Interrupt Handler
//!   https://problemkaputt.de/gbatek.htm#gbainterruptcontrol
//! Cited: GBATEK — BIOS Halt / IntrWait (check flags @ `03007FF8h`)
//!   https://problemkaputt.de/gbatek.htm#bioshaltfunctions
//! Cited: Tonc — Hardware interrupts (IF acknowledge pitfalls)
//!   https://www.coranac.com/tonc/text/interrupts.htm
//! Cross-check: research `docs/graycart-gba/05-io-timers-irq-input.md` §4–5, §8.
//! Note: IRQ delay mid-instruction / DMA preempt / Stop fidelity are TBD
//! ([05] §10 IO-TBD-4). Functional path samples at instruction boundaries.

use crate::cpu::{Cpu, ExceptionEntryPlan, ExceptionKind};

#[cfg(test)]
mod tests;

/// `IE` — Interrupt Enable (`04000200`).
pub const IE_ADDR: u32 = 0x0400_0200;
/// `IF` — Interrupt Request Flags / acknowledge (`04000202`).
pub const IF_ADDR: u32 = 0x0400_0202;
/// `IME` — Interrupt Master Enable (`04000208`).
pub const IME_ADDR: u32 = 0x0400_0208;

/// BIOS IntrWait / Interrupt Check Flags halfword in IWRAM (`03007FF8h`).
///
/// User ISR must OR acknowledged IF bits into this word or IntrWait never exits
/// (GBATEK). Same bit layout as IE/IF. BIOS/HLE concern for the trampoline that
/// loads `[03007FFC]`; this module only provides helpers.
pub const INTR_WAIT_FLAGS_ADDR: u32 = 0x0300_7FF8;

/// User IRQ handler pointer slot (`03007FFCh`) — BIOS stub loads this; see
/// [`crate::cpu::GBA_IRQ_HANDLER_PTR`].
pub const IRQ_HANDLER_PTR_ADDR: u32 = 0x0300_7FFC;

/// IWRAM base for IntrWait helper offset math.
const IWRAM_BASE: u32 = 0x0300_0000;

/// Implemented IE/IF source bits (0–13). Bits 14–15 unused; masked on write.
pub const IRQ_SOURCE_MASK: u16 = 0x3FFF;

/// LCD VBlank.
pub const IRQ_VBLANK: u16 = 1 << 0;
/// LCD HBlank.
pub const IRQ_HBLANK: u16 = 1 << 1;
/// LCD VCount match.
pub const IRQ_VCOUNT: u16 = 1 << 2;
/// Timer 0 overflow.
pub const IRQ_TIMER0: u16 = 1 << 3;
/// Timer 1 overflow.
pub const IRQ_TIMER1: u16 = 1 << 4;
/// Timer 2 overflow.
pub const IRQ_TIMER2: u16 = 1 << 5;
/// Timer 3 overflow.
pub const IRQ_TIMER3: u16 = 1 << 6;
/// Serial / SIO.
pub const IRQ_SERIAL: u16 = 1 << 7;
/// DMA 0 complete.
pub const IRQ_DMA0: u16 = 1 << 8;
/// DMA 1 complete.
pub const IRQ_DMA1: u16 = 1 << 9;
/// DMA 2 complete.
pub const IRQ_DMA2: u16 = 1 << 10;
/// DMA 3 complete.
pub const IRQ_DMA3: u16 = 1 << 11;
/// Keypad.
pub const IRQ_KEYPAD: u16 = 1 << 12;
/// Game Pak /IRQ.
pub const IRQ_GAMEPAK: u16 = 1 << 13;

/// Named candidate for later IRQ line delay (mGBA uses 7). **Not applied** yet —
/// functional P3 samples immediately at insn boundaries (IO-TBD-4).
pub const IRQ_DELAY_CYCLES_TBD: u32 = 7;

/// GBA interrupt controller (IE / IF / IME).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Irq {
    /// Interrupt Enable mask (`IE`).
    ie: u16,
    /// Pending request flags (`IF`) — sticky until W1C acknowledge.
    if_: u16,
    /// Master enable (`IME` bit0).
    ime: bool,
}

impl Default for Irq {
    fn default() -> Self {
        Self::new()
    }
}

impl Irq {
    /// Power-on: all sources disabled, no pending, IME clear.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ie: 0,
            if_: 0,
            ime: false,
        }
    }

    // --- Register accessors (MMIO semantics) ---

    #[inline]
    #[must_use]
    pub fn ie(&self) -> u16 {
        self.ie
    }

    #[inline]
    #[must_use]
    pub fn if_flags(&self) -> u16 {
        self.if_
    }

    #[inline]
    #[must_use]
    pub fn ime(&self) -> bool {
        self.ime
    }

    /// Read `IE` (`04000200`).
    #[inline]
    #[must_use]
    pub fn read_ie(&self) -> u16 {
        self.ie
    }

    /// Write `IE` — unused bits 14–15 masked off.
    #[inline]
    pub fn write_ie(&mut self, value: u16) {
        self.ie = value & IRQ_SOURCE_MASK;
    }

    /// Read `IF` (`04000202`).
    #[inline]
    #[must_use]
    pub fn read_if(&self) -> u16 {
        self.if_
    }

    /// Write `IF` — **write-1-to-clear** acknowledge (GBATEK / Tonc).
    ///
    /// Writing 0 leaves bits unchanged; writing 1 clears the corresponding bit.
    #[inline]
    pub fn write_if_ack(&mut self, value: u16) {
        self.if_ &= !(value & IRQ_SOURCE_MASK);
    }

    /// Read `IME` as a 32-bit I/O word (only bit0 meaningful).
    #[inline]
    #[must_use]
    pub fn read_ime(&self) -> u32 {
        u32::from(self.ime)
    }

    /// Write `IME` — only bit0 is kept.
    #[inline]
    pub fn write_ime(&mut self, value: u32) {
        self.ime = value & 1 != 0;
    }

    /// Convenience: set/clear IME bit0.
    #[inline]
    pub fn set_ime(&mut self, enabled: bool) {
        self.ime = enabled;
    }

    // --- Public API for timers / input / hw / DMA ---

    /// Raise one or more IF bits (OR). Does **not** require IME (GBATEK).
    ///
    /// Local peripheral enables (e.g. timer IRQ enable) are the caller's duty
    /// before calling this.
    #[inline]
    pub fn raise(&mut self, bits: u16) {
        self.if_ |= bits & IRQ_SOURCE_MASK;
    }

    /// `(IE & IF)` — Halt wake condition (IME / CPSR.I don't-care).
    ///
    /// Hw stream: pass this to [`crate::hw::Hw::poll_halt_wake`].
    #[inline]
    #[must_use]
    pub fn ie_and_if(&self) -> u16 {
        self.ie & self.if_
    }

    /// True when any enabled source is pending (Halt wake / line pre-IME).
    #[inline]
    #[must_use]
    pub fn halt_wake_pending(&self) -> bool {
        self.ie_and_if() != 0
    }

    /// CPU IRQ line: `IME && (IE & IF) != 0 && !cpsr_i`.
    #[inline]
    #[must_use]
    pub fn cpu_irq_asserted(&self, cpsr_i: bool) -> bool {
        self.ime && self.halt_wake_pending() && !cpsr_i
    }

    /// Sample the pending IRQ line and take a CPU IRQ exception when asserted.
    ///
    /// `resume_pc` is the instruction to resume after IRQ (passed to
    /// [`Cpu::take_exception`] / [`ExceptionKind::link_register`] Irq semantics).
    ///
    /// Does **not** refill the pipeline from the bus — caller / Gba step should
    /// refill after a `Some` return. IRQ delay ([`IRQ_DELAY_CYCLES_TBD`]) is not
    /// applied (TBD).
    ///
    /// BIOS vector at `0x18` eventually loads `[03007FFC]` — that trampoline is
    /// BIOS/HLE, not this controller.
    pub fn try_take_cpu_irq(&self, cpu: &mut Cpu, resume_pc: u32) -> Option<ExceptionEntryPlan> {
        if !self.cpu_irq_asserted(cpu.regs.irq_disabled()) {
            return None;
        }
        Some(cpu.take_exception(ExceptionKind::Irq, resume_pc))
    }

    /// Like [`Self::try_take_cpu_irq`], using Decode-slot PC as the resume address
    /// when the pipe has a next instruction; else `cpu.regs.pc()`.
    pub fn try_service_cpu(&self, cpu: &mut Cpu) -> Option<ExceptionEntryPlan> {
        let resume = cpu.pipeline.decode_pc().unwrap_or_else(|| cpu.regs.pc());
        self.try_take_cpu_irq(cpu, resume)
    }
}

/// Bridge for keypad / other peripherals that raise IF via [`crate::input::RaiseIf`].
impl crate::input::RaiseIf for Irq {
    #[inline]
    fn raise_if(&mut self, bits: u16) {
        self.raise(bits);
    }
}

// --- IntrWait flag helpers (IWRAM @ 03007FF8) ---

/// Byte offset of IntrWait check flags within IWRAM.
#[inline]
#[must_use]
pub const fn intr_wait_flags_iwram_offset() -> usize {
    (INTR_WAIT_FLAGS_ADDR - IWRAM_BASE) as usize
}

/// Read the BIOS Interrupt Check Flags halfword from an IWRAM image.
#[must_use]
pub fn read_intr_wait_flags(iwram: &[u8]) -> u16 {
    let off = intr_wait_flags_iwram_offset();
    if iwram.len() < off + 2 {
        return 0;
    }
    u16::from(iwram[off]) | (u16::from(iwram[off + 1]) << 8)
}

/// OR `bits` into the IntrWait check flags (user ISR acknowledge companion).
///
/// GBATEK: ISR must update `[03007FF8]` when acknowledging IF or IntrWait spins.
pub fn or_intr_wait_flags(iwram: &mut [u8], bits: u16) {
    let off = intr_wait_flags_iwram_offset();
    if iwram.len() < off + 2 {
        return;
    }
    let cur = u16::from(iwram[off]) | (u16::from(iwram[off + 1]) << 8);
    let next = cur | (bits & IRQ_SOURCE_MASK);
    iwram[off] = next as u8;
    iwram[off + 1] = (next >> 8) as u8;
}

/// Clear selected bits in the IntrWait check flags (IntrWait discard-old path).
pub fn clear_intr_wait_flags(iwram: &mut [u8], bits: u16) {
    let off = intr_wait_flags_iwram_offset();
    if iwram.len() < off + 2 {
        return;
    }
    let cur = u16::from(iwram[off]) | (u16::from(iwram[off + 1]) << 8);
    let next = cur & !(bits & IRQ_SOURCE_MASK);
    iwram[off] = next as u8;
    iwram[off + 1] = (next >> 8) as u8;
}
