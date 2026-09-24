//! ARM and Thumb decode/execute.
//!
//! Cited: ARM DDI 0210C. Cited: GBATEK ARM CPU Reference / Thumb.

use super::shift::{add_carry, barrel, rrx};
use super::{Cpu, StepError};
use crate::bus::Bus;

impl Cpu {
    pub(super) fn exec_arm(&mut self, bus: &mut Bus, instr: u32) -> Result<(), StepError> {
        let cond = instr >> 28;
        if !condition(self.cpsr, cond) {
            self.last_op = "cond";
            return Ok(());
        }
        if instr & 0x0FFF_FFF0 == 0x012F_FF10 {
            return self.bx(bus, instr);
        }
        if instr & 0x0F00_00F0 == 0x0000_0090 {
            return self.multiply(instr);
        }
        if instr & 0x0FB0_0FF0 == 0x0100_0090 {
            return self.swap(bus, instr);
        }
        if instr & 0x0E00_0090 == 0x0000_0090 && instr & 0x60 != 0 {
            return self.half(bus, instr);
        }
        if instr & 0x0C00_0000 == 0x0000_0000 {
            return self.data(instr);
        }
        if instr & 0x0C00_0000 == 0x0400_0000 {
            return self.single(bus, instr);
        }
        if instr & 0x0E00_0000 == 0x0800_0000 {
            return self.block(bus, instr);
        }
        if instr & 0x0E00_0000 == 0x0A00_0000 {
            return self.arm_b(bus, instr);
        }
        if instr & 0x0F00_0000 == 0x0F00_0000 {
            self.last_op = "swi";
            self.swi(bus, instr & 0x00FF_FFFF);
            return Ok(());
        }
        self.fail(format!("arm {instr:#010X}"))
    }

