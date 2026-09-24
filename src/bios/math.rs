//! BIOS math, affine, SoftReset, RegisterRamReset, BitUnPack, SoundBias.
//!
//! Cited: GBATEK BIOS Arithmetic / Rotation-Scaling / Reset / Decompression / Sound.
//! <https://problemkaputt.de/gbatek.htm>

use crate::bus::Bus;

/// Integer square root (floor). `sqrt(0)=0`, `sqrt(2)=1`.
pub fn sqrt_u32(n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    let mut op = n;
    let mut res = 0u32;
    let mut one = 1u32 << 30;
    while one > op {
        one >>= 2;
    }
    while one != 0 {
        if op >= res + one {
            op -= res + one;
            res = (res >> 1) + one;
        } else {
            res >>= 1;
        }
        one >>= 2;
    }
    res
}

/// BIOS ArcTan polynomial on a 2.14 fixed tangent (GBATEK / mGBA HLE).
/// Explicit `i64` multiplies; input clamped so later `b * a` steps cannot
/// overflow i64 under debug overflow checks.
fn arctan_bios_214(raw: i32) -> i32 {
    // Useful BIOS range is near ±1 in 2.14 (`±0x4000`). Clamp keeps extremes
    // (i32::MAX / MIN after the 16.16→2.14 shift) from exploding the poly
    // while still allowing `arctan(0x10000) == 0x4000` (shifted input `0x4000`).
    let i = i64::from(raw.clamp(-0x1_0000, 0x1_0000));
    let a: i64 = -((i * i) >> 14);
    let mut b: i64 = ((0xA9_i64 * a) >> 14) + 0x390;
    b = ((b * a) >> 14) + 0x91C;
    b = ((b * a) >> 14) + 0xFB6;
    b = ((b * a) >> 14) + 0x16AA;
    b = ((b * a) >> 14) + 0x2081;
    b = ((b * a) >> 14) + 0x3651;
    b = ((b * a) >> 14) + 0xA2F9;
    ((i * b) >> 16) as i32
}

/// ArcTan of a signed 16.16 tangent.
///
/// Angle scale: BIOS units stretched so π/4 = `0x4000` (input `0x10000` = 1.0).
/// That is the GBATEK 2.14 ArcTan result shifted left by 1.
pub fn arctan(tag: i32) -> i32 {
    arctan_bios_214(tag >> 2).wrapping_shl(1)
}

/// ArcTan2 of signed 16.16 `x`,`y`. Result is a signed 16-bit BIOS angle
/// (`0x10000` = 2π); `arctan2(positive x, 0) == 0`.
pub fn arctan2(x: i32, y: i32) -> i16 {
    let r = if y == 0 {
        if x >= 0 { 0 } else { 0x8000 }
    } else if x == 0 {
        if y >= 0 { 0x4000 } else { 0xC000 }
    } else if y >= 0 {
        if x >= 0 {
            if x >= y {
                arctan_bios_214(y.wrapping_shl(14).wrapping_div(x))
            } else {
                0x4000 - arctan_bios_214(x.wrapping_shl(14).wrapping_div(y))
            }
        } else if x.wrapping_neg() >= y {
            arctan_bios_214(y.wrapping_shl(14).wrapping_div(x)).wrapping_add(0x8000)
        } else {
            0x4000 - arctan_bios_214(x.wrapping_shl(14).wrapping_div(y))
        }
    } else if x <= 0 {
        if x.wrapping_neg() > y.wrapping_neg() {
            arctan_bios_214(y.wrapping_shl(14).wrapping_div(x)).wrapping_add(0x8000)
        } else {
            0xC000 - arctan_bios_214(x.wrapping_shl(14).wrapping_div(y))
        }
    } else if x >= y.wrapping_neg() {
        arctan_bios_214(y.wrapping_shl(14).wrapping_div(x)).wrapping_add(0x10000)
    } else {
        0xC000 - arctan_bios_214(x.wrapping_shl(14).wrapping_div(y))
    };
    r as i16
}

/// SWI 0x19 — set SOUNDBIAS bits 9–1 from `target` (`0` or `0x100`) << 1.
/// Preserves bits above bit 9. Immediate write (no BIOS delay spin).
pub fn sound_bias(bus: &mut Bus, target: u16) {
    let cur = bus.read16(0x0400_0088);
    let new = (cur & !0x03FE) | ((target << 1) & 0x03FE);
    bus.write16(0x0400_0088, new);
}

