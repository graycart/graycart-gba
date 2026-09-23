//! ARM7TDMI.
//!
//! Cited: GBATEK, ARM CPU Reference. https://problemkaputt.de/gbatek.htm
//! Cited: ARM DDI 0210C, ARM7TDMI Technical Reference Manual.

mod exec;
mod shift;

use crate::bus::Bus;

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug)]
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
    pub last_op: &'static str,
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

    pub fn cpsr(&self) -> u32 {
        self.cpsr
    }

    pub fn thumb(&self) -> bool {
        self.cpsr & 0x20 != 0
    }

    pub fn step(&mut self, bus: &mut Bus) -> Result<(), StepError> {
        if self.thumb() {
            let instr = bus.fetch16(self.fetch_pc);
            self.exec_pc = self.fetch_pc;
            self.fetch_pc = self.fetch_pc.wrapping_add(2);
            self.exec_thumb(bus, instr)
        } else {
            let instr = bus.fetch32(self.fetch_pc);
            self.exec_pc = self.fetch_pc;
            self.fetch_pc = self.fetch_pc.wrapping_add(4);
            self.exec_arm(bus, instr)
        }
    }

    /// IRQ entry. Nothing in the frame loop raises this yet.
    pub fn raise_irq(&mut self) {
        if self.cpsr & 0x80 != 0 {
            return;
        }
        let spsr = self.cpsr;
        let ret = self.fetch_pc.wrapping_add(4);
        self.write_cpsr(0x92, 0xFFFF_FFFF);
        self.set_spsr(spsr, 0xFFFF_FFFF);
        self.gpr[14] = ret;
        self.fetch_pc = 0x18;
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
        if n == 15 {
            pc
        } else {
            self.gpr[n]
        }
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
        if thumb {
            self.cpsr |= 0x20;
            self.fetch_pc = value & !1;
        } else {
            self.cpsr &= !0x20;
            self.fetch_pc = value & !3;
        }
    }

    fn write_cpsr(&mut self, value: u32, mask: u32) {
        let old_mode = self.cpsr & 0x1F;
        let mask = if old_mode == 0x10 {
            mask & 0xF000_0000
        } else {
            mask
        };
        let merged = (self.cpsr & !mask) | (value & mask);
        let new_mode = merged & 0x1F;
        if bank_index(old_mode) != bank_index(new_mode) || (old_mode == 0x11) != (new_mode == 0x11)
        {
            self.save_bank(old_mode);
            self.load_bank(new_mode);
        }
        self.cpsr = merged;
    }

    fn spsr(&self) -> u32 {
        self.spsr_bank[bank_index(self.cpsr & 0x1F)]
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
        }
        let back = self.spsr();
        let lr = self.gpr[14];
        self.write_cpsr(back, 0xFFFF_FFFF);
        self.fetch_pc = if self.thumb() { lr & !1 } else { lr & !3 };
        self.last_op = "swi";
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

#[cfg(test)]
mod tests;
