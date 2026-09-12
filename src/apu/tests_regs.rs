//! G6-regs — SOUNDCNT_* master enable + Wave RAM + FIFO ports.
//!
//! Cited: GBATEK — Sound Control Registers
//!   https://problemkaputt.de/gbatek.htm

use super::regs::{
    MASTER_ENABLE, OFF_FIFO_A, OFF_SOUND1CNT_L, OFF_SOUNDBIAS, OFF_SOUNDCNT_H, OFF_SOUNDCNT_L,
    OFF_SOUNDCNT_X, OFF_WAVE_RAM, SOUNDBIAS_DEFAULT,
};
use super::Apu;

#[test]
fn soundbias_defaults_to_0x200() {
    let apu = Apu::new();
    assert_eq!(apu.read16(OFF_SOUNDBIAS), SOUNDBIAS_DEFAULT);
}

#[test]
fn master_off_clears_psg_regs_keeps_h_and_bias() {
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE);
    apu.write16(OFF_SOUND1CNT_L, 0x0077);
    apu.write16(OFF_SOUNDCNT_L, 0x1177);
    apu.write16(OFF_SOUNDCNT_H, 0x0B0F); // also reset A
    apu.write16(OFF_SOUNDBIAS, 0x4200);

    assert_eq!(apu.read16(OFF_SOUND1CNT_L), 0x0077);
    // Master off
    apu.write16(OFF_SOUNDCNT_X, 0);
    assert_eq!(apu.read16(OFF_SOUND1CNT_L), 0);
    assert_eq!(apu.read16(OFF_SOUNDCNT_L), 0);
    // H + bias remain
    assert_eq!(apu.read16(OFF_SOUNDCNT_H) & 0x770F, 0x030F); // reset bits not sticky
    assert_eq!(apu.read16(OFF_SOUNDBIAS), 0x4200 & 0xC3FE);
}

#[test]
fn soundcnt_h_writable_while_master_off() {
    let mut apu = Apu::new();
    assert!(!apu.regs.master_enabled());
    apu.write16(OFF_SOUNDCNT_H, 0x330F);
    assert_eq!(apu.read16(OFF_SOUNDCNT_H), 0x330F);
}

#[test]
fn wave_ram_cpu_hits_opposite_bank() {
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE);
    // play bank 0 → CPU writes bank 1
    apu.write16(0x70, 0x0000); // SOUND3CNT_L bank=0
    apu.write16(OFF_WAVE_RAM, 0xBEEF);
    assert_eq!(apu.regs.wave_ram[1][0], 0xEF);
    assert_eq!(apu.regs.wave_ram[1][1], 0xBE);
    assert_eq!(apu.regs.wave_ram[0][0], 0);

    // play bank 1 → CPU writes bank 0
    apu.write16(0x70, 1 << 6);
    apu.write16(OFF_WAVE_RAM, 0x1234);
    assert_eq!(apu.regs.wave_ram[0][0], 0x34);
    assert_eq!(apu.regs.wave_ram[0][1], 0x12);
}

#[test]
fn fifo_a_32_push_lsb_first() {
    let mut apu = Apu::new();
    apu.write32(OFF_FIFO_A, 0x0403_0201);
    assert_eq!(apu.fifos.a.len(), 4);
    assert_eq!(apu.fifos.a.pop_sample(), 0x01);
    assert_eq!(apu.fifos.a.pop_sample(), 0x02);
    assert_eq!(apu.fifos.a.pop_sample(), 0x03);
    assert_eq!(apu.fifos.a.pop_sample(), 0x04);
}

#[test]
fn channel_on_flags_readonly_via_soundcnt_x() {
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE | 0x0F);
    // Writing bits 0–3 must not stick as writable enables
    let x = apu.read16(OFF_SOUNDCNT_X);
    assert_eq!(x & MASTER_ENABLE, MASTER_ENABLE);
    assert_eq!(x & 0x0F, 0); // no channel triggered yet
}