    pub(super) fn exec_thumb(&mut self, bus: &mut Bus, instr: u16) -> Result<(), StepError> {
        let pc = self.exec_pc.wrapping_add(4);

        // Format 1: move shifted register. Format 2: add/subtract.
        if instr & 0xE000 == 0 {
            if instr & 0x1800 != 0x1800 {
                let op = (instr >> 11) & 3;
                let imm5 = ((instr >> 6) & 0x1F) as u32;
                let rs = ((instr >> 3) & 7) as usize;
                let rd = (instr & 7) as usize;
                let value = self.gpr[rs];
                let amount = if op != 0 && imm5 == 0 { 32 } else { imm5 };
                let (result, carry) = barrel(value, op as u32, amount, self.carry());
                self.gpr[rd] = result;
                self.set_nz(result);
                self.set_carry(carry);
                self.last_op = "shift";
                return Ok(());
            }
            let imm = instr & (1 << 10) != 0;
            let sub = instr & (1 << 9) != 0;
            let rn_imm = ((instr >> 6) & 7) as u32;
            let rs = ((instr >> 3) & 7) as usize;
            let rd = (instr & 7) as usize;
            let left = self.gpr[rs];
            let right = if imm {
                rn_imm
            } else {
                self.gpr[rn_imm as usize]
            };
            let (result, c, v) = if sub {
                add_carry(left, !right, true)
            } else {
                add_carry(left, right, false)
            };
            self.gpr[rd] = result;
            self.set_nz(result);
            self.set_cv(c, v);
            self.last_op = if sub { "sub" } else { "add" };
            return Ok(());
        }

        // Format 3: MOV/CMP/ADD/SUB Rd, #imm8.
        if instr & 0xE000 == 0x2000 {
            let op = (instr >> 11) & 3;
            let rd = ((instr >> 8) & 7) as usize;
            let imm = (instr & 0xFF) as u32;
            let left = self.gpr[rd];
            match op {
                0 => {
                    self.gpr[rd] = imm;
                    self.set_nz(imm);
                    self.last_op = "mov";
                }
                1 => {
                    let (diff, c, v) = add_carry(left, !imm, true);
                    self.set_nz(diff);
                    self.set_cv(c, v);
                    self.last_op = "cmp";
                }
                2 => {
                    let (sum, c, v) = add_carry(left, imm, false);
                    self.gpr[rd] = sum;
                    self.set_nz(sum);
                    self.set_cv(c, v);
                    self.last_op = "add";
                }
                _ => {
                    let (diff, c, v) = add_carry(left, !imm, true);
                    self.gpr[rd] = diff;
                    self.set_nz(diff);
                    self.set_cv(c, v);
                    self.last_op = "sub";
                }
            }
            return Ok(());
        }

        // Format 4: ALU operations.
        if instr & 0xFC00 == 0x4000 {
            let op = (instr >> 6) & 0xF;
            let rs = ((instr >> 3) & 7) as usize;
            let rd = (instr & 7) as usize;
            let left = self.gpr[rd];
            let right = self.gpr[rs];
            let cin = self.carry();
            let (result, write, set_c_from_shift, arith, carry, overflow) = match op {
                0x0 => (left & right, true, false, false, cin, false),
                0x1 => (left ^ right, true, false, false, cin, false),
                0x2 => {
                    let (r, c) = barrel(left, 0, right & 0xFF, cin);
                    (r, true, true, false, c, false)
                }
                0x3 => {
                    let (r, c) = barrel(left, 1, right & 0xFF, cin);
                    (r, true, true, false, c, false)
                }
                0x4 => {
                    let (r, c) = barrel(left, 2, right & 0xFF, cin);
                    (r, true, true, false, c, false)
                }
                0x5 => {
                    let (r, c, v) = add_carry(left, right, cin);
                    (r, true, false, true, c, v)
                }
                0x6 => {
                    let (r, c, v) = add_carry(left, !right, cin);
                    (r, true, false, true, c, v)
                }
                0x7 => {
                    let (r, c) = barrel(left, 3, right & 0xFF, cin);
                    (r, true, true, false, c, false)
                }
                0x8 => (left & right, false, false, false, cin, false),
                0x9 => {
                    let (r, c, v) = add_carry(0, !right, true);
                    (r, true, false, true, c, v)
                }
                0xA => {
                    let (r, c, v) = add_carry(left, !right, true);
                    (r, false, false, true, c, v)
                }
                0xB => {
                    let (r, c, v) = add_carry(left, right, false);
                    (r, false, false, true, c, v)
                }
                0xC => (left | right, true, false, false, cin, false),
                0xD => (left.wrapping_mul(right), true, false, false, cin, false),
                0xE => (left & !right, true, false, false, cin, false),
                _ => (!right, true, false, false, cin, false),
            };
            self.set_nz(result);
            if arith {
                self.set_cv(carry, overflow);
            } else if set_c_from_shift {
                self.set_carry(carry);
            }
            if write {
                self.gpr[rd] = result;
            }
            self.last_op = if op == 0xD { "mul" } else { "alu" };
            return Ok(());
        }

        // Format 5: hi-register operations / BX.
        if instr & 0xFC00 == 0x4400 {
            let op = (instr >> 8) & 3;
            let src = (((instr >> 3) & 7) as usize) | ((((instr >> 6) & 1) as usize) << 3);
            let dst = ((instr & 7) as usize) | ((((instr >> 7) & 1) as usize) << 3);
            let s_val = self.read_gpr(src, pc);
            let d_val = self.read_gpr(dst, pc);
            self.last_op = "thumb-hi";
            match op {
                0 => {
                    let (sum, _, _) = add_carry(d_val, s_val, false);
                    if dst == 15 {
                        self.branch(sum, true);
                        bus.thumb_branch_refill(self.fetch_pc);
                    } else {
                        self.gpr[dst] = sum;
                    }
                }
                1 => {
                    let (diff, c, v) = add_carry(d_val, !s_val, true);
                    self.set_nz(diff);
                    self.set_cv(c, v);
                }
                2 => {
                    if dst == 15 {
                        // MOV to PC stays in Thumb; bit 0 is discarded (DDI 0210C).
                        self.branch(s_val, true);
                        bus.thumb_branch_refill(self.fetch_pc);
                    } else {
                        self.gpr[dst] = s_val;
                    }
                }
                _ => {
                    self.branch(s_val, s_val & 1 != 0);
                    if self.cpsr & 0x20 != 0 {
                        bus.thumb_branch_refill(self.fetch_pc);
                    } else {
                        bus.arm_branch_refill(self.fetch_pc);
                    }
                    self.last_op = "bx";
                }
            }
            return Ok(());
        }

        // Format 6: PC-relative load.
        if instr & 0xF800 == 0x4800 {
            let rd = ((instr >> 8) & 7) as usize;
            let imm = (instr & 0xFF) as u32;
            let addr = (pc & !2).wrapping_add(imm << 2);
            let value = bus.read32(addr).rotate_right((addr & 3) * 8);
            // Thumb LDR: 1S+1N+1I (GBATEK).
            bus.add_internal_cycles(1);
            self.gpr[rd] = value;
            self.last_op = "ldr";
            return Ok(());
        }

        // Format 7/8: register-offset load/store.
        if instr & 0xF000 == 0x5000 {
            let ro = ((instr >> 6) & 7) as usize;
            let rb = ((instr >> 3) & 7) as usize;
            let rd = (instr & 7) as usize;
            let addr = self.gpr[rb].wrapping_add(self.gpr[ro]);
            if instr & (1 << 9) != 0 {
                let h = instr & (1 << 11) != 0;
                let s = instr & (1 << 10) != 0;
                match (h, s) {
                    (false, false) => {
                        bus.write16(addr, self.gpr[rd] as u16);
                        // Thumb STRH: 2N (GBATEK).
                        bus.add_internal_cycles(2);
                        self.last_op = "strh";
                    }
                    (false, true) => {
                        self.gpr[rd] = bus.read8(addr) as i8 as i32 as u32;
                        bus.add_internal_cycles(1);
                        self.last_op = "ldrsb";
                    }
                    (true, false) => {
                        let raw = bus.read16(addr) as u32;
                        self.gpr[rd] = if addr & 1 != 0 {
                            raw.rotate_right(8)
                        } else {
                            raw
                        };
                        bus.add_internal_cycles(1);
                        self.last_op = "ldrh";
                    }
                    (true, true) => {
                        self.gpr[rd] = if addr & 1 != 0 {
                            bus.read8(addr) as i8 as i32 as u32
                        } else {
                            bus.read16(addr) as i16 as i32 as u32
                        };
                        bus.add_internal_cycles(1);
                        self.last_op = "ldrsh";
                    }
                }
            } else {
                let load = instr & (1 << 11) != 0;
                let byte = instr & (1 << 10) != 0;
                if load {
                    self.gpr[rd] = if byte {
                        bus.read8(addr) as u32
                    } else {
                        bus.read32(addr).rotate_right((addr & 3) * 8)
                    };
                    bus.add_internal_cycles(1);
                    self.last_op = if byte { "ldrb" } else { "ldr" };
                } else if byte {
                    bus.write8(addr, self.gpr[rd] as u8);
                    // Thumb STRB: 2N (GBATEK).
                    bus.add_internal_cycles(2);
                    self.last_op = "strb";
                } else {
                    bus.write32(addr, self.gpr[rd]);
                    // Thumb STR: 2N (GBATEK).
                    bus.add_internal_cycles(2);
                    self.last_op = "str";
                }
            }
            return Ok(());
        }

        // Format 9: immediate-offset word/byte load/store.
        if instr & 0xE000 == 0x6000 {
            let byte = instr & (1 << 12) != 0;
            let load = instr & (1 << 11) != 0;
            let imm = ((instr >> 6) & 0x1F) as u32;
            let rb = ((instr >> 3) & 7) as usize;
            let rd = (instr & 7) as usize;
            let addr = if byte {
                self.gpr[rb].wrapping_add(imm)
            } else {
                self.gpr[rb].wrapping_add(imm << 2)
            };
            if load {
                self.gpr[rd] = if byte {
                    bus.read8(addr) as u32
                } else {
                    bus.read32(addr).rotate_right((addr & 3) * 8)
                };
                bus.add_internal_cycles(1);
                self.last_op = if byte { "ldrb" } else { "ldr" };
            } else if byte {
                bus.write8(addr, self.gpr[rd] as u8);
                // Thumb STRB: 2N (GBATEK) — data beat plus trailing bus slot.
                bus.add_internal_cycles(2);
                self.last_op = "strb";
            } else {
                bus.write32(addr, self.gpr[rd]);
                // Thumb STR: 2N (GBATEK) — data beat plus trailing bus slot.
                bus.add_internal_cycles(2);
                self.last_op = "str";
            }
            return Ok(());
        }

        // Format 10: immediate halfword load/store.
        if instr & 0xF000 == 0x8000 {
            let load = instr & (1 << 11) != 0;
            let imm = ((instr >> 6) & 0x1F) as u32;
            let rb = ((instr >> 3) & 7) as usize;
            let rd = (instr & 7) as usize;
            let addr = self.gpr[rb].wrapping_add(imm << 1);
            if load {
                let raw = bus.read16(addr) as u32;
                self.gpr[rd] = if addr & 1 != 0 {
                    raw.rotate_right(8)
                } else {
                    raw
                };
                bus.add_internal_cycles(1);
                self.last_op = "ldrh";
            } else {
                bus.write16(addr, self.gpr[rd] as u16);
                // Thumb STRH: 2N (GBATEK).
                bus.add_internal_cycles(2);
                self.last_op = "strh";
            }
            return Ok(());
        }

        // Format 11: SP-relative load/store.
        if instr & 0xF000 == 0x9000 {
            let load = instr & (1 << 11) != 0;
            let rd = ((instr >> 8) & 7) as usize;
            let imm = (instr & 0xFF) as u32;
            let addr = self.gpr[13].wrapping_add(imm << 2);
            if load {
                self.gpr[rd] = bus.read32(addr).rotate_right((addr & 3) * 8);
                bus.add_internal_cycles(1);
                self.last_op = "ldr";
            } else {
                bus.write32(addr, self.gpr[rd]);
                // Thumb STR (SP-relative): 2N (GBATEK).
                bus.add_internal_cycles(2);
                self.last_op = "str";
            }
            return Ok(());
        }

        // Format 12: load address (ADD Rd, PC/SP, #imm).
        if instr & 0xF000 == 0xA000 {
            let rd = ((instr >> 8) & 7) as usize;
            let imm = (instr & 0xFF) as u32;
            let base = if instr & (1 << 11) != 0 {
                self.gpr[13]
            } else {
                pc & !2
            };
            self.gpr[rd] = base.wrapping_add(imm << 2);
            self.last_op = if instr & (1 << 11) != 0 { "add" } else { "adr" };
            return Ok(());
        }

        // Format 13: add/subtract offset to SP.
        if instr & 0xFF00 == 0xB000 {
            let imm = (instr & 0x7F) as u32;
            if instr & (1 << 7) != 0 {
                self.gpr[13] = self.gpr[13].wrapping_sub(imm << 2);
            } else {
                self.gpr[13] = self.gpr[13].wrapping_add(imm << 2);
            }
            self.last_op = "add";
            return Ok(());
        }

        // Format 14: PUSH / POP.
        if instr & 0xF600 == 0xB400 {
            let load = instr & (1 << 11) != 0;
            let mut list = u32::from(instr & 0xFF);
            if instr & (1 << 8) != 0 {
                list |= if load { 1 << 15 } else { 1 << 14 };
            }
            let count = list.count_ones();
            if load {
                let mut addr = self.gpr[13];
                for reg in 0..16 {
                    if list & (1 << reg) == 0 {
                        continue;
                    }
                    let value = bus.read32(addr);
                    if reg == 15 {
                        // ARMv4T Thumb POP: stay in Thumb; bit 0 is ignored for state.
                        self.branch(value, true);
                    } else {
                        self.gpr[reg] = value;
                    }
                    addr = addr.wrapping_add(4);
                }
                self.gpr[13] = self.gpr[13].wrapping_add(count.wrapping_mul(4));
                self.last_op = "pop";
            } else {
                let mut addr = self.gpr[13].wrapping_sub(count.wrapping_mul(4));
                self.gpr[13] = addr;
                for reg in 0..16 {
                    if list & (1 << reg) == 0 {
                        continue;
                    }
                    bus.write32(addr, self.gpr[reg]);
                    addr = addr.wrapping_add(4);
                }
                self.last_op = "push";
            }
            return Ok(());
        }

        // Format 15: multiple load/store.
        if instr & 0xF000 == 0xC000 {
            let load = instr & (1 << 11) != 0;
            let rb = ((instr >> 8) & 7) as usize;
            let mut list = u32::from(instr & 0xFF);
            let mut count = list.count_ones();
            // Empty rlist: transfer R15 and writeback as 16 words (GBATEK / jsmolka t227–t229).
            if count == 0 {
                list = 1 << 15;
                count = 16;
            }
            let base = self.gpr[rb];
            let new_base = base.wrapping_add(count.wrapping_mul(4));
            let first = list.trailing_zeros() as usize;
            self.gpr[rb] = new_base;
            let mut addr = base;
            for reg in 0..16 {
                if list & (1 << reg) == 0 {
                    continue;
                }
                if load {
                    let value = bus.read32(addr);
                    if reg == 15 {
                        self.branch(value, true);
                    } else {
                        self.gpr[reg] = value;
                    }
                } else {
                    let value = if reg == 15 {
                        // Match following `mov r1, pc` (PC+4 of that insn = STM exec_pc + 6).
                        self.exec_pc.wrapping_add(6)
                    } else if reg == rb {
                        if reg == first { base } else { new_base }
                    } else {
                        self.gpr[reg]
                    };
                    bus.write32(addr, value);
                }
                addr = addr.wrapping_add(4);
            }
            self.last_op = if load { "ldm" } else { "stm" };
            return Ok(());
        }

        // Format 16/17: conditional branch / SWI.
        if instr & 0xF000 == 0xD000 {
            let cond = (instr >> 8) & 0xF;
            if cond == 0xF {
                self.last_op = "swi";
                self.swi(bus, u32::from(instr & 0xFF) << 16);
                return Ok(());
            }
            // Condition 0xE (AL) is undefined in Thumb on ARM7; 0xF is SWI.
            if cond == 0xE {
                return self.fail(format!("thumb b {instr:#06X}"));
            }
            self.last_op = "b";
            if condition(self.cpsr, u32::from(cond)) {
                let offset = ((instr & 0xFF) as i8 as i32) << 1;
                self.fetch_pc = pc.wrapping_add(offset as u32);
                bus.thumb_branch_refill(self.fetch_pc);
            }
            return Ok(());
        }

        // Format 18: unconditional branch.
        if instr & 0xF800 == 0xE000 {
            let offset = ((((instr & 0x7FF) as i32) << 21) >> 21) << 1;
            let target = pc.wrapping_add(offset as u32);
            if target == self.exec_pc {
                self.idle = true;
            }
            self.fetch_pc = target;
            bus.thumb_branch_refill(self.fetch_pc);
            self.last_op = "b";
            return Ok(());
        }

        // Format 19: BL / BLX. 11101 is BLX (not ARM7).
        if instr & 0xF800 == 0xF000 {
            let offset = ((((instr & 0x7FF) as i32) << 21) >> 21) << 12;
            self.gpr[14] = pc.wrapping_add(offset as u32);
            self.last_op = "bl";
            return Ok(());
        }
        if instr & 0xF800 == 0xF800 {
            let offset = u32::from(instr & 0x7FF) << 1;
            let temp = self.gpr[14].wrapping_add(offset);
            self.gpr[14] = self.exec_pc.wrapping_add(2) | 1;
            // ARM7 BL stays in Thumb; bit 0 of temp is always clear.
            self.branch(temp, true);
            bus.thumb_branch_refill(self.fetch_pc);
            self.last_op = "bl";
            return Ok(());
        }
        if instr & 0xF800 == 0xE800 {
            return self.fail(format!("thumb blx {instr:#06X}"));
        }

        self.fail(format!("thumb {instr:#06X}"))
    }

