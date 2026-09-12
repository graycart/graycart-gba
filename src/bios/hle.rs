//! BiosHle SWI stubs + soft-boot side effects.
//!
//! Cited: GBATEK — BIOS Functions (SoftReset / Halt / IntrWait / Div / Sqrt / CpuSet)
//!   https://problemkaputt.de/gbatek.htm#biosfunctionsummary
//! Cited: GBATEK — BIOS Halt Functions / Reset Functions
//!   https://problemkaputt.de/gbatek.htm#bioshaltfunctions
//!   https://problemkaputt.de/gbatek-bios-reset-functions.htm
//! Cited: jsmolka/gba-tests — Div + Sqrt used by suite ROMs (MIT)
//! Research: Project store `docs/graycart-gba/06-cart-bios-saves.md` §3
//! Note: Div remains the arm/thumb fail-digit path; Sqrt needed for `bios.gba`.
//!   Halt/IntrWait/VBlankIntrWait are required for commercial carts under BiosHle
//!   (unhandled SWI vectors to empty BIOS `0x08` → permanent black screen).

use crate::bus::CpuMem;
use crate::cpu::{soft_boot, Cpu};
use crate::irq::{self, IRQ_VBLANK};

use super::protect::{LATCH_AFTER_SWI, LATCH_SOFT_RESET};

/// SWI numbers handled by BiosHle (comment field).
pub mod swi {
    pub const SOFT_RESET: u8 = 0x00;
    pub const REGISTER_RAM_RESET: u8 = 0x01;
    pub const HALT: u8 = 0x02;
    pub const STOP: u8 = 0x03;
    pub const INTR_WAIT: u8 = 0x04;
    pub const VBLANK_INTR_WAIT: u8 = 0x05;
    pub const DIV: u8 = 0x06;
    pub const DIV_ARM: u8 = 0x07;
    pub const SQRT: u8 = 0x08;
    pub const CPU_SET: u8 = 0x0B;
    pub const CPU_FAST_SET: u8 = 0x0C;
    pub const CUSTOM_HALT: u8 = 0x27;
}

/// Result of a BiosHle SWI attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwiHleResult {
    /// Not implemented — caller should take the real SWI exception vector.
    Unhandled,
    /// Finished; resume at the next instruction.
    Done,
    /// SoftReset already rewrote PC/stacks.
    SoftReset,
    /// IntrWait / VBlankIntrWait: Halt entered; `mask` is the BIOS-flag wait mask.
    IntrWait(u16),
}

/// Apply soft-boot CPU state and return the BIOS-protect latch to install.
#[must_use]
pub fn soft_boot_cart(cpu: &mut Cpu) -> u32 {
    soft_boot::apply(cpu, soft_boot::CART_ENTRY);
    LATCH_SOFT_RESET
}

/// Apply multiboot soft entry; same SoftReset latch.
#[must_use]
pub fn soft_boot_multiboot(cpu: &mut Cpu) -> u32 {
    soft_boot::apply(cpu, soft_boot::MULTIBOOT_ENTRY);
    LATCH_SOFT_RESET
}

