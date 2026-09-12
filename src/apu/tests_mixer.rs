//! G6-mixer — PCM/PSG mix, bias, PWM truncate, ratio-3→25%.
//!
//! Cited: jsgroth — GBA audio mixing (secondary)
//!   https://jsgroth.dev/blog/posts/gba-audio/
//! Cited: GBATEK — SOUNDBIAS / SOUNDCNT_H
//!   https://problemkaputt.de/gbatek.htm

use super::mixer::{apply_pwm_truncate, mix, to_i16_pcm};
use super::regs::{MASTER_ENABLE, OFF_SOUNDCNT_H, OFF_SOUNDCNT_L, OFF_SOUNDCNT_X};
use super::Apu;

#[test]
fn psg_ratio_3_behaves_like_25_percent() {
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_H, 0x0003);
    assert_eq!(apu.regs.psg_ratio_code(), 0);
}

#[test]
fn pwm_truncate_strips_lsbs() {
    assert_eq!(apply_pwm_truncate(0x3FF, 0), 0x3FE); // 9-bit
    assert_eq!(apply_pwm_truncate(0x3FF, 1), 0x3FC); // 8-bit
    assert_eq!(apply_pwm_truncate(0x3FF, 2), 0x3F8); // 7-bit
    assert_eq!(apply_pwm_truncate(0x3FF, 3), 0x3F0); // 6-bit
}

#[test]
fn bias_centers_silence() {
    let apu = Apu::new();
    let m = mix(&apu.regs, &apu.psg, &apu.fifos);
    // master off → bias only
    assert_eq!(m.left, 0x200);
    assert_eq!(m.right, 0x200);
}

#[test]
fn fifo_full_volume_scales() {
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE);
    apu.write16(OFF_SOUNDCNT_H, (1 << 2) | (1 << 8) | (1 << 9)); // A 100%, L+R
    apu.fifos.latch_a = 0x40; // 64
    let m = mix(&apu.regs, &apu.psg, &apu.fifos);
    // pcm = 64 << 2 = 256; + bias 0x200 = 0x300 before clamp
    assert!(m.left > 0x200);
    assert_eq!(m.left, m.right);
}

#[test]
fn to_i16_pcm_centers_zero_at_bias() {
    assert_eq!(to_i16_pcm(0x200, 0x200), 0);
    assert!(to_i16_pcm(0x300, 0x200) > 0);
    assert!(to_i16_pcm(0x100, 0x200) < 0);
}

#[test]
fn to_i16_centers_on_programmed_bias_not_hardcoded() {
    // Silence at a non-default bias must center to 0 (else DC rumble).
    assert_eq!(to_i16_pcm(0x100, 0x100), 0);
    assert_eq!(to_i16_pcm(0x180, 0x100).signum(), 1);
}

#[test]
fn fifo_full_scale_leaves_host_headroom() {
    // Direct Sound at 100% spans nearly the full signed 10-bit mix range.
    // Mapping that 1:1 onto i16 (<<6) rails the host DAC — aliased PCM then
    // reads as a loud saw on top of the music (FireRed after FIFO #25).
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE);
    apu.write16(OFF_SOUNDCNT_H, (1 << 2) | (1 << 8) | (1 << 9));
    apu.fifos.latch_a = 127;
    let m = mix(&apu.regs, &apu.psg, &apu.fifos);
    let pcm = to_i16_pcm(m.left, apu.regs.bias_level());
    assert!(
        pcm.unsigned_abs() < 30_000,
        "full-scale FIFO must keep host headroom, got {pcm}"
    );
}

#[test]
fn soundcnt_l_routes_psg() {
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE);
    apu.write16(OFF_SOUNDCNT_L, 0); // no stereo enables
                                    // Without stereo enable bits, PSG contrib is 0.
    let m = mix(&apu.regs, &apu.psg, &apu.fifos);
    assert_eq!(m.left, apply_pwm_truncate(0x200, 0));
}
