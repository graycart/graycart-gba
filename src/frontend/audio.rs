//! Host audio — drain core PCM into a cpal output stream.
//!
//! Cited: graycart-gb `src/frontend/audio` posture (simplified ring)
//!   https://github.com/graycart/graycart-gb/tree/main/src/frontend/audio
//! Cited: cpal 0.15 — https://docs.rs/cpal/0.15.3
//! Note: device open may fail in headless CI; callers treat that as soft-fail.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use graycart_gba::apu::PcmFrame;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Interleaved stereo f32 ring shared with the cpal callback.
type SampleRing = Arc<Mutex<VecDeque<f32>>>;

/// Live host output (stream kept alive for the app lifetime).
pub struct AudioOut {
    _stream: Stream,
    ring: SampleRing,
    pub sample_rate: u32,
}

impl AudioOut {
    /// Open the default output device. Returns an explicit error string on failure.
    pub fn open_default() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "no default audio output device".to_string())?;
        let supported = device
            .default_output_config()
            .map_err(|e| format!("default_output_config: {e}"))?;
        let sample_rate = supported.sample_rate().0;
        let channels = supported.channels();
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let ring: SampleRing = Arc::new(Mutex::new(VecDeque::with_capacity(8192)));
        let ring_cb = Arc::clone(&ring);

        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(&device, &config, channels, ring_cb)?,
            SampleFormat::I16 => build_stream::<i16>(&device, &config, channels, ring_cb)?,
            SampleFormat::U16 => build_stream::<u16>(&device, &config, channels, ring_cb)?,
            other => return Err(format!("unsupported sample format: {other:?}")),
        };
        stream.play().map_err(|e| format!("stream.play: {e}"))?;
        Ok(Self {
            _stream: stream,
            ring,
            sample_rate,
        })
    }

    /// Push core PCM frames into the host ring (nearest-neighbor, no resample).
    pub fn push_frames(&self, frames: &[PcmFrame]) {
        let Ok(mut q) = self.ring.lock() else {
            return;
        };
        let interleaved = pcm_to_f32_interleaved(frames);
        for sample in interleaved {
            while q.len() > 32_000 {
                q.pop_front();
            }
            q.push_back(sample);
        }
    }
}

fn i16_to_f32(s: i16) -> f32 {
    f32::from(s) / 32768.0
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: u16,
    ring: SampleRing,
) -> Result<Stream, String>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    let ch = channels as usize;
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let mut q = ring.lock().unwrap_or_else(|e| e.into_inner());
                for frame in data.chunks_mut(ch) {
                    let (l, r) = if q.len() >= 2 {
                        (q.pop_front().unwrap_or(0.0), q.pop_front().unwrap_or(0.0))
                    } else {
                        (0.0, 0.0)
                    };
                    if !frame.is_empty() {
                        frame[0] = T::from_sample(l);
                    }
                    if frame.len() > 1 {
                        frame[1] = T::from_sample(r);
                    }
                    for s in frame.iter_mut().skip(2) {
                        *s = T::from_sample(0.0);
                    }
                }
            },
            |err| eprintln!("cpal stream error: {err}"),
            None,
        )
        .map_err(|e| format!("build_output_stream: {e}"))
}

/// Convert PCM frames to interleaved f32 (unit-tested without a device).
#[must_use]
pub fn pcm_to_f32_interleaved(frames: &[PcmFrame]) -> Vec<f32> {
    let mut out = Vec::with_capacity(frames.len() * 2);
    for f in frames {
        out.push(i16_to_f32(f.left));
        out.push(i16_to_f32(f.right));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_to_f32_interleaved_centers_zero() {
        let frames = [PcmFrame { left: 0, right: 0 }];
        let v = pcm_to_f32_interleaved(&frames);
        assert_eq!(v, vec![0.0, 0.0]);
    }

    #[test]
    fn pcm_to_f32_scales_full_scale() {
        let frames = [PcmFrame {
            left: i16::MAX,
            right: i16::MIN,
        }];
        let v = pcm_to_f32_interleaved(&frames);
        assert!(v[0] > 0.9);
        assert!(v[1] < -0.9);
    }
}