    fn bx(&mut self, bus: &mut Bus, instr: u32) -> Result<(), StepError> {
        let rm = (instr & 0xF) as usize;
        let value = self.read_gpr(rm, self.exec_pc.wrapping_add(8));
        let thumb = value & 1 != 0;
        self.branch(value, thumb);
        if thumb {
            bus.thumb_branch_refill(self.fetch_pc);
        } else {
            bus.arm_branch_refill(self.fetch_pc);
        }
        self.last_op = "bx";
        Ok(())
    }

    fn arm_b(&mut self, bus: &mut Bus, instr: u32) -> Result<(), StepError> {
        let offset = ((instr & 0x00FF_FFFF) << 8) as i32 >> 6;
        let target = self.exec_pc.wrapping_add(8).wrapping_add(offset as u32);
        if instr & (1 << 24) != 0 {
            self.gpr[14] = self.exec_pc.wrapping_add(4);
            self.last_op = "bl";
        } else {
            self.last_op = "b";
            if target == self.exec_pc {
                self.idle = true;
            }
        }
        self.fetch_pc = target;
        if !self.idle {
            bus.arm_branch_refill(target);
        }
        Ok(())
    }

    fn data(&mut self, instr: u32) -> Result<(), StepError> {
        if instr & 0x0FBF_0FFF == 0x010F_0000 {
            let rd = ((instr >> 12) & 0xF) as usize;
            let spsr = instr & (1 << 22) != 0;
            let value = if spsr { self.spsr() } else { self.cpsr };
            self.write_gpr(rd, value);
            self.last_op = "mrs";
            return Ok(());
        }
        if instr & 0x0FB0_FFF0 == 0x0120_F000 || instr & 0x0FB0_F000 == 0x0320_F000 {
            return self.msr(instr);
        }
        let opcode = (instr >> 21) & 0xF;
        let set = instr & (1 << 20) != 0;
        let rn = ((instr >> 16) & 0xF) as usize;
        let rd = ((instr >> 12) & 0xF) as usize;
        let reg_shift = instr & 0x0200_0010 == 0x10;
        let pc = self.exec_pc.wrapping_add(if reg_shift { 12 } else { 8 });
        let (op2, sh_carry) = self.operand2(instr, pc);
        let left = self.read_gpr(rn, pc);
        let cin = self.carry();
        let (result, carry, overflow, logical) = match opcode {
            0x0 | 0x8 => (left & op2, sh_carry, false, true),
            0x1 | 0x9 => (left ^ op2, sh_carry, false, true),
            0x2 | 0xA => {
                let (r, c, v) = add_carry(left, !op2, true);
                (r, c, v, false)
            }
            0x3 => {
                let (r, c, v) = add_carry(op2, !left, true);
                (r, c, v, false)
            }
            0x4 | 0xB => {
                let (r, c, v) = add_carry(left, op2, false);
                (r, c, v, false)
            }
            0x5 => {
                let (r, c, v) = add_carry(left, op2, cin);
                (r, c, v, false)
            }
            0x6 => {
                let (r, c, v) = add_carry(left, !op2, cin);
                (r, c, v, false)
            }
            0x7 => {
                let (r, c, v) = add_carry(op2, !left, cin);
                (r, c, v, false)
            }
            0xC => (left | op2, sh_carry, false, true),
            0xD => (op2, sh_carry, false, true),
            0xE => (left & !op2, sh_carry, false, true),
            0xF => (!op2, sh_carry, false, true),
            _ => (0, false, false, true),
        };
        let write = !matches!(opcode, 0x8..=0xB);
        if set && rd == 15 {
            let spsr = self.spsr();
            self.write_cpsr(spsr, 0xFFFF_FFFF);
        } else if set {
            if logical {
                self.set_nz(result);
                self.set_carry(carry);
            } else {
                self.set_nz(result);
                self.set_cv(carry, overflow);
            }
        }
        if write && rd == 15 {
            self.branch(result, self.thumb());
        } else if write {
            self.gpr[rd] = result;
        }
        self.last_op = "alu";
        Ok(())
    }

