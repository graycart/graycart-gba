//! G7-hle unit tests.
//!
//! Cited: GBATEK — BIOS Div / Sqrt / Affine / ArcTan / MidiKey2Freq / Decompression
//!   https://problemkaputt.de/gbatek.htm#biosfunctionsummary
//! Cross-check: mGBA `src/gba/bios.c` affine / MidiKey2Freq / ArcTan (secondary)
//! Note: synthetic payloads only — no commercial ROM data.

use super::hle::{self, swi};
use crate::bus::{CpuMem, FlatRam};
use crate::cpu::{soft_boot, Cpu};

#[test]
fn soft_boot_latch_constant() {
    let mut cpu = Cpu::new();
    let latch = hle::soft_boot_cart(&mut cpu);
    assert_eq!(latch, super::LATCH_SOFT_RESET);
    assert_eq!(cpu.regs.pc(), soft_boot::CART_ENTRY);
}

#[test]
fn sqrt_swi() {
    let mut cpu = Cpu::new();
    let mut mem = FlatRam::new(16);
    cpu.regs.set(0, 144);
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::SQRT),
        hle::SwiHleResult::Done
    ));
    assert_eq!(cpu.regs.get(0), 12);
    assert_eq!(
        hle::latch_after_handled_swi(swi::SQRT),
        super::LATCH_AFTER_SWI
    );
}

#[test]
fn div_swi() {
    let mut cpu = Cpu::new();
    let mut mem = FlatRam::new(16);
    cpu.regs.set(0, 100);
    cpu.regs.set(1, 7);
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::DIV),
        hle::SwiHleResult::Done
    ));
    assert_eq!(cpu.regs.get(0), 14);
    assert_eq!(cpu.regs.get(1), 2);
}

#[test]
fn cpuset_rejects_bios_source() {
    let mut cpu = Cpu::new();
    let mut mem = FlatRam::with_base(0, 0x100);
    cpu.regs.set(0, 0x100); // BIOS-range source
    cpu.regs.set(1, 0x80);
    cpu.regs.set(2, 4);
    mem.write32(0x80, 0xDEAD_BEEF);
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::CPU_SET),
        hle::SwiHleResult::Done
    ));
    // Destination unchanged because src < 0x4000.
    assert_eq!(mem.read32(0x80), 0xDEAD_BEEF);
}

/// Pack an all-literal LZ77 stream (type `10h`) for `plain`.
fn lz77_literals(plain: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let header = 0x10u32 | ((plain.len() as u32) << 8);
    out.extend_from_slice(&header.to_le_bytes());
    let mut i = 0;
    while i < plain.len() {
        let chunk = (plain.len() - i).min(8);
        out.push(0); // all uncompressed flags
        out.extend_from_slice(&plain[i..i + chunk]);
        i += chunk;
    }
    out
}

/// LZ77 with one back-reference: `"ABCDABCD"` via literals + copy.
fn lz77_with_backref() -> (Vec<u8>, Vec<u8>) {
    let plain = b"ABCDABCD".to_vec();
    // header + flag(0b1111_0000): 4 literals, then compressed block length=3 disp=4
    // After "ABCD", copy 4 bytes from dest-4 → "ABCD"
    let mut packed = Vec::new();
    packed.extend_from_slice(&(0x10u32 | (8u32 << 8)).to_le_bytes());
    // flags MSB-first: 0=lit,1=comp → four lits then one compressed (need 8 flags)
    // Use: lit lit lit lit comp pad pad pad → 0b0000_1000 = 0x08? Wait MSB first:
    // bit7 first: lit,lit,lit,lit,comp,lit?,… for 8 bytes we need lit×4 + comp(covers 4)
    // remaining after 4 lits = 4; compressed length = (n>>12)+3 so n>>12 = 1 → length 4
    // block = (1 << 12) | (disp-1) with disp=4 → 0x1003
    packed.push(0b0000_1000); // bits: 0,0,0,0,1,0,0,0 — only 5 ops but remaining hits 0 after comp
    packed.extend_from_slice(b"ABCD");
    packed.extend_from_slice(&0x1003u16.to_be_bytes()); // big-endian store as in GBATEK stream
                                                        // Actually mGBA reads: load8(source+1) | (load8(source)<<8) so first byte is high → BE
    (packed, plain)
}