/// SWI 0x00 — clear `0x03007E00..=0x03007FFF` except the 32-bit flag at `0x03007FFA`.
/// Returns the jump target: ROM if flag bit 0 is clear, else EWRAM.
pub fn soft_reset(bus: &mut Bus) -> u32 {
    let saved = [
        bus.read8(0x0300_7FFA),
        bus.read8(0x0300_7FFB),
        bus.read8(0x0300_7FFC),
        bus.read8(0x0300_7FFD),
    ];
    let flag0 = saved[0];
    let mut addr = 0x0300_7E00u32;
    while addr <= 0x0300_7FFF {
        bus.write8(addr, 0);
        addr = addr.wrapping_add(1);
    }
    bus.write8(0x0300_7FFA, saved[0]);
    bus.write8(0x0300_7FFB, saved[1]);
    bus.write8(0x0300_7FFC, saved[2]);
    bus.write8(0x0300_7FFD, saved[3]);
    if flag0 & 1 == 0 {
        0x0800_0000
    } else {
        0x0200_0000
    }
}

/// SWI 0x01 — RegisterRamReset. `flags` bits select blocks (GBATEK).
pub fn register_ram_reset(bus: &mut Bus, flags: u32) {
    // Always forced blank (GBATEK).
    bus.write16(0x0400_0000, 0x0080);
    if flags & 0x01 != 0 {
        clear_range(bus, 0x0200_0000, 256 * 1024);
    }
    if flags & 0x02 != 0 {
        // 32K IWRAM excluding last 0x200 bytes.
        clear_range(bus, 0x0300_0000, 32 * 1024 - 0x200);
    }
    if flags & 0x04 != 0 {
        clear_range(bus, 0x0500_0000, 1024);
    }
    if flags & 0x08 != 0 {
        clear_range(bus, 0x0600_0000, 96 * 1024);
    }
    if flags & 0x10 != 0 {
        clear_range(bus, 0x0700_0000, 1024);
    }
    if flags & 0x20 != 0 {
        // SIO → general-purpose mode.
        bus.write16(0x0400_0128, 0); // SIOCNT
        bus.write16(0x0400_0134, 0x8000); // RCNT initial
        bus.write16(0x0400_012A, 0);
        bus.write16(0x0400_0140, 0); // JOYCNT
        bus.write32(0x0400_0150, 0);
        bus.write32(0x0400_0154, 0);
    }
    if flags & 0x40 != 0 {
        for off in (0x60u32..=0x86).step_by(2) {
            bus.write16(0x0400_0000 + off, 0);
        }
        bus.write16(0x0400_0088, 0x0200); // SOUNDBIAS
        clear_range(bus, 0x0400_0090, 0x10); // wave RAM
    }
    if flags & 0x80 != 0 {
        // Other I/O except SIO and sound (keep-outs: SIO 0x120–0x15E, sound 0x60–0xA6).
        bus.write16(0x0400_0004, 0); // DISPSTAT
        for off in (0x08u32..=0x56).step_by(2) {
            let value = match off {
                0x20 | 0x30 => 0x0100, // BG2PA / BG3PA
                0x26 | 0x36 => 0x0100, // BG2PD / BG3PD
                _ => 0,
            };
            bus.write16(0x0400_0000 + off, value);
        }
        for off in (0xB0u32..=0xDE).step_by(2) {
            bus.write16(0x0400_0000 + off, 0); // DMA
        }
        for off in (0x100u32..=0x10E).step_by(2) {
            bus.write16(0x0400_0000 + off, 0); // timers
        }
        bus.write16(0x0400_0200, 0); // IE
        bus.write16(0x0400_0202, 0xFFFF); // IF clear
        bus.write16(0x0400_0204, 0); // WAITCNT
        bus.write16(0x0400_0208, 0); // IME
    }
}

fn clear_range(bus: &mut Bus, base: u32, len: usize) {
    let mut addr = base;
    let end = base.wrapping_add(len as u32);
    while addr != end {
        bus.write8(addr, 0);
        addr = addr.wrapping_add(1);
    }
}