    fn msr(&mut self, instr: u32) -> Result<(), StepError> {
        let mut mask = 0u32;
        if instr & (1 << 16) != 0 {
            mask |= 0x0000_00FF;
        }
        if instr & (1 << 17) != 0 {
            mask |= 0x0000_FF00;
        }
        if instr & (1 << 18) != 0 {
            mask |= 0x00FF_0000;
        }
        if instr & (1 << 19) != 0 {
            mask |= 0xFF00_0000;
        }
        let value = if instr & (1 << 25) != 0 {
            let imm = instr & 0xFF;
            let rot = ((instr >> 8) & 0xF) * 2;
            imm.rotate_right(rot)
        } else {
            let rm = (instr & 0xF) as usize;
            self.read_gpr(rm, self.exec_pc.wrapping_add(8))
        };
        if instr & (1 << 22) != 0 {
            self.set_spsr(value, mask);
        } else {
            self.write_cpsr(value, mask);
        }
        self.last_op = "msr";
        Ok(())
    }

    fn operand2(&mut self, instr: u32, pc: u32) -> (u32, bool) {
        let carry = self.carry();
        if instr & (1 << 25) != 0 {
            let imm = instr & 0xFF;
            let rot = ((instr >> 8) & 0xF) * 2;
            if rot == 0 {
                (imm, carry)
            } else {
                let value = imm.rotate_right(rot);
                (value, value & 0x8000_0000 != 0)
            }
        } else {
            let rm = (instr & 0xF) as usize;
            let value = self.read_gpr(rm, pc);
            let kind = (instr >> 5) & 3;
            let by_reg = instr & (1 << 4) != 0;
            if by_reg {
                let rs = ((instr >> 8) & 0xF) as usize;
                let amount = self.read_gpr(rs, pc) & 0xFF;
                barrel(value, kind, amount, carry)
            } else {
                let amount = (instr >> 7) & 0x1F;
                if kind == 3 && amount == 0 {
                    rrx(value, carry)
                } else if amount == 0 && (kind == 1 || kind == 2) {
                    barrel(value, kind, 32, carry)
                } else {
                    barrel(value, kind, amount, carry)
                }
            }
        }
    }