#[test]
fn lz77_write8_swi_11_literals() {
    let plain = b"HelloGBA!";
    let packed = lz77_literals(plain);
    let mut mem = FlatRam::new(0x200);
    let src = 0x40u32;
    let dst = 0x100u32;
    for (i, b) in packed.iter().enumerate() {
        mem.write8(src + i as u32, *b);
    }
    let mut cpu = Cpu::new();
    cpu.regs.set(0, src);
    cpu.regs.set(1, dst);
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::LZ77_UNCOMP_WRITE8),
        hle::SwiHleResult::Done
    ));
    for (i, b) in plain.iter().enumerate() {
        assert_eq!(mem.read8(dst + i as u32), *b, "byte {i}");
    }
}

#[test]
fn lz77_write16_swi_12_literals_and_backref() {
    let (packed, plain) = lz77_with_backref();
    let mut mem = FlatRam::new(0x200);
    let src = 0x40u32;
    let dst = 0x100u32;
    for (i, b) in packed.iter().enumerate() {
        mem.write8(src + i as u32, *b);
    }
    let mut cpu = Cpu::new();
    cpu.regs.set(0, src);
    cpu.regs.set(1, dst);
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::LZ77_UNCOMP_WRITE16),
        hle::SwiHleResult::Done
    ));
    for (i, b) in plain.iter().enumerate() {
        assert_eq!(mem.read8(dst + i as u32), *b, "byte {i}");
    }
}

/// RL type `30h`: one compressed run of 5× `0xA5`, then two literals.
fn rl_sample() -> (Vec<u8>, Vec<u8>) {
    let plain = vec![0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0x11, 0x22];
    let mut packed = Vec::new();
    packed.extend_from_slice(&(0x30u32 | ((plain.len() as u32) << 8)).to_le_bytes());
    // compressed: bit7=1, len = (hdr&0x7F)+3 → hdr=0x80|(5-3)=0x82, then byte
    packed.push(0x82);
    packed.push(0xA5);
    // uncompressed: bit7=0, count = hdr+1 → hdr=1 for 2 bytes
    packed.push(0x01);
    packed.push(0x11);
    packed.push(0x22);
    (packed, plain)
}

#[test]
fn rl_write8_swi_14() {
    let (packed, plain) = rl_sample();
    let mut mem = FlatRam::new(0x200);
    let src = 0x40u32;
    let dst = 0x100u32;
    for (i, b) in packed.iter().enumerate() {
        mem.write8(src + i as u32, *b);
    }
    let mut cpu = Cpu::new();
    cpu.regs.set(0, src);
    cpu.regs.set(1, dst);
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::RL_UNCOMP_WRITE8),
        hle::SwiHleResult::Done
    ));
    for (i, b) in plain.iter().enumerate() {
        assert_eq!(mem.read8(dst + i as u32), *b, "byte {i}");
    }
}

#[test]
fn diff8_unfilter_swi_16() {
    // Diff8: store first byte, then each subsequent is cumulative sum.
    // Encoded: type 80h, size, then deltas [1, 2, 3, 4] → plain [1, 3, 6, 10]
    let mut packed = Vec::new();
    packed.extend_from_slice(&(0x80u32 | (4u32 << 8)).to_le_bytes());
    packed.extend_from_slice(&[1, 2, 3, 4]);
    let mut mem = FlatRam::new(0x100);
    let src = 0x20u32;
    let dst = 0x80u32;
    for (i, b) in packed.iter().enumerate() {
        mem.write8(src + i as u32, *b);
    }
    let mut cpu = Cpu::new();
    cpu.regs.set(0, src);
    cpu.regs.set(1, dst);
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::DIFF8_UNFILTER_WRITE8),
        hle::SwiHleResult::Done
    ));
    assert_eq!(mem.read8(dst), 1);
    assert_eq!(mem.read8(dst + 1), 3);
    assert_eq!(mem.read8(dst + 2), 6);
    assert_eq!(mem.read8(dst + 3), 10);
}

/// ObjAffineSet identity: scale 1.0 / angle 0 → PA=0x100, PB=0, PC=0, PD=0x100.
#[test]
fn obj_affine_set_swi_0f_identity() {
    let mut mem = FlatRam::new(0x100);
    let src = 0x20u32;
    let dst = 0x40u32;
    // sx, sy as 8.8; angle (full circle = 0x10000).
    mem.write16(src, 0x0100);
    mem.write16(src + 2, 0x0100);
    mem.write16(src + 4, 0);
    let mut cpu = Cpu::new();
    cpu.regs.set(0, src);
    cpu.regs.set(1, dst);
    cpu.regs.set(2, 1); // count
    cpu.regs.set(3, 2); // halfword stride (contiguous OAM matrix)
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::OBJ_AFFINE_SET),
        hle::SwiHleResult::Done
    ));
    assert_eq!(mem.read16(dst) as i16, 0x0100);
    assert_eq!(mem.read16(dst + 2) as i16, 0);
    assert_eq!(mem.read16(dst + 4) as i16, 0);
    assert_eq!(mem.read16(dst + 6) as i16, 0x0100);
}

