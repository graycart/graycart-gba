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
    if let Some(name) = chosen {
        if enumerated_contains(enumerated, name) {
            return AudioOutputChoice::Device {
                name: name.to_string(),
                source: AudioDeviceSource::Chosen,
            };
        }
    }

    if let Some(name) = os_default {
        if enumerated_contains(enumerated, name) {
            return AudioOutputChoice::Device {
                name: name.to_string(),
                source: AudioDeviceSource::OsDefault,
            };
        }
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
    fn resample_linear_at_32768_to_32768_copies_samples() {
        let input = vec![(100, -100), (200, -200), (300, -300)];
        let out = resample_linear(&input, 32_768, 32_768);
        assert_eq!(out, input);
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