    fn multiply(&mut self, instr: u32) -> Result<(), StepError> {
        let set = instr & (1 << 20) != 0;
        let rd = ((instr >> 16) & 0xF) as usize;
        let rn = ((instr >> 12) & 0xF) as usize;
        let rs = ((instr >> 8) & 0xF) as usize;
        let rm = (instr & 0xF) as usize;
        let pc = self.exec_pc.wrapping_add(8);
        let a = self.read_gpr(rm, pc) as u64;
        let b = self.read_gpr(rs, pc) as u64;
        let long = instr & (1 << 23) != 0;
        if long {
            let signed = instr & (1 << 22) != 0;
            let acc = instr & (1 << 21) != 0;
            let product = if signed {
                (self.read_gpr(rm, pc) as i32 as i64)
                    .wrapping_mul(self.read_gpr(rs, pc) as i32 as i64) as u64
            } else {
                a.wrapping_mul(b)
            };
            let product = if acc {
                let hi = self.read_gpr(rd, pc) as u64;
                let lo = self.read_gpr(rn, pc) as u64;
                product.wrapping_add(lo | (hi << 32))
            } else {
                product
            };
            let lo = product as u32;
            let hi = (product >> 32) as u32;
            self.gpr[rn] = lo;
            self.gpr[rd] = hi;
            if set {
                let nz = if hi == 0 && lo == 0 {
                    0
                } else {
                    (hi & 0x8000_0000) | 1
                };
                self.set_nz(nz);
            }
        } else {
            let acc = instr & (1 << 21) != 0;
            let mut product = (a as u32).wrapping_mul(b as u32);
            if acc {
                product = product.wrapping_add(self.read_gpr(rn, pc));
            }
            self.gpr[rd] = product;
            if set {
                self.set_nz(product);
            }
        }
        self.last_op = "mul";
        Ok(())
    }

