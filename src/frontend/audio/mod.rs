//! Pure audio output selection and PCM resampling (no device I/O).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioDeviceSource {
    Chosen,
    OsDefault,
    NamedFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioOutputChoice {
    Device {
        name: String,
        source: AudioDeviceSource,
    },
    Silent,
}

fn enumerated_contains(enumerated: &[&str], name: &str) -> bool {
    enumerated.contains(&name)
}

/// Pick an output device in priority order: chosen name, OS default name, named fallback, then silent.
/// Each candidate is used only when its name appears in `enumerated`.
pub fn choose_output(
    chosen: Option<&str>,
    os_default: Option<&str>,
    named_fallback: &str,
    enumerated: &[&str],
) -> AudioOutputChoice {
    if let Some(name) = chosen
        && enumerated_contains(enumerated, name)
    {
        return AudioOutputChoice::Device {
            name: name.to_string(),
            source: AudioDeviceSource::Chosen,
        };
    }

    if let Some(name) = os_default
        && enumerated_contains(enumerated, name)
    {
        return AudioOutputChoice::Device {
            name: name.to_string(),
            source: AudioDeviceSource::OsDefault,
        };
    }

    if enumerated_contains(enumerated, named_fallback) {
        return AudioOutputChoice::Device {
            name: named_fallback.to_string(),
            source: AudioDeviceSource::NamedFallback,
        };
    }

    AudioOutputChoice::Silent
}

