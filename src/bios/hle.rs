//! BiosHle SWI stubs + soft-boot side effects.
//!
//! Cited: GBATEK — BIOS Functions (SoftReset / Halt / IntrWait / Div / Sqrt / CpuSet)
//!   https://problemkaputt.de/gbatek.htm#biosfunctionsummary
//! Cited: GBATEK — BIOS Halt Functions / Reset Functions
//!   https://problemkaputt.de/gbatek.htm#bioshaltfunctions
//!   https://problemkaputt.de/gbatek-bios-reset-functions.htm
//! Cited: GBATEK — BIOS Decompression Functions (LZ77 / RL / Diff)
//!   https://problemkaputt.de/gbatek.htm#biosdecompressionfunctions
//! Cited: GBATEK — BIOS Math / Affine / Sound (ArcTan / BgAffineSet / ObjAffineSet / MidiKey2Freq)
//!   https://problemkaputt.de/gbatek.htm#biosarithmeticfunctions
//!   https://problemkaputt.de/gbatek.htm#biosaffinefunctions
//!   https://problemkaputt.de/gbatek.htm#biossoundfunctions
//! Cited: jsmolka/gba-tests — Div + Sqrt used by suite ROMs (MIT)
//! Cross-check: mGBA `src/gba/bios.c` `_unLz77` / `_ObjAffineSet` / `_MidiKey2Freq` / `_ArcTan`
//! Research: Project store `docs/graycart-gba/06-cart-bios-saves.md` §3
//! Note: Div remains the arm/thumb fail-digit path; Sqrt needed for `bios.gba`.
//!   Halt/IntrWait/VBlankIntrWait are required for commercial carts under BiosHle
//!   (unhandled SWI vectors to empty BIOS `0x08` → permanent black screen).
//!   LZ77 SWI `12h` is required after that fix — FireRed resumes but leaves
//!   garbage tiles when VRAM decompress is stubbed.
//!   ObjAffineSet SWI `0Fh` (not MidiKey2Freq — that is `1Fh`) is hit when FireRed
//!   enables windows / affine OBJ; leave unhandled and matrices stay identity-zero.

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
    pub const ARCTAN: u8 = 0x09;
    pub const ARCTAN2: u8 = 0x0A;
    pub const CPU_SET: u8 = 0x0B;
    pub const CPU_FAST_SET: u8 = 0x0C;
    /// BgAffineSet — compute BG PA/PB/PC/PD + origin.
    pub const BG_AFFINE_SET: u8 = 0x0E;
    /// ObjAffineSet — compute OBJ affine matrix (FireRed SWI `0Fh`).
    pub const OBJ_AFFINE_SET: u8 = 0x0F;
    /// LZ77UnCompReadNormalWrite8bit (WRAM byte stores).
    pub const LZ77_UNCOMP_WRITE8: u8 = 0x11;
    /// LZ77UnCompReadNormalWrite16bit (VRAM halfword stores).
    pub const LZ77_UNCOMP_WRITE16: u8 = 0x12;
    /// RLUnCompReadNormalWrite8bit.
    pub const RL_UNCOMP_WRITE8: u8 = 0x14;
    /// RLUnCompReadNormalWrite16bit.
    pub const RL_UNCOMP_WRITE16: u8 = 0x15;
    /// Diff8bitUnFilterWrite8bit.
    pub const DIFF8_UNFILTER_WRITE8: u8 = 0x16;
    /// Diff8bitUnFilterWrite16bit.
    pub const DIFF8_UNFILTER_WRITE16: u8 = 0x17;
    /// Diff16bitUnFilter.
    pub const DIFF16_UNFILTER: u8 = 0x18;
    /// MidiKey2Freq — music pitch from WaveData key + midi/fine.
    pub const MIDI_KEY2FREQ: u8 = 0x1F;
    pub const CUSTOM_HALT: u8 = 0x27;
}

