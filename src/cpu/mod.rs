//! ARM7TDMI.
//!
//! Cited: GBATEK, ARM CPU Reference. https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI 0210C, ARM7TDMI Technical Reference Manual.

mod exec;
mod shift;

use crate::bus::Bus;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StepError {
    Unimplemented { pc: u32, mnemonic: String },
}

impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepError::Unimplemented { pc, mnemonic } => {
                write!(f, "unimplemented pc={pc:#010X} mnemonic={mnemonic}")
            }
        }
    }
}

impl std::error::Error for StepError {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Cpu {
    gpr: [u32; 16],
    usr: [u32; 5],
    fiq: [u32; 5],
    r13b: [u32; 6],
    r14b: [u32; 6],
    spsr_bank: [u32; 6],
    cpsr: u32,
    pub fetch_pc: u32,
    pub exec_pc: u32,
    pub idle: bool,
    pub faults: u32,
    /// Not restored from disk snapshots (always `"restored"` after load).
    #[serde(skip, default = "default_last_op")]
    pub last_op: &'static str,
    /// When `Some`, Halted for IntrWait / VBlankIntrWait until `0x03007FF8` matches.
    #[serde(default)]
    intr_wait_mask: Option<u16>,
    /// Game PC to resume after a Halt IRQ. The stacked BIOS link is not that PC.
    #[serde(default)]
    halt_resume: Option<u32>,
    /// Opcodes read two instructions ahead, before a later store can overwrite them.
    #[serde(default)]
    ahead: [u32; 2],
    #[serde(default)]
    ahead_pc: [u32; 2],
    #[serde(default)]
    ahead_len: u8,
}

fn default_last_op() -> &'static str {
    "restored"
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        let mut cpu = Self {
            gpr: [0; 16],
            usr: [0; 5],
            fiq: [0; 5],
            r13b: [0; 6],
            r14b: [0; 6],
            spsr_bank: [0; 6],
            cpsr: 0x1F,
            fetch_pc: 0x0800_0000,
            exec_pc: 0x0800_0000,
            idle: false,
            faults: 0,
            last_op: "reset",
            intr_wait_mask: None,
            halt_resume: None,
            ahead: [0; 2],
            ahead_pc: [0; 2],
            ahead_len: 0,
        };
        cpu.gpr[13] = 0x0300_7F00;
        cpu.r13b[0] = 0x0300_7F00;
        cpu.r13b[2] = 0x0300_7FA0;
        cpu.r13b[3] = 0x0300_7FE0;
        cpu
    }

    pub fn reg(&self, index: u32) -> u32 {
        self.gpr[index as usize]
    }

    /// Test helper: set a GPR before a SWI under test.
    pub fn set_reg_for_test(&mut self, index: u32, value: u32) {
        self.gpr[index as usize] = value;
    }

    pub fn cpsr(&self) -> u32 {
        self.cpsr
    }

    pub fn thumb(&self) -> bool {
        self.cpsr & 0x20 != 0
    }

    /// Test helper: run HLE SWI `number` (comment field bits 16..23).
    pub fn swi_number_for_test(&mut self, bus: &mut Bus, number: u32) {
        self.swi(bus, number << 16);
    }

    /// While halted for IntrWait, check `0x03007FF8` against the wait mask.
    pub fn poll_intr_wait(&mut self, bus: &mut Bus) {
        let Some(mask) = self.intr_wait_mask else {
            return;
        };
        if !bus.halted {
            return;
        }
        let flags = bus.irq.check_flags();
        if flags & mask == 0 {
            return;
        }
        bus.irq.set_check_flags(flags & !mask);
        bus.halted = false;
        self.intr_wait_mask = None;
        self.finish_swi_return();
    }

    /// True while IntrWait / VBlankIntrWait still owns halt.
    pub fn intr_waiting(&self) -> bool {
        self.intr_wait_mask.is_some()
    }

    pub fn step(&mut self, bus: &mut Bus) -> Result<(), StepError> {
        // HLE BIOS IRQ return stub (real ROM at 0x138): ldmfd + subs pc, lr, #4.
        if self.fetch_pc == IRQ_RETURN_STUB {
            self.hle_irq_return(bus);
            return Ok(());
        }
        if self.thumb() {
            let pc = self.fetch_pc;
            let instr = match self.take_ahead(pc) {
                Some(bits) => {
                    let _ = bus.fetch16(pc);
                    bits as u16
                }
                None => bus.fetch16(pc),
            };
            if let Some(bits) = bus.peek_code(pc.wrapping_add(4), 2) {
                self.push_ahead(pc.wrapping_add(4), bits);
            }
            self.exec_pc = pc;
            self.fetch_pc = pc.wrapping_add(2);
            let seq = self.fetch_pc;
            let result = self.exec_thumb(bus, instr);
            if self.fetch_pc != seq {
                self.ahead_len = 0;
            }
            result
        } else {
            let pc = self.fetch_pc;
            let instr = match self.take_ahead(pc) {
                Some(bits) => {
                    let _ = bus.fetch32(pc);
                    bits
                }
                None => bus.fetch32(pc),
            };
            if let Some(bits) = bus.peek_code(pc.wrapping_add(8), 4) {
                self.push_ahead(pc.wrapping_add(8), bits);
            }
            self.exec_pc = pc;
            self.fetch_pc = pc.wrapping_add(4);
            let seq = self.fetch_pc;
            let result = self.exec_arm(bus, instr);
            if self.fetch_pc != seq {
                self.ahead_len = 0;
            }
            result
        }
    }

    fn take_ahead(&mut self, pc: u32) -> Option<u32> {
        while self.ahead_len > 0 && self.ahead_pc[0] < pc {
            self.ahead[0] = self.ahead[1];
            self.ahead_pc[0] = self.ahead_pc[1];
            self.ahead_len -= 1;
        }
        if self.ahead_len > 0 && self.ahead_pc[0] == pc {
            let instr = self.ahead[0];
            self.ahead[0] = self.ahead[1];
            self.ahead_pc[0] = self.ahead_pc[1];
            self.ahead_len -= 1;
            Some(instr)
        } else {
            None
        }
    }

    fn push_ahead(&mut self, pc: u32, instr: u32) {
        if self.ahead_len == 2 {
            self.ahead[0] = self.ahead[1];
            self.ahead_pc[0] = self.ahead_pc[1];
            self.ahead_len = 1;
        }
        let index = self.ahead_len as usize;
        self.ahead[index] = instr;
        self.ahead_pc[index] = pc;
        self.ahead_len += 1;
    }

    /// IRQ entry. With no BIOS image, HLE the vector: IRQ mode, LR → return stub,
    /// latch the during-IRQ prefetch word, jump to `[0x03007FFC]`.
    pub fn raise_irq(&mut self, bus: &mut Bus) {
        self.raise_irq_from(bus, false);
    }

    /// IRQ entry. `from_halt` is the Halt wakeup path: the BIOS prologue runs
    /// before `[0x03007FFC]`, which is 42 cycles on the halt-pc timer (0xAD → 0xD7).
    pub fn raise_irq_from(&mut self, bus: &mut Bus, from_halt: bool) {
        if self.cpsr & 0x80 != 0 {
            return;
        }
        let spsr = self.cpsr;
        let ret = self.fetch_pc.wrapping_add(4);
        self.write_cpsr(0x92, 0xFFFF_FFFF);
        self.set_spsr(spsr, 0xFFFF_FFFF);
        if from_halt {
            bus.elapse(42);
            self.halt_resume = Some(self.fetch_pc);
            self.gpr[14] = 0x74;
        } else {
            self.gpr[14] = ret;
        }

        // stmfd sp!, {r0-r3,r12,lr} — same stack frame the BIOS builds before the user ISR.
        let sp = self.gpr[13].wrapping_sub(24);
        self.gpr[13] = sp;
        bus.write32(sp, self.gpr[0]);
        bus.write32(sp.wrapping_add(4), self.gpr[1]);
        bus.write32(sp.wrapping_add(8), self.gpr[2]);
        bus.write32(sp.wrapping_add(12), self.gpr[3]);
        bus.write32(sp.wrapping_add(16), self.gpr[12]);
        bus.write32(sp.wrapping_add(20), self.gpr[14]);

        self.gpr[14] = IRQ_RETURN_STUB;
        bus.set_bios_prefetch(LATCH_DURING_IRQ);
        let handler = bus.read32(0x0300_7FFC);
        self.fetch_pc = handler & !3;
        self.last_op = "irq";
    }

    /// `ldmfd sp!, {r0-r3,r12,lr}` then `subs pc, lr, #4` (SPSR → CPSR).
    fn hle_irq_return(&mut self, bus: &mut Bus) {
        bus.set_bios_prefetch(LATCH_AFTER_IRQ);
        let sp = self.gpr[13];
        self.gpr[0] = bus.read32(sp);
        self.gpr[1] = bus.read32(sp.wrapping_add(4));
        self.gpr[2] = bus.read32(sp.wrapping_add(8));
        self.gpr[3] = bus.read32(sp.wrapping_add(12));
        self.gpr[12] = bus.read32(sp.wrapping_add(16));
        self.gpr[14] = bus.read32(sp.wrapping_add(20));
        self.gpr[13] = sp.wrapping_add(24);

        let spsr = self.spsr();
        let bios_halt = self.gpr[14] == 0x74;
        let resume = self.halt_resume.take();
        let ret = if bios_halt {
            resume.unwrap_or(self.gpr[14])
        } else {
            self.gpr[14].wrapping_sub(4)
        };
        self.write_cpsr(spsr, 0xFFFF_FFFF);
        if self.thumb() {
            self.fetch_pc = ret & !1;
        } else {
            self.fetch_pc = ret & !3;
        }
        self.last_op = "irq";
    }

    fn carry(&self) -> bool {
        self.cpsr & 0x2000_0000 != 0
    }

    fn set_carry(&mut self, carry: bool) {
        if carry {
            self.cpsr |= 0x2000_0000;
        } else {
            self.cpsr &= !0x2000_0000;
        }
    }

    fn set_nz(&mut self, result: u32) {
        self.cpsr &= !0xC000_0000;
        self.cpsr |= result & 0x8000_0000;
        if result == 0 {
            self.cpsr |= 0x4000_0000;
        }
    }

    fn set_cv(&mut self, carry: bool, overflow: bool) {
        self.cpsr &= !0x3000_0000;
        if carry {
            self.cpsr |= 0x2000_0000;
        }
        if overflow {
            self.cpsr |= 0x1000_0000;
        }
    }

    fn read_gpr(&self, n: usize, pc: u32) -> u32 {
        if n == 15 { pc } else { self.gpr[n] }
    }

    fn write_gpr(&mut self, n: usize, value: u32) {
        if n == 15 {
            self.branch(value, false);
        } else {
            self.gpr[n] = value;
        }
    }

    fn read_user(&self, n: usize) -> u32 {
        let mode = self.cpsr & 0x1F;
        if mode == 0x10 || mode == 0x1F {
            return self.gpr[n];
        }
        match n {
            8..=12 if mode == 0x11 => self.usr[n - 8],
            13 => self.r13b[0],
            14 => self.r14b[0],
            _ => self.gpr[n],
        }
    }

    fn write_user(&mut self, n: usize, value: u32) {
        let mode = self.cpsr & 0x1F;
        if mode == 0x10 || mode == 0x1F {
            self.gpr[n] = value;
            return;
        }
        match n {
            8..=12 if mode == 0x11 => self.usr[n - 8] = value,
            13 => self.r13b[0] = value,
            14 => self.r14b[0] = value,
            _ => self.gpr[n] = value,
        }
    }

    fn branch(&mut self, value: u32, thumb: bool) {
        let was_thumb = self.cpsr & 0x20 != 0;
        if thumb {
            self.cpsr |= 0x20;
            self.fetch_pc = value & !1;
        } else {
            self.cpsr &= !0x20;
            self.fetch_pc = value & !3;
        }
        // The lookahead slot was fetched at the old width. A Thumb halfword
        // must not be executed as the ARM word at the same address.
        if was_thumb != thumb {
            self.ahead_len = 0;
        }
    }

    fn write_cpsr(&mut self, value: u32, mask: u32) {
        let old_mode = self.cpsr & 0x1F;
        let mask = if old_mode == 0x10 {
            mask & 0xF000_0000
        } else {
            mask
        };
        let mut merged = (self.cpsr & !mask) | (value & mask);
        // An MSR that clears mode bit 4 is ignored for that bit. Hardware keeps
        // it set, so mode 0x03 lands as Supervisor 0x13 (alyosha psr).
        if mask & 0x10 != 0 && merged & 0x10 == 0 {
            merged |= 0x10;
        }
        let new_mode = merged & 0x1F;
        if bank_index(old_mode) != bank_index(new_mode) || (old_mode == 0x11) != (new_mode == 0x11)
        {
            self.save_bank(old_mode);
            self.load_bank(new_mode);
        }
        self.cpsr = merged;
    }

    fn spsr(&self) -> u32 {
        let bank = bank_index(self.cpsr & 0x1F);
        // User and System have no SPSR. MRS reads CPSR (alyosha arm t254).
        if bank == 0 {
            self.cpsr
        } else {
            self.spsr_bank[bank]
        }
    }

    fn set_spsr(&mut self, value: u32, mask: u32) {
        let bank = bank_index(self.cpsr & 0x1F);
        if bank == 0 {
            return;
        }
        self.spsr_bank[bank] = (self.spsr_bank[bank] & !mask) | (value & mask);
    }

    fn save_bank(&mut self, mode: u32) {
        let bank = bank_index(mode);
        self.r13b[bank] = self.gpr[13];
        self.r14b[bank] = self.gpr[14];
        if mode == 0x11 {
            self.fiq.copy_from_slice(&self.gpr[8..13]);
        } else {
            self.usr.copy_from_slice(&self.gpr[8..13]);
        }
    }

    fn load_bank(&mut self, mode: u32) {
        let bank = bank_index(mode);
        self.gpr[13] = self.r13b[bank];
        self.gpr[14] = self.r14b[bank];
        if mode == 0x11 {
            self.gpr[8..13].copy_from_slice(&self.fiq);
        } else {
            self.gpr[8..13].copy_from_slice(&self.usr);
        }
    }

    fn swi(&mut self, bus: &mut Bus, comment: u32) {
        let number = (comment >> 16) & 0xFF;
        if number == 2 {
            bus.halted = true;
            self.last_op = "swi";
            return;
        }
        let spsr = self.cpsr;
        let ret = self.exec_pc.wrapping_add(if self.thumb() { 2 } else { 4 });
        self.write_cpsr(0x93, 0xFFFF_FFFF);
        self.set_spsr(spsr, 0xFFFF_FFFF);
        self.gpr[14] = ret;
        if number == 0x04 || number == 0x05 {
            let (discard, mask) = if number == 0x05 {
                self.gpr[0] = 1;
                self.gpr[1] = 1;
                (true, 1u16)
            } else {
                (self.gpr[0] != 0, self.gpr[1] as u16)
            };
            if discard {
                let flags = bus.irq.check_flags() & !mask;
                bus.irq.set_check_flags(flags);
            }
            let flags = bus.irq.check_flags();
            if flags & mask != 0 {
                bus.irq.set_check_flags(flags & !mask);
                self.finish_swi_return();
                self.last_op = "swi";
                return;
            }
            self.intr_wait_mask = Some(mask);
            bus.halted = true;
            self.last_op = "swi";
            return;
        }
        if number == 0x00 {
            let target = crate::bios::soft_reset(bus);
            self.write_cpsr(0x1F, 0xFFFF_FFFF);
            self.fetch_pc = target;
            self.exec_pc = target;
            self.last_op = "swi";
            return;
        }
        if number == 0x06 {
            let numerator = self.gpr[0] as i32;
            let denominator = self.gpr[1] as i32;
            if denominator != 0 {
                let quot = numerator.wrapping_div(denominator);
                let rem = numerator.wrapping_rem(denominator);
                self.gpr[0] = quot as u32;
                self.gpr[1] = rem as u32;
                self.gpr[3] = quot.unsigned_abs();
            }
        } else if number == 0x07 {
            let denominator = self.gpr[0] as i32;
            let numerator = self.gpr[1] as i32;
            if denominator != 0 {
                let quot = numerator.wrapping_div(denominator);
                let rem = numerator.wrapping_rem(denominator);
                self.gpr[0] = quot as u32;
                self.gpr[1] = rem as u32;
                self.gpr[3] = quot.unsigned_abs();
            }
        } else if number == 0x08 {
            self.gpr[0] = crate::bios::sqrt_u32(self.gpr[0]);
            bus.set_bios_prefetch(LATCH_AFTER_SQRT);
        } else if number == 0x09 {
            self.gpr[0] = crate::bios::arctan(self.gpr[0] as i32) as u32;
        } else if number == 0x0A {
            self.gpr[0] =
                crate::bios::arctan2(self.gpr[0] as i32, self.gpr[1] as i32) as u16 as u32;
        } else if number == 0x0B {
            crate::bios::cpu_set(bus, self.gpr[0], self.gpr[1], self.gpr[2]);
        } else if number == 0x0C {
            crate::bios::cpu_fast_set(bus, self.gpr[0], self.gpr[1], self.gpr[2]);
        } else if number == 0x0E {
            crate::bios::bg_affine_set(bus, self.gpr[0], self.gpr[1], self.gpr[2]);
        } else if number == 0x0F {
            crate::bios::obj_affine_set(bus, self.gpr[0], self.gpr[1], self.gpr[2], self.gpr[3]);
        } else if number == 0x10 {
            crate::bios::bit_unpack(bus, self.gpr[0], self.gpr[1], self.gpr[2]);
        } else if number == 0x11 {
            crate::bios::lz77_wram(bus, self.gpr[0], self.gpr[1]);
        } else if number == 0x12 {
            crate::bios::lz77_vram(bus, self.gpr[0], self.gpr[1]);
        } else if number == 0x13 {
            crate::bios::huff(bus, self.gpr[0], self.gpr[1]);
        } else if number == 0x14 {
            crate::bios::rl_wram(bus, self.gpr[0], self.gpr[1]);
        } else if number == 0x15 {
            crate::bios::rl_vram(bus, self.gpr[0], self.gpr[1]);
        } else if number == 0x16 {
            crate::bios::diff8_wram(bus, self.gpr[0], self.gpr[1]);
        } else if number == 0x17 {
            crate::bios::diff8_vram(bus, self.gpr[0], self.gpr[1]);
        } else if number == 0x18 {
            crate::bios::diff16(bus, self.gpr[0], self.gpr[1]);
        } else if number == 0x19 {
            crate::bios::sound_bias(bus, self.gpr[0] as u16);
        } else if number == 0x01 {
            crate::bios::register_ram_reset(bus, self.gpr[0]);
        } else if matches!(number, 0x1A..=0x1F | 0x27..=0x29) {
            crate::bios::warn_swi(bus, "music unimplemented", number);
        } else if number == 0x25 || number == 0x26 {
            crate::bios::warn_swi(bus, "unsupported", number);
        }
        self.finish_swi_return();
        self.last_op = "swi";
    }

    fn finish_swi_return(&mut self) {
        let back = self.spsr();
        let lr = self.gpr[14];
        self.write_cpsr(back, 0xFFFF_FFFF);
        self.fetch_pc = if self.thumb() { lr & !1 } else { lr & !3 };
    }

    fn fail(&mut self, mnemonic: String) -> Result<(), StepError> {
        self.faults = self.faults.wrapping_add(1);
        self.last_op = "undef";
        Err(StepError::Unimplemented {
            pc: self.exec_pc,
            mnemonic,
        })
    }
}

fn bank_index(mode: u32) -> usize {
    match mode & 0x1F {
        0x11 => 1,
        0x12 => 2,
        0x13 => 3,
        0x17 => 4,
        0x1B => 5,
        _ => 0,
    }
}

/// BIOS IRQ return stub address (`mov pc, lr` from the user ISR lands here).
const IRQ_RETURN_STUB: u32 = 0x138;
/// Prefetch latch after `swi 0x08` (Sqrt): word at BIOS 0x188.
const LATCH_AFTER_SQRT: u32 = 0xE3A0_2004;
/// Prefetch latch while the user IRQ handler runs: word at BIOS 0x13C.
const LATCH_DURING_IRQ: u32 = 0xE25E_F004;
/// Prefetch latch after the IRQ stub returns to the game: word at BIOS 0x144.
const LATCH_AFTER_IRQ: u32 = 0xE55E_C002;

#[cfg(test)]
mod tests;
