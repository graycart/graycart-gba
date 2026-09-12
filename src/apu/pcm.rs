//! Host PCM ring + soft WAV helper (P6).
//!
//! Cited: graycart-gba test strategy §7 (audio soft gate)
//!   Project store: `docs/graycart-gba/07-test-strategy.md`
//! Research: Project store `docs/graycart-gba/04-apu.md` §8.2
//! Note: nearest-neighbor at PWM rate; soft WAV is RMS/length — not bit-exact CI.

use super::mixer::{to_i16_pcm, MixedSample};

/// Stereo PCM sample (host-facing).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PcmFrame {
    pub left: i16,
    pub right: i16,
}

impl From<MixedSample> for PcmFrame {
    fn from(m: MixedSample) -> Self {
        Self {
            left: to_i16_pcm(m.left),
            right: to_i16_pcm(m.right),
        }
    }
}

/// Power-of-two ring capacity (frames).
pub const PCM_CAPACITY: usize = 8192;

/// Simple stereo PCM ring buffer.
#[derive(Debug, Clone)]
pub struct PcmBuffer {
    buf: [PcmFrame; PCM_CAPACITY],
    head: usize,
    len: usize,
    /// Total frames ever pushed (for soft-gate length checks).
    pub total_pushed: u64,
}

impl Default for PcmBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl PcmBuffer {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buf: [PcmFrame { left: 0, right: 0 }; PCM_CAPACITY],
            head: 0,
            len: 0,
            total_pushed: 0,
        }
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
        self.total_pushed = 0;
    }

    pub fn push(&mut self, frame: PcmFrame) {
        let idx = (self.head + self.len) % PCM_CAPACITY;
        if self.len < PCM_CAPACITY {
            self.len += 1;
        } else {
            // Drop oldest
            self.head = (self.head + 1) % PCM_CAPACITY;
        }
        self.buf[idx] = frame;
        self.total_pushed = self.total_pushed.wrapping_add(1);
    }

    /// Pull up to `out.len()` frames; returns count written.
    pub fn pull(&mut self, out: &mut [PcmFrame]) -> usize {
        let n = out.len().min(self.len);
        for (dst, _) in out.iter_mut().zip(0..n) {
            *dst = self.buf[self.head];
            self.head = (self.head + 1) % PCM_CAPACITY;
            self.len -= 1;
        }
        n
    }

    /// Peek without consuming (for soft RMS).
    #[must_use]
    pub fn snapshot(&self) -> Vec<PcmFrame> {
        let mut v = Vec::with_capacity(self.len);
        for i in 0..self.len {
            v.push(self.buf[(self.head + i) % PCM_CAPACITY]);
        }
        v
    }
}

/// Soft RMS of interleaved L/R as f64 (for soft WAV gate).
#[must_use]
pub fn soft_rms(frames: &[PcmFrame]) -> f64 {
    if frames.is_empty() {
        return 0.0;
    }
    let mut acc = 0.0f64;
    let mut n = 0u64;
    for f in frames {
        let l = f64::from(f.left);
        let r = f64::from(f.right);
        acc += l * l + r * r;
        n += 2;
    }
    (acc / n as f64).sqrt()
}

/// Write a minimal 16-bit stereo WAV (PCM) to bytes.
#[must_use]
pub fn encode_wav_s16le(frames: &[PcmFrame], sample_rate: u32) -> Vec<u8> {
    let data_len = frames.len() * 4; // 2 ch × 2 bytes
    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&2u16.to_le_bytes()); // channels
    out.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate * 4;
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    for f in frames {
        out.extend_from_slice(&f.left.to_le_bytes());
        out.extend_from_slice(&f.right.to_le_bytes());
    }
    out
}