/// Try to handle a SWI under BiosHle.
///
/// On [`SwiHleResult::Done`] / [`SwiHleResult::IntrWait`], caller should resume at
/// the next instruction and set the After-SWI protect latch
/// ([`LATCH_AFTER_SWI`]), except SoftReset which uses SoftReset latch.
pub fn try_swi(cpu: &mut Cpu, bus: &mut impl CpuMem, number: u8) -> SwiHleResult {
    match number {
        swi::SOFT_RESET => {
            // Flag at 3007FFA: 0 → ROM, else RAM.
            let flag = bus.read8(0x0300_7FFA);
            let entry = if flag == 0 {
                soft_boot::CART_ENTRY
            } else {
                soft_boot::MULTIBOOT_ENTRY
            };
            soft_boot::apply(cpu, entry);
            SwiHleResult::SoftReset
        }
        swi::REGISTER_RAM_RESET => {
            hle_register_ram_reset(cpu, bus);
            SwiHleResult::Done
        }
        swi::HALT => {
            bus.write8(0x0400_0301, 0x00);
            SwiHleResult::Done
        }
        swi::STOP => {
            bus.write8(0x0400_0301, 0x80);
            SwiHleResult::Done
        }
        swi::CUSTOM_HALT => {
            bus.write8(0x0400_0301, cpu.regs.get(2) as u8);
            SwiHleResult::Done
        }
        swi::INTR_WAIT => hle_intr_wait(cpu, bus, cpu.regs.get(0), cpu.regs.get(1) as u16),
        swi::VBLANK_INTR_WAIT => {
            // GBATEK: sets r0=1, r1=1 then IntrWait.
            cpu.regs.set(0, 1);
            cpu.regs.set(1, u32::from(IRQ_VBLANK));
            hle_intr_wait(cpu, bus, 1, IRQ_VBLANK)
        }
        swi::DIV | swi::DIV_ARM => {
            let (num, den) = if number == swi::DIV_ARM {
                (cpu.regs.get(1) as i32, cpu.regs.get(0) as i32)
            } else {
                (cpu.regs.get(0) as i32, cpu.regs.get(1) as i32)
            };
            if den != 0 {
                let quot = num / den;
                let rem = num % den;
                if number == swi::DIV_ARM {
                    cpu.regs.set(1, quot as u32);
                    cpu.regs.set(0, rem as u32);
                } else {
                    cpu.regs.set(0, quot as u32);
                    cpu.regs.set(1, rem as u32);
                }
                cpu.regs.set(3, quot.unsigned_abs());
            }
            SwiHleResult::Done
        }
        swi::SQRT => {
            let v = cpu.regs.get(0);
            cpu.regs.set(0, isqrt_u32(v));
            SwiHleResult::Done
        }
        swi::CPU_SET | swi::CPU_FAST_SET => {
            hle_cpu_set(cpu, bus, number == swi::CPU_FAST_SET);
            SwiHleResult::Done
        }
        _ => SwiHleResult::Unhandled,
    }
}

/// After a handled SWI, install this latch (except SoftReset which uses SoftReset latch).
#[must_use]
pub fn latch_after_handled_swi(number: u8) -> u32 {
    if number == swi::SOFT_RESET {
        LATCH_SOFT_RESET
    } else {
        LATCH_AFTER_SWI
    }
}

fn hle_intr_wait(
    cpu: &mut Cpu,
    bus: &mut impl CpuMem,
    discard_old: u32,
    mask: u16,
) -> SwiHleResult {
    let mask = mask & irq::IRQ_SOURCE_MASK;
    // Force IME=1 (GBATEK IntrWait).
    bus.write32(0x0400_0208, 1);

    // Read BIOS Interrupt Check Flags @ 03007FF8.
    let mut flags = read_intr_flags(bus);
    if discard_old != 0 {
        flags &= !mask;
        write_intr_flags(bus, flags);
    } else if flags & mask != 0 {
        // Already set — return immediately without Halt.
        write_intr_flags(bus, flags & !mask);
        let _ = cpu;
        return SwiHleResult::Done;
    }

    // Enter Halt; Gba polls `hle_intr_wait_mask` across frames.
    bus.write8(0x0400_0301, 0x00);
    SwiHleResult::IntrWait(mask)
}

fn read_intr_flags(bus: &mut impl CpuMem) -> u16 {
    u16::from(bus.read8(irq::INTR_WAIT_FLAGS_ADDR))
        | (u16::from(bus.read8(irq::INTR_WAIT_FLAGS_ADDR.wrapping_add(1))) << 8)
}

fn write_intr_flags(bus: &mut impl CpuMem, flags: u16) {
    bus.write8(irq::INTR_WAIT_FLAGS_ADDR, flags as u8);
    bus.write8(
        irq::INTR_WAIT_FLAGS_ADDR.wrapping_add(1),
        (flags >> 8) as u8,
    );
}