    fn swap(&mut self, bus: &mut Bus, instr: u32) -> Result<(), StepError> {
        let byte = instr & (1 << 22) != 0;
        let rn = ((instr >> 16) & 0xF) as usize;
        let rd = ((instr >> 12) & 0xF) as usize;
        let rm = (instr & 0xF) as usize;
        let pc = self.exec_pc.wrapping_add(8);
        let addr = self.read_gpr(rn, pc);
        let store = self.read_gpr(rm, pc);
        if byte {
            let loaded = bus.read8(addr) as u32;
            bus.write8(addr, store as u8);
            self.write_gpr(rd, loaded);
        } else {
            let loaded = bus.read32(addr).rotate_right((addr & 3) * 8);
            bus.write32(addr, store);
            self.write_gpr(rd, loaded);
        }
        self.last_op = "swp";
        Ok(())
    }

    fn single(&mut self, bus: &mut Bus, instr: u32) -> Result<(), StepError> {
        let reg_off = instr & (1 << 25) != 0;
        let pre = instr & (1 << 24) != 0;
        let up = instr & (1 << 23) != 0;
        let byte = instr & (1 << 22) != 0;
        let writeback = instr & (1 << 21) != 0 || !pre;
        let load = instr & (1 << 20) != 0;
        let rn = ((instr >> 16) & 0xF) as usize;
        let rd = ((instr >> 12) & 0xF) as usize;
        let pc = self.exec_pc.wrapping_add(8);
        let base = self.read_gpr(rn, pc);
        let (offset, sh_carry) = if reg_off {
            let rm = (instr & 0xF) as usize;
            let value = self.read_gpr(rm, pc);
            let kind = (instr >> 5) & 3;
            let amount = (instr >> 7) & 0x1F;
            if kind == 3 && amount == 0 {
                rrx(value, self.carry())
            } else if amount == 0 && (kind == 1 || kind == 2) {
                barrel(value, kind, 32, self.carry())
            } else {
                barrel(value, kind, amount, self.carry())
            }
        } else {
            (instr & 0xFFF, self.carry())
        };
        if reg_off {
            self.set_carry(sh_carry);
        }
        let indexed = if up {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };
        let access = if pre { indexed } else { base };
        let stored = if rd == 15 {
            self.exec_pc.wrapping_add(12)
        } else {
            self.gpr[rd]
        };
        if load {
            let value = if byte {
                bus.read8(access) as u32
            } else {
                bus.read32(access).rotate_right((access & 3) * 8)
            };
            // ARM LDR: 1S+1N+1I — data access paid N; trailing internal cycle.
            bus.add_internal_cycles(1);
            if writeback {
                self.gpr[rn] = indexed;
            }
            if rd == 15 {
                self.branch(value, false);
            } else {
                self.gpr[rd] = value;
            }
        } else {
            if byte {
                bus.write8(access, stored as u8);
            } else {
                bus.write32(access, stored);
            }
            // ARM STR: 2N (GBATEK) — data access paid; one more bus slot.
            bus.add_internal_cycles(1);
            if writeback {
                self.gpr[rn] = indexed;
            }
        }
        self.last_op = if load { "ldr" } else { "str" };
        Ok(())
    }