fn lerp_i16(a: i16, b: i16, frac_num: u64, frac_den: u64) -> i16 {
    let a = a as i32;
    let b = b as i32;
    let delta = b - a;
    // Integer division truncates toward zero (Rust default).
    let offset = (delta as i64 * frac_num as i64 / frac_den as i64) as i32;
    (a + offset).clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

fn lerp_stereo(a: (i16, i16), b: (i16, i16), frac_num: u64, frac_den: u64) -> (i16, i16) {
    (
        lerp_i16(a.0, b.0, frac_num, frac_den),
        lerp_i16(a.1, b.1, frac_num, frac_den),
    )
}

/// Host frames owed for one GBA video frame.
///
/// A frame is 280896 cycles at 16.78 MHz (~59.7275 Hz). Truncating
/// `549 * host / 32768` emits 804 every frame, about 10 samples/sec more than
/// the device plays. The ring then sits on its ceiling and each dropped sample
/// is a tick. The accumulator makes the long-run total exact.
pub fn gba_host_samples(accum: &mut u64, host_rate: u32) -> usize {
    const FRAME_CYCLES: u64 = 280_896;
    const CPU_HZ: u64 = 16_777_216;
    *accum = accum.wrapping_add(FRAME_CYCLES.wrapping_mul(u64::from(host_rate)));
    let n = *accum / CPU_HZ;
    *accum %= CPU_HZ;
    n as usize
}

/// Continuous  mixer → host resampler.
///
/// Chopping each frame with [`resample_linear`] and keeping only the first 804
/// outputs drops the tail of that frame (~13 mixer samples). The next frame
/// starts later in the waveform, so the join clicks at 60 Hz. This carries the
/// unread sample and plays every input sample across exactly `out_len` outputs.
#[derive(Debug, Clone)]
pub struct PcmStream {
    pending: std::collections::VecDeque<(i16, i16)>,
    phase: f64,
}

impl Default for PcmStream {
    fn default() -> Self {
        Self::new()
    }
}

impl PcmStream {
    pub fn new() -> Self {
        Self {
            pending: std::collections::VecDeque::new(),
            phase: 0.0,
        }
    }

    pub fn emit(&mut self, incoming: &[(i16, i16)], out_len: usize) -> Vec<(i16, i16)> {
        self.pending.extend(incoming.iter().copied());
        if out_len == 0 {
            return Vec::new();
        }
        if self.pending.len() < 2 {
            let sample = self.pending.back().copied().unwrap_or((0, 0));
            return vec![sample; out_len];
        }
        let last = self.pending.len() - 1;
        let span = (last as f64 - self.phase).max(0.0);
        let step = if span > 0.0 {
            span / out_len as f64
        } else {
            0.0
        };
        let mut out = Vec::with_capacity(out_len);
        for _ in 0..out_len {
            let i = (self.phase as usize).min(last - 1);
            let t = (self.phase - i as f64).clamp(0.0, 1.0) as f32;
            let a = self.pending[i];
            let b = self.pending[i + 1];
            out.push((
                (f32::from(a.0) + (f32::from(b.0) - f32::from(a.0)) * t).round() as i16,
                (f32::from(a.1) + (f32::from(b.1) - f32::from(a.1)) * t).round() as i16,
            ));
            self.phase += step;
        }
        let drop = (self.phase as usize).min(self.pending.len().saturating_sub(1));
        self.pending.drain(..drop);
        self.phase -= drop as f64;
        if self.phase < 0.0 {
            self.phase = 0.0;
        }
        out
    }
}

/// Resample stereo PCM with linear interpolation. GBA mixer input rate is typically 32768 Hz.
pub fn resample_linear(input: &[(i16, i16)], in_rate: u32, out_rate: u32) -> Vec<(i16, i16)> {
    if in_rate == 0 || out_rate == 0 {
        return Vec::new();
    }
    if input.is_empty() {
        return Vec::new();
    }
    if in_rate == out_rate {
        return input.to_vec();
    }

    let out_len = ((input.len() as u64 * out_rate as u64) / in_rate as u64) as usize;
    let last = input.len() - 1;
    let mut out = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_pos = i as u64 * in_rate as u64;
        let idx = (src_pos / out_rate as u64) as usize;
        let idx = idx.min(last);
        let next = (idx + 1).min(last);
        let frac_num = src_pos % out_rate as u64;
        let frac_den = out_rate as u64;

        if frac_num == 0 {
            out.push(input[idx]);
        } else {
            out.push(lerp_stereo(input[idx], input[next], frac_num, frac_den));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choose_output_chosen_present_wins_even_when_not_os_default() {
        let enumerated = &["Speakers", "Headphones"];
        let choice = choose_output(Some("Headphones"), Some("Speakers"), "Fallback", enumerated);
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "Headphones".to_string(),
                source: AudioDeviceSource::Chosen,
            }
        );
    }

    #[test]
    fn choose_output_missing_chosen_falls_through_to_os_default_when_enumerated() {
        let enumerated = &["Speakers", "Headphones"];
        let choice = choose_output(None, Some("Speakers"), "Fallback", enumerated);
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "Speakers".to_string(),
                source: AudioDeviceSource::OsDefault,
            }
        );
    }

    #[test]
    fn choose_output_missing_chosen_and_os_default_uses_named_fallback_when_enumerated() {
        let enumerated = &["Fallback", "Other"];
        let choice = choose_output(None, None, "Fallback", enumerated);
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "Fallback".to_string(),
                source: AudioDeviceSource::NamedFallback,
            }
        );
    }

    #[test]
    fn choose_output_none_of_three_names_enumerated_returns_silent() {
        let enumerated = &["Unrelated A", "Unrelated B"];
        let choice = choose_output(Some("Chosen"), Some("Default"), "Fallback", enumerated);
        assert_eq!(choice, AudioOutputChoice::Silent);
    }

    #[test]
    fn pcm_stream_joins_frames_without_a_jump() {
        let mut stream = PcmStream::new();
        let mut src = Vec::new();
        for i in 0..549 * 2 {
            let v = ((i as f32) * 0.7) as i16;
            src.push((v, v));
        }
        let a = stream.emit(&src[..549], 804);
        let b = stream.emit(&src[549..], 804);
        assert_eq!(a.len(), 804);
        assert_eq!(b.len(), 804);
        let jump = (i32::from(b[0].0) - i32::from(a[803].0)).abs();
        assert!(
            jump < 30,
            "frame join jumped by {jump} (tail of the chunk was discarded)"
        );
        assert!(
            stream.pending.len() < 8,
            "unread backlog {} should stay at the overlap",
            stream.pending.len()
        );
    }

    #[test]
    fn resample_linear_at_32768_to_32768_copies_samples() {
        let input = vec![(100, -100), (200, -200), (300, -300)];
        let out = resample_linear(&input, 32_768, 32_768);
        assert_eq!(out, input);
    }

    #[test]
    fn gba_host_samples_track_device_rate() {
        // 804 samples every frame is ~10/sec fast and fills the ring until drops tick.
        let mut accum = 0u64;
        let frames = 10_000u64;
        let mut total = 0usize;
        for _ in 0..frames {
            total += gba_host_samples(&mut accum, 48_000);
        }
        let expect = (frames * 48_000 * 280_896 / 16_777_216) as usize;
        assert_eq!(total, expect);
        let naive = frames as usize * 804;
        assert!(
            total < naive,
            "exact rate {total} should be under truncate-804 {naive}"
        );
    }

    #[test]
    fn resample_linear_two_samples_32768_to_65536_length_and_middle_average() {
        let input = vec![(0, 0), (100, -200)];
        let out = resample_linear(&input, 32_768, 65_536);
        assert_eq!(out.len(), 4);
        assert_eq!(out[0], (0, 0));
        assert_eq!(out[1], (50, -100));
        assert_eq!(out[2], (100, -200));
        assert_eq!(out[3], (100, -200));
    }
}