/// Sin/cos for BIOS affine: upper 8 bits of angle cover a full turn; result is 8.8.
fn affine_sin_cos(angle: u16) -> (i32, i32) {
    let theta = f64::from(angle >> 8) * std::f64::consts::PI / 128.0;
    let sin = (theta.sin() * 256.0).round() as i32;
    let cos = (theta.cos() * 256.0).round() as i32;
    (sin, cos)
}

/// SWI 0x0E — BgAffineSet. `count` source records → destination matrices.
pub fn bg_affine_set(bus: &mut Bus, mut src: u32, mut dst: u32, count: u32) {
    for _ in 0..count {
        let ox = bus.read32(src) as i32;
        let oy = bus.read32(src.wrapping_add(4)) as i32;
        let cx = bus.read16(src.wrapping_add(8)) as i16 as i32;
        let cy = bus.read16(src.wrapping_add(10)) as i16 as i32;
        let sx = bus.read16(src.wrapping_add(12)) as i16 as i32;
        let sy = bus.read16(src.wrapping_add(14)) as i16 as i32;
        let alpha = bus.read16(src.wrapping_add(16));
        src = src.wrapping_add(20);

        let (sin, cos) = affine_sin_cos(alpha);
        let a = (sx * cos) >> 8;
        let b = -((sx * sin) >> 8);
        let c = (sy * sin) >> 8;
        let d = (sy * cos) >> 8;
        let rx = ox - (a * cx + b * cy);
        let ry = oy - (c * cx + d * cy);

        bus.write16(dst, a as u16);
        bus.write16(dst.wrapping_add(2), b as u16);
        bus.write16(dst.wrapping_add(4), c as u16);
        bus.write16(dst.wrapping_add(6), d as u16);
        bus.write32(dst.wrapping_add(8), rx as u32);
        bus.write32(dst.wrapping_add(12), ry as u32);
        dst = dst.wrapping_add(16);
    }
}

/// SWI 0x0F — ObjAffineSet. `stride` is bytes between PA/PB/PC/PD (2 or 8).
pub fn obj_affine_set(bus: &mut Bus, mut src: u32, mut dst: u32, count: u32, stride: u32) {
    let stride = stride as i32;
    for _ in 0..count {
        let sx = bus.read16(src) as i16 as i32;
        let sy = bus.read16(src.wrapping_add(2)) as i16 as i32;
        let alpha = bus.read16(src.wrapping_add(4));
        src = src.wrapping_add(8);

        let (sin, cos) = affine_sin_cos(alpha);
        let a = (sx * cos) >> 8;
        let b = -((sx * sin) >> 8);
        let c = (sy * sin) >> 8;
        let d = (sy * cos) >> 8;

        bus.write16(dst, a as u16);
        bus.write16(dst.wrapping_add(stride as u32), b as u16);
        bus.write16(dst.wrapping_add((stride * 2) as u32), c as u16);
        bus.write16(dst.wrapping_add((stride * 3) as u32), d as u16);
        dst = dst.wrapping_add((stride * 4) as u32);
    }
}

/// SWI 0x10 — BitUnPack. Info record at `info`: length, src width, dest width, offset/flag.
pub fn bit_unpack(bus: &mut Bus, mut source: u32, mut dest: u32, info: u32) {
    let source_len = u32::from(bus.read16(info));
    let source_width = u32::from(bus.read8(info.wrapping_add(2)));
    let dest_width = u32::from(bus.read8(info.wrapping_add(3)));
    let bias = bus.read32(info.wrapping_add(4));
    if !matches!(source_width, 1 | 2 | 4 | 8) {
        return;
    }
    if !matches!(dest_width, 1 | 2 | 4 | 8 | 16 | 32) {
        return;
    }

    let mut remaining = source_len;
    let mut in_byte = 0u8;
    let mut bits_remaining = 0u32;
    let mut out = 0u32;
    let mut bits_eaten = 0u32;

    while remaining > 0 || bits_remaining > 0 {
        if bits_remaining == 0 {
            if remaining == 0 {
                break;
            }
            in_byte = bus.read8(source);
            source = source.wrapping_add(1);
            remaining -= 1;
            bits_remaining = 8;
        }
        let mask = (1u32 << source_width) - 1;
        let mut scaled = u32::from(in_byte) & mask;
        in_byte >>= source_width as u8;
        bits_remaining -= source_width;
        if scaled != 0 || bias & 0x8000_0000 != 0 {
            scaled = scaled.wrapping_add(bias & 0x7FFF_FFFF);
        }
        out |= scaled << bits_eaten;
        bits_eaten += dest_width;
        if bits_eaten >= 32 {
            bus.write32(dest, out);
            bits_eaten = 0;
            out = 0;
            dest = dest.wrapping_add(4);
        }
    }
}

