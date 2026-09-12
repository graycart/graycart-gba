//! G6-psg — channel triggers, wave bank sample, noise/square digital out.
//!
//! Cited: GBATEK — Sound Channel 1–4
//!   https://problemkaputt.de/gbatek.htm

use super::regs::{
    MASTER_ENABLE, OFF_SOUND1CNT_H, OFF_SOUND1CNT_L, OFF_SOUND1CNT_X, OFF_SOUND2CNT_H,
    OFF_SOUND2CNT_L, OFF_SOUND3CNT_H, OFF_SOUND3CNT_L, OFF_SOUND3CNT_X, OFF_SOUND4CNT_H,
    OFF_SOUND4CNT_L, OFF_SOUNDCNT_L, OFF_SOUNDCNT_X, OFF_WAVE_RAM,
};
use super::Apu;

#[test]
fn trigger_ch2_sets_channel_on_flag() {
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE);
    apu.write16(OFF_SOUND2CNT_L, 0xF080); // vol 15, duty 50%, length
    apu.write16(OFF_SOUND2CNT_H, 0x8000 | 0x700); // trigger + freq
    assert!(apu.psg.ch2.enabled);
    apu.psg.step(1, &mut apu.regs);
    assert_ne!(apu.read16(OFF_SOUNDCNT_X) & 0x02, 0);
}

#[test]
fn trigger_ch1_sweep_enables() {
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE);
    apu.write16(OFF_SOUND1CNT_L, 0x0077);
    apu.write16(OFF_SOUND1CNT_H, 0xF080);
    apu.write16(OFF_SOUND1CNT_X, 0x8000 | 0x500);
    assert!(apu.psg.ch1.enabled);
}

#[test]
fn wave_channel_reads_play_bank_nibble() {
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE);
    // play bank 0; CPU writes bank 1 — so put pattern in bank 0 via bank flip
    apu.write16(OFF_SOUND3CNT_L, 1 << 6); // play=1 → CPU bank 0
    apu.write16(OFF_WAVE_RAM, 0x00F0); // first nibble high = 0xF when play bank 0... wait
                                       // Switch play to 0 so bank 0 is play (CPU wrote bank 0 while play was 1)
    apu.write16(OFF_SOUND3CNT_L, 1 << 7); // dac on, play bank 0
    apu.write16(OFF_SOUND3CNT_H, (1 << 13) | 0x0100); // 100% vol code 1
    apu.write16(OFF_SOUND3CNT_X, 0x8000 | 0x600);
    assert!(apu.psg.ch3.enabled);
    let s = apu.psg.sample_ch3(&apu.regs);
    // first sample high nibble of byte0 = 0xF → 2*15-15 = 15
    assert_eq!(s, 15);
}

#[test]
fn noise_trigger_enables_lfsr() {
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE);
    apu.write16(OFF_SOUND4CNT_L, 0xF000);
    apu.write16(OFF_SOUND4CNT_H, 0x8000);
    assert!(apu.psg.ch4.enabled);
}

#[test]
fn master_off_silences_psg_step() {
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE);
    apu.write16(OFF_SOUND2CNT_L, 0xF080);
    apu.write16(OFF_SOUND2CNT_H, 0x87FF);
    apu.write16(OFF_SOUNDCNT_L, 0x2202); // ch2 both ears, vol
    apu.write16(OFF_SOUNDCNT_X, 0); // off
    apu.step(10_000);
    assert!(!apu.psg.ch2.enabled);
}
