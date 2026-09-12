//! G6-pcm — host PCM ring + soft WAV encode + soft RMS.
//!
//! Cited: graycart-gba test strategy §7
//!   Project store: `docs/graycart-gba/07-test-strategy.md`

use super::pcm::{encode_wav_s16le, soft_rms, PcmBuffer, PcmFrame};
use super::regs::{MASTER_ENABLE, OFF_SOUNDCNT_H, OFF_SOUNDCNT_X};
use super::Apu;

#[test]
fn pull_samples_drains_ring() {
    let mut buf = PcmBuffer::new();
    buf.push(PcmFrame { left: 1, right: 2 });
    buf.push(PcmFrame { left: 3, right: 4 });
    let mut out = [PcmFrame::default(); 4];
    assert_eq!(buf.pull(&mut out), 2);
    assert_eq!(out[0].left, 1);
    assert_eq!(out[1].right, 4);
    assert_eq!(buf.len(), 0);
}

#[test]
fn encode_wav_has_riff_header() {
    let frames = [PcmFrame {
        left: 0x100,
        right: -0x100,
    }; 8];
    let wav = encode_wav_s16le(&frames, 32768);
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    assert!(wav.len() >= 44 + 8 * 4);
}

#[test]
fn apu_step_emits_pcm_at_pwm_rate() {
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE);
    apu.write16(OFF_SOUNDCNT_H, 0);
    // Default PWM 32.768 kHz → period 512 cycles
    apu.step(512 * 10);
    assert!(apu.pcm.total_pushed >= 10);
    let mut out = [PcmFrame::default(); 16];
    let n = apu.pull_samples(&mut out);
    assert!(n >= 10);
}

#[test]
fn soft_rms_silence_near_zero() {
    let frames = [PcmFrame::default(); 32];
    assert!(soft_rms(&frames) < 1.0);
}

#[test]
fn soft_wav_bytes_nonempty_after_step() {
    let mut apu = Apu::new();
    apu.step(512 * 4);
    let wav = apu.soft_wav_bytes();
    assert!(wav.len() > 44);
    assert_eq!(&wav[0..4], b"RIFF");
}