/// Destination write width for decompress SWIs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StoreWidth {
    /// Byte stores (WRAM variants).
    Byte,
    /// Halfword-buffered stores (VRAM variants).
    Half,
}

/// Result of a BiosHle SWI attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwiHleResult {
    /// Not implemented — under BiosHle the CPU must **not** vector into empty
    /// BIOS (that runaway is what Dave’s FireRed log showed: `pc=0x0011…`).
    /// Caller resumes past the SWI and logs `swi unhandled`.
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
        swi::ARCTAN => {
            let (out, a, b) = arctan_i32(cpu.regs.get(0) as i32);
            cpu.regs.set(0, out as u32);
            cpu.regs.set(1, a as u32);
            cpu.regs.set(3, b as u32);
            SwiHleResult::Done
        }
        swi::ARCTAN2 => {
            let x = cpu.regs.get(0) as i32;
            let y = cpu.regs.get(1) as i32;
            let (out, scratch) = arctan2_i32(x, y);
            cpu.regs.set(0, u32::from(out));
            cpu.regs.set(1, scratch as u32);
            cpu.regs.set(3, 0x170);
            SwiHleResult::Done
        }
        swi::CPU_SET | swi::CPU_FAST_SET => {
            hle_cpu_set(cpu, bus, number == swi::CPU_FAST_SET);
            SwiHleResult::Done
        }
        swi::BG_AFFINE_SET => {
            hle_bg_affine_set(cpu, bus);
            SwiHleResult::Done
        }
        swi::OBJ_AFFINE_SET => {
            hle_obj_affine_set(cpu, bus);
            SwiHleResult::Done
        }
        swi::LZ77_UNCOMP_WRITE8 => {
            hle_lz77(cpu, bus, StoreWidth::Byte);
            SwiHleResult::Done
        }
        swi::LZ77_UNCOMP_WRITE16 => {
            hle_lz77(cpu, bus, StoreWidth::Half);
            SwiHleResult::Done
        }
        swi::RL_UNCOMP_WRITE8 => {
            hle_rl(cpu, bus, StoreWidth::Byte);
            SwiHleResult::Done
        }
        swi::RL_UNCOMP_WRITE16 => {
            hle_rl(cpu, bus, StoreWidth::Half);
            SwiHleResult::Done
        }
        swi::DIFF8_UNFILTER_WRITE8 => {
            hle_diff(cpu, bus, /*in_half=*/ false, StoreWidth::Byte);
            SwiHleResult::Done
        }
        swi::DIFF8_UNFILTER_WRITE16 => {
            hle_diff(cpu, bus, /*in_half=*/ false, StoreWidth::Half);
            SwiHleResult::Done
        }
        swi::DIFF16_UNFILTER => {
            hle_diff(cpu, bus, /*in_half=*/ true, StoreWidth::Half);
            SwiHleResult::Done
        }
        swi::MIDI_KEY2FREQ => {
            hle_midi_key2freq(cpu, bus);
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

/// GBA angle high-byte → radians (full circle = 256 units → 2π).
fn gba_theta(angle: u16) -> f32 {
    f32::from(angle >> 8) / 128.0 * std::f32::consts::PI
}

/// Truncate float→i16 the way C `store16(..., float)` does (toward zero).
fn f32_to_i16_trunc(v: f32) -> i16 {
    v as i32 as i16
}

fn hle_obj_affine_set(cpu: &mut Cpu, bus: &mut impl CpuMem) {
    let mut src = cpu.regs.get(0);
    let mut dst = cpu.regs.get(1);
    let mut count = cpu.regs.get(2);
    let stride = cpu.regs.get(3);
    while count > 0 {
        count -= 1;
        let sx = f32::from(bus.read16(src) as i16) / 256.0;
        let sy = f32::from(bus.read16(src.wrapping_add(2)) as i16) / 256.0;
        let theta = gba_theta(bus.read16(src.wrapping_add(4)));
        src = src.wrapping_add(8);
        let (sin, cos) = theta.sin_cos();
        // [sx 0; 0 sy] * [cos -sin; sin cos]
        let a = cos * sx;
        let b = -sin * sx;
        let c = sin * sy;
        let d = cos * sy;
        bus.write16(dst, f32_to_i16_trunc(a * 256.0) as u16);
        bus.write16(dst.wrapping_add(stride), f32_to_i16_trunc(b * 256.0) as u16);
        bus.write16(
            dst.wrapping_add(stride.wrapping_mul(2)),
            f32_to_i16_trunc(c * 256.0) as u16,
        );
        bus.write16(
            dst.wrapping_add(stride.wrapping_mul(3)),
            f32_to_i16_trunc(d * 256.0) as u16,
        );
        dst = dst.wrapping_add(stride.wrapping_mul(4));
    }
}

fn hle_bg_affine_set(cpu: &mut Cpu, bus: &mut impl CpuMem) {
    let mut src = cpu.regs.get(0);
    let mut dst = cpu.regs.get(1);
    let mut count = cpu.regs.get(2);
    while count > 0 {
        count -= 1;
        let ox = (bus.read32(src) as i32) as f32 / 256.0;
        let oy = (bus.read32(src.wrapping_add(4)) as i32) as f32 / 256.0;
        let cx = f32::from(bus.read16(src.wrapping_add(8)) as i16);
        let cy = f32::from(bus.read16(src.wrapping_add(10)) as i16);
        let sx = f32::from(bus.read16(src.wrapping_add(12)) as i16) / 256.0;
        let sy = f32::from(bus.read16(src.wrapping_add(14)) as i16) / 256.0;
        let theta = gba_theta(bus.read16(src.wrapping_add(16)));
        src = src.wrapping_add(20);
        let (sin, cos) = theta.sin_cos();
        let a = cos * sx;
        let b = -sin * sx;
        let c = sin * sy;
        let d = cos * sy;
        let rx = ox - (a * cx + b * cy);
        let ry = oy - (c * cx + d * cy);
        bus.write16(dst, f32_to_i16_trunc(a * 256.0) as u16);
        bus.write16(dst.wrapping_add(2), f32_to_i16_trunc(b * 256.0) as u16);
        bus.write16(dst.wrapping_add(4), f32_to_i16_trunc(c * 256.0) as u16);
        bus.write16(dst.wrapping_add(6), f32_to_i16_trunc(d * 256.0) as u16);
        bus.write32(dst.wrapping_add(8), (rx * 256.0) as i32 as u32);
        bus.write32(dst.wrapping_add(12), (ry * 256.0) as i32 as u32);
        dst = dst.wrapping_add(16);
    }
}

fn hle_midi_key2freq(cpu: &mut Cpu, bus: &mut impl CpuMem) {
    let wave = cpu.regs.get(0);
    let key = bus.read32(wave.wrapping_add(4));
    let mk = cpu.regs.get(1) as f32;
    let fine = cpu.regs.get(2) as f32 / 256.0;
    let denom = ((180.0 - mk - fine) / 12.0).exp2();
    cpu.regs.set(0, (key as f32 / denom) as u32);
}

/// BIOS ArcTan fixed-point polynomial (mGBA / GBATEK). Returns (r0, r1, r3).
fn arctan_i32(i: i32) -> (i16, i32, i32) {
    let a = -((i.wrapping_mul(i)) >> 14);
    let mut b = ((0xA9i32.wrapping_mul(a)) >> 14).wrapping_add(0x390);
    b = (b.wrapping_mul(a) >> 14).wrapping_add(0x91C);
    b = (b.wrapping_mul(a) >> 14).wrapping_add(0xFB6);
    b = (b.wrapping_mul(a) >> 14).wrapping_add(0x16AA);
    b = (b.wrapping_mul(a) >> 14).wrapping_add(0x2081);
    b = (b.wrapping_mul(a) >> 14).wrapping_add(0x3651);
    b = (b.wrapping_mul(a) >> 14).wrapping_add(0xA2F9);
    let out = (i.wrapping_mul(b) >> 16) as i16;
    (out, a, b)
}

fn arctan2_i32(x: i32, y: i32) -> (u16, i32) {
    if y == 0 {
        return (if x >= 0 { 0 } else { 0x8000 }, 0);
    }
    if x == 0 {
        return (if y >= 0 { 0x4000 } else { 0xC000 }, 0);
    }
    if y >= 0 {
        if x >= 0 {
            if x >= y {
                let (v, a, _) = arctan_i32((y << 14) / x);
                return (v as u16, a);
            }
        } else if -x >= y {
            let (v, a, _) = arctan_i32((y << 14) / x);
            return ((v as u16).wrapping_add(0x8000), a);
        }
        let (v, a, _) = arctan_i32((x << 14) / y);
        return (0x4000u16.wrapping_sub(v as u16), a);
    }
    if x <= 0 {
        if -x > -y {
            let (v, a, _) = arctan_i32((y << 14) / x);
            return ((v as u16).wrapping_add(0x8000), a);
        }
    } else if x >= -y {
        // mGBA: ArcTan(...) + 0x10000, then store as uint16_t (wrap).
        let (v, a, _) = arctan_i32((y << 14) / x);
        return ((i32::from(v).wrapping_add(0x1_0000)) as u16, a);
    }
    let (v, a, _) = arctan_i32((x << 14) / y);
    (0xC000u16.wrapping_sub(v as u16), a)
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

/// Emit one decompressed byte with WRAM (byte) or VRAM (halfword-buffered) stores.
fn store_decomp_byte(
    bus: &mut impl CpuMem,
    dest: &mut u32,
    halfword: &mut u16,
    width: StoreWidth,
    byte: u8,
) {
    match width {
        StoreWidth::Byte => {
            bus.write8(*dest, byte);
            *dest = dest.wrapping_add(1);
        }
        StoreWidth::Half => {
            if (*dest & 1) != 0 {
                *halfword |= u16::from(byte) << 8;
                bus.write16(*dest ^ 1, *halfword);
            } else {
                *halfword = u16::from(byte);
            }
            *dest = dest.wrapping_add(1);
        }
    }
}

fn hle_lz77(cpu: &mut Cpu, bus: &mut impl CpuMem, width: StoreWidth) {
    let mut source = cpu.regs.get(0);
    let mut dest = cpu.regs.get(1);
    let header = bus.read32(source);
    // Size in bits 8–31; type nybble (10h) unchecked like commercial BIOS soft paths.
    let mut remaining = header >> 8;
    source = source.wrapping_add(4);
    let mut blocks_remaining = 0u8;
    let mut blockheader = 0u8;
    let mut halfword = 0u16;
    while remaining > 0 {
        if blocks_remaining == 0 {
            blockheader = bus.read8(source);
            source = source.wrapping_add(1);
            blocks_remaining = 8;
            continue;
        }
        if blockheader & 0x80 != 0 {
            // Compressed: 16-bit BE length/disp, then copy from sliding window.
            let hi = u16::from(bus.read8(source));
            let lo = u16::from(bus.read8(source.wrapping_add(1)));
            source = source.wrapping_add(2);
            let block = (hi << 8) | lo;
            let mut disp = dest.wrapping_sub(u32::from(block & 0x0FFF)).wrapping_sub(1);
            let mut bytes = usize::from((block >> 12) + 3);
            while bytes > 0 {
                bytes -= 1;
                remaining = remaining.saturating_sub(1);
                let byte = match width {
                    StoreWidth::Byte => bus.read8(disp),
                    StoreWidth::Half => {
                        let hw = bus.read16(disp & !1);
                        ((hw >> ((disp & 1) * 8)) & 0xFF) as u8
                    }
                };
                store_decomp_byte(bus, &mut dest, &mut halfword, width, byte);
                disp = disp.wrapping_add(1);
            }
        } else {
            let byte = bus.read8(source);
            source = source.wrapping_add(1);
            store_decomp_byte(bus, &mut dest, &mut halfword, width, byte);
            remaining = remaining.saturating_sub(1);
        }
        blockheader <<= 1;
        blocks_remaining -= 1;
    }
    cpu.regs.set(0, source);
    cpu.regs.set(1, dest);
    cpu.regs.set(3, 0);
}

fn hle_rl(cpu: &mut Cpu, bus: &mut impl CpuMem, width: StoreWidth) {
    let mut source = cpu.regs.get(0);
    let mut dest = cpu.regs.get(1);
    // RL header is read from word-aligned source (GBATEK / mGBA).
    let header = bus.read32(source & !3);
    let mut remaining = header >> 8;
    let mut padding = (4u32.wrapping_sub(remaining)) & 3;
    source = source.wrapping_add(4);
    let mut halfword = 0u16;
    while remaining > 0 {
        let blockheader = bus.read8(source);
        source = source.wrapping_add(1);
        if blockheader & 0x80 != 0 {
            let mut count = u32::from(blockheader & 0x7F) + 3;
            let fill = bus.read8(source);
            source = source.wrapping_add(1);
            while count > 0 && remaining > 0 {
                count -= 1;
                remaining -= 1;
                store_decomp_byte(bus, &mut dest, &mut halfword, width, fill);
            }
        } else {
            let mut count = u32::from(blockheader) + 1;
            while count > 0 && remaining > 0 {
                count -= 1;
                remaining -= 1;
                let byte = bus.read8(source);
                source = source.wrapping_add(1);
                store_decomp_byte(bus, &mut dest, &mut halfword, width, byte);
            }
        }
    }
    // Pad destination to a multiple of 4 (BIOS behaviour).
    if width == StoreWidth::Half {
        if dest & 1 != 0 {
            padding = padding.saturating_sub(1);
            dest = dest.wrapping_add(1);
        }
        while padding > 0 {
            bus.write16(dest, 0);
            dest = dest.wrapping_add(2);
            padding = padding.saturating_sub(2);
        }
    } else {
        while padding > 0 {
            bus.write8(dest, 0);
            dest = dest.wrapping_add(1);
            padding -= 1;
        }
    }
    cpu.regs.set(0, source);
    cpu.regs.set(1, dest);
}

fn hle_diff(cpu: &mut Cpu, bus: &mut impl CpuMem, in_half: bool, out: StoreWidth) {
    let mut source = cpu.regs.get(0) & !3;
    let mut dest = cpu.regs.get(1);
    let header = bus.read32(source);
    let mut remaining = header >> 8;
    source = source.wrapping_add(4);
    let mut old: u16 = 0;
    let mut halfword: u16 = 0;
    let in_step = if in_half { 2u32 } else { 1u32 };
    while remaining > 0 {
        let mut new = if in_half {
            bus.read16(source)
        } else {
            u16::from(bus.read8(source))
        };
        new = new.wrapping_add(old);
        match out {
            StoreWidth::Byte => {
                bus.write8(dest, new as u8);
                dest = dest.wrapping_add(1);
                remaining = remaining.saturating_sub(1);
            }
            StoreWidth::Half if !in_half => {
                // Diff8 → Write16: accumulate two bytes into one halfword store.
                halfword >>= 8;
                halfword |= new << 8;
                if source & 1 != 0 {
                    bus.write16(dest, halfword);
                    dest = dest.wrapping_add(2);
                    remaining = remaining.saturating_sub(2);
                }
            }
            StoreWidth::Half => {
                bus.write16(dest, new);
                dest = dest.wrapping_add(2);
                remaining = remaining.saturating_sub(2);
            }
        }
        old = new;
        source = source.wrapping_add(in_step);
    }
    cpu.regs.set(0, source);
    cpu.regs.set(1, dest);
}