fn hle_register_ram_reset(cpu: &mut Cpu, bus: &mut impl CpuMem) {
    let flags = cpu.regs.get(0);
    // Bit0: clear 256K EWRAM (02000000–0203FFFF).
    if flags & 1 != 0 {
        clear_range(bus, 0x0200_0000, 0x4_0000);
    }
    // Bit1: clear 32K IWRAM excluding last 0x200 bytes (03007E00–03007FFF).
    if flags & 2 != 0 {
        clear_range(bus, 0x0300_0000, 0x7E00);
    }
    // Bit2: palette
    if flags & 4 != 0 {
        clear_range(bus, 0x0500_0000, 0x400);
    }
    // Bit3: VRAM
    if flags & 8 != 0 {
        clear_range(bus, 0x0600_0000, 0x1_8000);
    }
    // Bit4: OAM
    if flags & 0x10 != 0 {
        clear_range(bus, 0x0700_0000, 0x400);
    }
    // Bit5: SIO → general-purpose (RCNT)
    if flags & 0x20 != 0 {
        bus.write16(0x0400_0134, 0);
    }
    // Bit6: sound registers — clear SOUNDCNT_X master + common ports.
    if flags & 0x40 != 0 {
        for off in (0x60u32..0xA8).step_by(2) {
            bus.write16(0x0400_0000 | off, 0);
        }
    }
    // Bit7: other registers (timers/DMA/keypad/irq/wait/…) — light clear.
    if flags & 0x80 != 0 {
        for off in [
            0xB0u32, 0xBC, 0xC8, 0xD4, 0x100, 0x104, 0x108, 0x10C, 0x200, 0x204,
        ] {
            bus.write16(0x0400_0000 | off, 0);
        }
        bus.write32(0x0400_0208, 0);
    }
    // Always force forced-blank DISPCNT (GBATEK).
    bus.write16(0x0400_0000, 0x0080);
}

fn clear_range(bus: &mut impl CpuMem, base: u32, len: u32) {
    // Word clears where possible; CpuMem may open-bus unused mirrors.
    let mut addr = base;
    let end = base.wrapping_add(len);
    while addr.wrapping_add(4) <= end && addr < end {
        bus.write32(addr, 0);
        addr = addr.wrapping_add(4);
    }
    while addr < end {
        bus.write8(addr, 0);
        addr = addr.wrapping_add(1);
    }
}

fn isqrt_u32(n: u32) -> u32 {
    // Integer square root (floor).
    if n <= 1 {
        return n;
    }
    let mut y = n;
    let mut z = (y.saturating_add(n / y)) / 2;
    while z < y {
        y = z;
        z = (y.saturating_add(n / y)) / 2;
    }
    y
}

fn hle_cpu_set(cpu: &mut Cpu, bus: &mut impl CpuMem, fast: bool) {
    let src = cpu.regs.get(0);
    let dst = cpu.regs.get(1);
    let ctl = cpu.regs.get(2);
    // Reject BIOS as source (GBATEK).
    if src < 0x4000 {
        return;
    }
    let count = ctl & 0x001F_FFFF;
    let fill = (ctl & (1 << 24)) != 0;
    if fast {
        let words = count;
        if fill {
            let v = bus.read32(src);
            for i in 0..words {
                bus.write32(dst.wrapping_add(i.wrapping_mul(4)), v);
            }
        } else {
            for i in 0..words {
                let v = bus.read32(src.wrapping_add(i.wrapping_mul(4)));
                bus.write32(dst.wrapping_add(i.wrapping_mul(4)), v);
            }
        }
    } else {
        let halfwords = count;
        let word_mode = (ctl & (1 << 26)) != 0;
        if word_mode {
            if fill {
                let v = bus.read32(src);
                for i in 0..halfwords {
                    bus.write32(dst.wrapping_add(i.wrapping_mul(4)), v);
                }
            } else {
                for i in 0..halfwords {
                    let v = bus.read32(src.wrapping_add(i.wrapping_mul(4)));
                    bus.write32(dst.wrapping_add(i.wrapping_mul(4)), v);
                }
            }
        } else if fill {
            let v = bus.read16(src);
            for i in 0..halfwords {
                bus.write16(dst.wrapping_add(i.wrapping_mul(2)), v);
            }
        } else {
            for i in 0..halfwords {
                let v = bus.read16(src.wrapping_add(i.wrapping_mul(2)));
                bus.write16(dst.wrapping_add(i.wrapping_mul(2)), v);
            }
        }
    }
}