    fn half(&mut self, bus: &mut Bus, instr: u32) -> Result<(), StepError> {
        let pre = instr & (1 << 24) != 0;
        let up = instr & (1 << 23) != 0;
        let imm = instr & (1 << 22) != 0;
        let writeback = instr & (1 << 21) != 0 || !pre;
        let load = instr & (1 << 20) != 0;
        let rn = ((instr >> 16) & 0xF) as usize;
        let rd = ((instr >> 12) & 0xF) as usize;
        let sh = (instr >> 5) & 3;
        let pc = self.exec_pc.wrapping_add(8);
        let base = self.read_gpr(rn, pc);
        let offset = if imm {
            ((instr >> 4) & 0xF0) | (instr & 0xF)
        } else {
            let rm = (instr & 0xF) as usize;
            self.read_gpr(rm, pc)
        };
        let indexed = if up {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };
        let access = if pre { indexed } else { base };
        if load {
            let value = match sh {
                1 => {
                    let raw = bus.read16(access & !1) as u32;
                    if access & 1 != 0 {
                        raw.rotate_right(8)
                    } else {
                        raw
                    }
                }
                2 => bus.read8(access) as i8 as i32 as u32,
                3 => {
                    if access & 1 != 0 {
                        bus.read8(access) as i8 as i32 as u32
                    } else {
                        bus.read16(access) as i16 as i32 as u32
                    }
                }
                _ => 0,
            };
            if writeback {
                self.gpr[rn] = indexed;
            }
            if rd == 15 {
                self.branch(value, false);
            } else {
                self.gpr[rd] = value;
            }
        } else {
            let stored = if rd == 15 {
                self.exec_pc.wrapping_add(12)
            } else {
                self.gpr[rd]
            };
            bus.write16(access, stored as u16);
            if writeback {
                self.gpr[rn] = indexed;
            }
        }
        self.last_op = "ldrh";
        Ok(())
    }