/// Once-per-number music / unsupported SWI warn line.
pub fn warn_swi(bus: &mut Bus, kind: &str, number: u32) {
    bus.warn_swi_once(number, kind);
}

#[cfg(test)]
mod tests {
    use super::{
        arctan, arctan2, bg_affine_set, bit_unpack, obj_affine_set, soft_reset, sound_bias,
        sqrt_u32,
    };
    use crate::bus::Bus;
    use crate::cpu::Cpu;

    #[test]
    fn sqrt_of_81_is_9() {
        assert_eq!(sqrt_u32(81), 9);
    }

    #[test]
    fn sqrt_zero_and_floor_two() {
        assert_eq!(sqrt_u32(0), 0);
        assert_eq!(sqrt_u32(2), 1);
    }

    #[test]
    fn divarm_swaps_operands() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new(vec![0; 0x200]);
        cpu.set_reg_for_test(0, 2);
        cpu.set_reg_for_test(1, 7);
        cpu.swi_number_for_test(&mut bus, 0x07);
        assert_eq!(cpu.reg(0) as i32, 3);
        assert_eq!(cpu.reg(1) as i32, 1);
        assert_eq!(cpu.reg(3), 3);
    }

    #[test]
    fn arctan_zero_is_zero_scale_pi4_is_0x4000() {
        // Scale: input 16.16 (1.0 = 0x10000) → angle with π/4 = 0x4000.
        assert_eq!(arctan(0), 0);
        assert!(arctan(0x10000) > 0);
        assert_eq!(arctan(0x10000), 0x4000);
    }

    #[test]
    fn arctan_i32_extremes_do_not_panic() {
        let _ = arctan(i32::MAX);
        let _ = arctan(i32::MIN);
        assert_eq!(arctan(0), 0);
        assert_eq!(arctan(0x10000), 0x4000);
    }

    #[test]
    fn arctan2_positive_x_zero_y_is_zero() {
        assert_eq!(arctan2(0x10000, 0), 0);
    }

    #[test]
    fn arctan2_min_over_neg_one_path_returns() {
        // y<<14 == i32::MIN; x == -1. wrapping_div avoids the i32::MIN / -1 panic.
        let _ = arctan2(-1, 0x2_0000);
        // Definite i32::MIN / -1 ratio on the x.wrapping_shl(14) / y path.
        let _ = arctan2(i32::MIN, -1);
    }

    #[test]
    fn soft_reset_jumps_to_rom_when_flag_bit0_clear() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new(vec![0; 0x200]);
        bus.write8(0x0300_7E00, 0xAB);
        bus.write8(0x0300_7FFA, 0);
        cpu.swi_number_for_test(&mut bus, 0x00);
        assert_eq!(cpu.fetch_pc, 0x0800_0000);
        assert_eq!(bus.read8(0x0300_7E00), 0);
        assert_eq!(bus.read8(0x0300_7FFA), 0);
    }

    #[test]
    fn soft_reset_jumps_to_ewram_when_flag_bit0_set() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new(vec![0; 0x200]);
        bus.write8(0x0300_7FFA, 1);
        cpu.swi_number_for_test(&mut bus, 0x00);
        assert_eq!(cpu.fetch_pc, 0x0200_0000);
        assert_eq!(bus.read8(0x0300_7FFA), 1);
    }

    #[test]
    fn register_ram_reset_clears_iwram_when_bit1_set() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new(vec![0; 0x200]);
        bus.write8(0x0300_0100, 0x55);
        cpu.set_reg_for_test(0, 1 << 1);
        cpu.swi_number_for_test(&mut bus, 0x01);
        assert_eq!(bus.read8(0x0300_0100), 0);
    }

    #[test]
    fn register_ram_reset_leaves_iwram_when_bit1_clear() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new(vec![0; 0x200]);
        bus.write8(0x0300_0100, 0x55);
        cpu.set_reg_for_test(0, 0);
        cpu.swi_number_for_test(&mut bus, 0x01);
        assert_eq!(bus.read8(0x0300_0100), 0x55);
    }

    #[test]
    fn bg_affine_set_identity_writes_0x100_matrix() {
        let mut bus = Bus::new(vec![0; 0x200]);
        // ox,oy=0; cx,cy=0; sx,sy=0x100; alpha=0
        bus.write32(0x0300_0000, 0);
        bus.write32(0x0300_0004, 0);
        bus.write16(0x0300_0008, 0);
        bus.write16(0x0300_000A, 0);
        bus.write16(0x0300_000C, 0x0100);
        bus.write16(0x0300_000E, 0x0100);
        bus.write16(0x0300_0010, 0);
        bg_affine_set(&mut bus, 0x0300_0000, 0x0300_0100, 1);
        assert_eq!(bus.read16(0x0300_0100), 0x0100);
        assert_eq!(bus.read16(0x0300_0102), 0);
        assert_eq!(bus.read16(0x0300_0104), 0);
        assert_eq!(bus.read16(0x0300_0106), 0x0100);
    }

    #[test]
    fn obj_affine_set_identity_writes_0x100_matrix() {
        let mut bus = Bus::new(vec![0; 0x200]);
        bus.write16(0x0300_0000, 0x0100);
        bus.write16(0x0300_0002, 0x0100);
        bus.write16(0x0300_0004, 0);
        obj_affine_set(&mut bus, 0x0300_0000, 0x0300_0100, 1, 2);
        assert_eq!(bus.read16(0x0300_0100), 0x0100);
        assert_eq!(bus.read16(0x0300_0102), 0);
        assert_eq!(bus.read16(0x0300_0104), 0);
        assert_eq!(bus.read16(0x0300_0106), 0x0100);
    }

    #[test]
    fn bit_unpack_len1_src1_dst8_eight_ones_from_0xff() {
        // Info: length=1, source width=1, dest width=8, offset=0.
        let mut bus = Bus::new(vec![0; 0x200]);
        bus.write8(0x0300_0000, 0xFF);
        bus.write16(0x0300_0010, 1);
        bus.write8(0x0300_0012, 1);
        bus.write8(0x0300_0013, 8);
        bus.write32(0x0300_0014, 0);
        bit_unpack(&mut bus, 0x0300_0000, 0x0300_0100, 0x0300_0010);
        for i in 0..8 {
            assert_eq!(bus.read8(0x0300_0100 + i), 1, "byte {i}");
        }
    }

    #[test]
    fn sound_bias_from_0x200_down_to_level_0() {
        let mut bus = Bus::new(vec![0; 0x200]);
        bus.write16(0x0400_0088, 0x0200);
        sound_bias(&mut bus, 0);
        assert_eq!(bus.read16(0x0400_0088), 0);
    }

    #[test]
    fn soft_reset_via_helper_preserves_flag_word() {
        let mut bus = Bus::new(vec![0; 0x200]);
        bus.write8(0x0300_7FFA, 0x43);
        bus.write8(0x0300_7E10, 0x99);
        let target = soft_reset(&mut bus);
        assert_eq!(target, 0x0200_0000);
        assert_eq!(bus.read8(0x0300_7E10), 0);
        assert_eq!(bus.read8(0x0300_7FFA), 0x43);
    }

    #[test]
    fn music_swi_warns_once_per_number() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new(vec![0; 0x200]);
        cpu.swi_number_for_test(&mut bus, 0x1A);
        cpu.swi_number_for_test(&mut bus, 0x1A);
        let music: Vec<_> = bus
            .warn_lines
            .iter()
            .filter(|l| l.contains("music unimplemented"))
            .collect();
        assert_eq!(music.len(), 1);
        assert_eq!(music[0], "gba-debug: warn swi music unimplemented n=1a");
    }

    #[test]
    fn multiboot_swi_warns_once() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new(vec![0; 0x200]);
        cpu.swi_number_for_test(&mut bus, 0x25);
        cpu.swi_number_for_test(&mut bus, 0x25);
        let lines: Vec<_> = bus
            .warn_lines
            .iter()
            .filter(|l| l.contains("unsupported"))
            .collect();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "gba-debug: warn swi unsupported n=25");
    }
}