/// ObjAffineSet 90° (angle high-byte 0x40): cos=0, sin=1 → A=0,B=-sx,C=sy,D=0.
#[test]
fn obj_affine_set_swi_0f_quarter_turn() {
    let mut mem = FlatRam::new(0x100);
    let src = 0x20u32;
    let dst = 0x60u32;
    mem.write16(src, 0x0100);
    mem.write16(src + 2, 0x0100);
    mem.write16(src + 4, 0x4000);
    let mut cpu = Cpu::new();
    cpu.regs.set(0, src);
    cpu.regs.set(1, dst);
    cpu.regs.set(2, 1);
    cpu.regs.set(3, 8); // OAM affine parameter spacing
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::OBJ_AFFINE_SET),
        hle::SwiHleResult::Done
    ));
    let a = mem.read16(dst) as i16;
    let b = mem.read16(dst + 8) as i16;
    let c = mem.read16(dst + 16) as i16;
    let d = mem.read16(dst + 24) as i16;
    assert!(a.abs() <= 1, "A≈0 got {a}");
    assert!((b - (-0x0100)).abs() <= 1, "B≈-0x100 got {b}");
    assert!((c - 0x0100).abs() <= 1, "C≈0x100 got {c}");
    assert!(d.abs() <= 1, "D≈0 got {d}");
}

#[test]
fn bg_affine_set_swi_0e_identity() {
    let mut mem = FlatRam::new(0x100);
    let src = 0x10u32;
    let dst = 0x40u32;
    // ox, oy (26.8), cx, cy, sx, sy, angle
    mem.write32(src, 0); // ox
    mem.write32(src + 4, 0); // oy
    mem.write16(src + 8, 0); // cx
    mem.write16(src + 10, 0); // cy
    mem.write16(src + 12, 0x0100); // sx
    mem.write16(src + 14, 0x0100); // sy
    mem.write16(src + 16, 0); // angle
    let mut cpu = Cpu::new();
    cpu.regs.set(0, src);
    cpu.regs.set(1, dst);
    cpu.regs.set(2, 1);
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::BG_AFFINE_SET),
        hle::SwiHleResult::Done
    ));
    assert_eq!(mem.read16(dst) as i16, 0x0100); // PA
    assert_eq!(mem.read16(dst + 2) as i16, 0); // PB
    assert_eq!(mem.read16(dst + 4) as i16, 0); // PC
    assert_eq!(mem.read16(dst + 6) as i16, 0x0100); // PD
    assert_eq!(mem.read32(dst + 8), 0); // x
    assert_eq!(mem.read32(dst + 12), 0); // y
}

#[test]
fn arctan_swi_09_zero() {
    let mut cpu = Cpu::new();
    let mut mem = FlatRam::new(16);
    cpu.regs.set(0, 0);
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::ARCTAN),
        hle::SwiHleResult::Done
    ));
    assert_eq!(cpu.regs.get(0) as i16, 0);
}

#[test]
fn arctan2_swi_0a_axes() {
    let mut cpu = Cpu::new();
    let mut mem = FlatRam::new(16);
    // +x → 0
    cpu.regs.set(0, 0x1000);
    cpu.regs.set(1, 0);
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::ARCTAN2),
        hle::SwiHleResult::Done
    ));
    assert_eq!(cpu.regs.get(0) as u16, 0);
    // +y → 0x4000
    cpu.regs.set(0, 0);
    cpu.regs.set(1, 0x1000);
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::ARCTAN2),
        hle::SwiHleResult::Done
    ));
    assert_eq!(cpu.regs.get(0) as u16, 0x4000);
}

/// MidiKey2Freq: key / 2^((180 - mk - fine/256)/12).
#[test]
fn midi_key2freq_swi_1f() {
    let mut mem = FlatRam::new(0x40);
    let wave = 0x10u32;
    // Frequency base at WaveData+4.
    mem.write32(wave + 4, 0x1_0000);
    let mut cpu = Cpu::new();
    cpu.regs.set(0, wave);
    cpu.regs.set(1, 60); // midi key
    cpu.regs.set(2, 0); // fine
    assert!(matches!(
        hle::try_swi(&mut cpu, &mut mem, swi::MIDI_KEY2FREQ),
        hle::SwiHleResult::Done
    ));
    // 2^((180-60)/12) = 2^10 = 1024 → 0x10000 / 1024 = 64
    assert_eq!(cpu.regs.get(0), 64);
}
