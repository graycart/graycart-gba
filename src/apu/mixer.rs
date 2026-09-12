//! Digital mixer + SOUNDBIAS / PWM truncate (P6).
//!
//! Cited: GBATEK — Sound Control / SOUNDBIAS
//!   https://problemkaputt.de/gbatek.htm
//! Cited: jsgroth — GBA audio mixing math (secondary)
//!   https://jsgroth.dev/blog/posts/gba-audio/
//! Research: Project store `docs/graycart-gba/04-apu.md` §7–§8

use super::fifo::FifoPair;
use super::psg::Psg;
use super::regs::ApuRegs;

/// Stereo sample after mix (pre-host), unsigned 10-bit per channel then shifted to i16 PCM.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MixedSample {
    pub left: u16,
    pub right: u16,
}

/// Mix FIFO + PSG for one ear pair.
#[must_use]
pub fn mix(regs: &ApuRegs, psg: &Psg, fifos: &FifoPair) -> MixedSample {
    if !regs.master_enabled() {
        let bias = regs.bias_level().clamp(0, 0x3FF) as u16;
        return MixedSample {
            left: bias,
            right: bias,
        };
    }

    let left = mix_ear(regs, psg, fifos, true);
    let right = mix_ear(regs, psg, fifos, false);
    MixedSample { left, right }
}

fn mix_ear(regs: &ApuRegs, psg: &Psg, fifos: &FifoPair, left: bool) -> u16 {
    // --- PCM path ---
    let mut pcm: i32 = 0;
    let a = i32::from(fifos.latch_a);
    let b = i32::from(fifos.latch_b);
    let route_a = if left {
        regs.fifo_a_left()
    } else {
        regs.fifo_a_right()
    };
    let route_b = if left {
        regs.fifo_b_left()
    } else {
        regs.fifo_b_right()
    };
    if route_a {
        let shift = if regs.fifo_a_full_volume() { 2 } else { 1 };
        pcm += a << shift;
    }
    if route_b {
        let shift = if regs.fifo_b_full_volume() { 2 } else { 1 };
        pcm += b << shift;
    }
    pcm = pcm.clamp(-0x200, 0x1FF);

    // --- PSG path ---
    let nr51 = regs.soundcnt_l;
    let enable = |ch: u8| -> bool {
        let bit = if left { 12 + ch } else { 8 + ch };
        nr51 & (1 << bit) != 0
    };
    let mut psg_sum = 0i32;
    if enable(0) {
        psg_sum += psg.sample_ch1();
    }
    if enable(1) {
        psg_sum += psg.sample_ch2();
    }
    if enable(2) {
        psg_sum += psg.sample_ch3(regs);
    }
    if enable(3) {
        psg_sum += psg.sample_ch4();
    }
    let master_vol = if left {
        ((nr51 >> 4) & 7) + 1
    } else {
        (nr51 & 7) + 1
    };
    psg_sum *= i32::from(master_vol);
    let ratio = regs.psg_ratio_code();
    // >>= (2 - ratio): 0→>>2 (25%), 1→>>1 (50%), 2→>>0 (100%)
    psg_sum >>= 2 - ratio as i32;

    let mut signed = (pcm + psg_sum).clamp(-0x200, 0x1FF);
    signed += regs.bias_level();
    let out = signed.clamp(0, 0x3FF) as u16;
    apply_pwm_truncate(out, regs.pwm_resolution())
}

/// Truncate LSBs per SOUNDBIAS PWM resolution (not round).
#[must_use]
pub fn apply_pwm_truncate(sample10: u16, res: u16) -> u16 {
    let shift = match res & 0b11 {
        0 => 1, // 9-bit
        1 => 2, // 8-bit
        2 => 3, // 7-bit
        _ => 4, // 6-bit
    };
    (sample10 >> shift) << shift
}

/// PWM output rate in Hz for resolution code.
#[must_use]
pub fn pwm_rate_hz(res: u16) -> u32 {
    match res & 0b11 {
        0 => 32768,
        1 => 65536,
        2 => 131072,
        _ => 262144,
    }
}

/// System clocks per PWM sample period.
#[must_use]
pub fn pwm_period_cycles(res: u16) -> u32 {
    // 2^24 / rate
    16_777_216 / pwm_rate_hz(res)
}

/// Convert unsigned 10-bit (biased) to signed 16-bit PCM centered on `bias`.
///
/// Uses `<< 5` (½ of full-scale `<< 6`) so a max Direct Sound peak keeps host
/// DAC headroom. Full `<< 6` rails i16 on ordinary FIFO peaks; the stair-step
/// aliasing then reads as a loud saw on top of the music (FireRed after #25).
/// Matches MAME's ~0.5 FIFO DAC route gain posture for host playback.
#[must_use]
pub fn to_i16_pcm(sample10: u16, bias: i32) -> i16 {
    let centered = i32::from(sample10) - bias;
    (centered << 5).clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}
