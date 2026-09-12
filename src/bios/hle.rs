//! BiosHle SWI stubs + soft-boot side effects.
//!
//! Cited: GBATEK — BIOS Functions (SoftReset / Div / Sqrt / CpuSet)
//!   https://problemkaputt.de/gbatek.htm#biosfunctionsummary
//! Cited: jsmolka/gba-tests — Div + Sqrt used by suite ROMs (MIT)
//! Research: Project store `docs/graycart-gba/06-cart-bios-saves.md` §3
//! Note: Div remains the arm/thumb fail-digit path; Sqrt needed for `bios.gba`.

use crate::bus::CpuMem;
use crate::cpu::{soft_boot, Cpu};

use super::protect::{LATCH_AFTER_SWI, LATCH_SOFT_RESET};

/// SWI numbers handled by BiosHle (comment field).
pub mod swi {
    pub const SOFT_RESET: u8 = 0x00;
    pub const DIV: u8 = 0x06;
    pub const DIV_ARM: u8 = 0x07;
    pub const SQRT: u8 = 0x08;
    pub const CPU_SET: u8 = 0x0B;
    pub const CPU_FAST_SET: u8 = 0x0C;
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

/// Try to handle a SWI under BiosHle. Returns `true` if handled.
///
/// On success, caller should resume at the next instruction and set the
/// After-SWI protect latch ([`LATCH_AFTER_SWI`]).
pub fn try_swi(cpu: &mut Cpu, bus: &mut impl CpuMem, number: u8) -> bool {
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
            true
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
            true
        }
        swi::SQRT => {
            let v = cpu.regs.get(0);
            cpu.regs.set(0, isqrt_u32(v));
            true
        }
        swi::CPU_SET | swi::CPU_FAST_SET => {
            hle_cpu_set(cpu, bus, number == swi::CPU_FAST_SET);
            true
        }
        _ => false,
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