    fn block(&mut self, bus: &mut Bus, instr: u32) -> Result<(), StepError> {
        let pre = instr & (1 << 24) != 0;
        let up = instr & (1 << 23) != 0;
        let set = instr & (1 << 22) != 0;
        let writeback = instr & (1 << 21) != 0;
        let load = instr & (1 << 20) != 0;
        let rn = ((instr >> 16) & 0xF) as usize;
        let mut list = instr & 0xFFFF;
        let mut count = list.count_ones();
        if count == 0 {
            list = 1 << 15;
            count = 16;
        }
        let pc = self.exec_pc.wrapping_add(8);
        let base = self.read_gpr(rn, pc);
        let user = set && !(load && list & (1 << 15) != 0);
        let bytes = count.wrapping_mul(4);
        let start = match (pre, up) {
            (false, true) => base,
            (true, true) => base.wrapping_add(4),
            (false, false) => base.wrapping_sub(bytes.wrapping_sub(4)),
            (true, false) => base.wrapping_sub(bytes),
        };
        let new_base = if up {
            base.wrapping_add(bytes)
        } else {
            base.wrapping_sub(bytes)
        };
        let first = list.trailing_zeros();
        if writeback {
            self.gpr[rn] = new_base;
        }
        let mut addr = start;
        for reg in 0..16 {
            if list & (1 << reg) == 0 {
                continue;
            }
            if load {
                let value = bus.read32(addr);
                if reg == 15 {
                    if set {
                        let spsr = self.spsr();
                        self.write_cpsr(spsr, 0xFFFF_FFFF);
                    }
                    self.branch(value, false);
                } else if user {
                    self.write_user(reg, value);
                } else {
                    self.gpr[reg] = value;
                }
            } else {
                let value = if reg == 15 {
                    self.exec_pc.wrapping_add(12)
                } else if user {
                    self.read_user(reg)
                } else if writeback && reg == rn {
                    if reg == first as usize {
                        base
                    } else {
                        new_base
                    }
                } else {
                    self.gpr[reg]
                };
                bus.write32(addr, value);
            }
            addr = addr.wrapping_add(4);
        }
        // GBATEK: LDM is nS+1N+1I (trailing I). STM is (n-1)S+2N (no extra I here;
        // first/last N are from bus sequential breaks on the write bursts).
        if load {
            bus.add_internal_cycles(1);
        }
        self.last_op = if load { "ldm" } else { "stm" };
        Ok(())
    }
}

fn condition(cpsr: u32, cond: u32) -> bool {
    let n = cpsr & 0x8000_0000 != 0;
    let z = cpsr & 0x4000_0000 != 0;
    let c = cpsr & 0x2000_0000 != 0;
    let v = cpsr & 0x1000_0000 != 0;
    match cond {
        0x0 => z,
        0x1 => !z,
        0x2 => c,
        0x3 => !c,
        0x4 => n,
        0x5 => !n,
        0x6 => v,
        0x7 => !v,
        0x8 => c && !z,
        0x9 => !c || z,
        0xA => n == v,
        0xB => n != v,
        0xC => !z && n == v,
        0xD => z || n != v,
        0xE => true,
        _ => false,
    }
}
