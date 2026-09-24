//! Output-device selection: delegates to [`crate::frontend::choose_output`].

use crate::frontend::audio::{
    AudioDeviceSource as LibSource, AudioOutputChoice as LibChoice, choose_output,
};

/// Persisted output preference. Fallback devices are never stored here.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioDevicePref {
    #[default]
    SystemDefault,
    Device {
        name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioDeviceSource {
    User,
    SystemDefault,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioOutputChoice {
    Device {
        name: String,
        source: AudioDeviceSource,
    },
    Offline,
}

/// Pure selection (no CPAL). Order matches [`crate::frontend::choose_output`]:
/// chosen name if enumerated, else OS default if enumerated, else `"Speakers"`
/// if enumerated, else silent. Never falls through to the first enumerated device.
pub fn choose_output_device(
    pref: &AudioDevicePref,
    cpal_default: Option<&str>,
    os_defaults: &[&str],
    enumerated: &[&str],
) -> AudioOutputChoice {
    let chosen = match pref {
        AudioDevicePref::Device { name } => Some(name.as_str()),
        AudioDevicePref::SystemDefault => None,
    };
    let os_default = os_defaults
        .iter()
        .copied()
        .find(|n| enumerated.contains(n))
        .or(cpal_default);
    match choose_output(chosen, os_default, "Speakers", enumerated) {
        LibChoice::Silent => AudioOutputChoice::Offline,
        LibChoice::Device { name, source } => AudioOutputChoice::Device {
            name,
            source: match source {
                LibSource::Chosen => AudioDeviceSource::User,
                LibSource::OsDefault => AudioDeviceSource::SystemDefault,
                LibSource::NamedFallback => AudioDeviceSource::Fallback,
            },
        },
    }
}

pub fn should_persist_choice(source: AudioDeviceSource) -> bool {
    matches!(source, AudioDeviceSource::User)
}

/// Menu listing must not re-enumerate WASAPI/CPAL every egui frame.
pub fn should_refresh_output_device_list(
    age: Option<std::time::Duration>,
    ttl: std::time::Duration,
) -> bool {
    age.is_none_or(|a| a >= ttl)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_default_prefers_os_endpoint_not_first_enumerated() {
        let choice = choose_output_device(
            &AudioDevicePref::SystemDefault,
            None,
            &["Speakers (Realtek)"],
            &["Digital Output (Sound Blaster X4)", "Speakers (Realtek)"],
        );
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "Speakers (Realtek)".into(),
                source: AudioDeviceSource::SystemDefault,
            }
        );
    }

    #[test]
    fn system_default_uses_cpal_default_over_enumeration_order() {
        let choice = choose_output_device(
            &AudioDevicePref::SystemDefault,
            Some("Speakers (Realtek)"),
            &[],
            &["Digital Output (Sound Blaster X4)", "Speakers (Realtek)"],
        );
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "Speakers (Realtek)".into(),
                source: AudioDeviceSource::SystemDefault,
            }
        );
    }

    #[test]
    fn missing_defaults_use_speakers_literal_when_enumerated() {
        let choice = choose_output_device(
            &AudioDevicePref::SystemDefault,
            None,
            &[],
            &["Digital Output (Sound Blaster X4)", "Speakers"],
        );
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "Speakers".into(),
                source: AudioDeviceSource::Fallback,
            }
        );
        assert!(!should_persist_choice(AudioDeviceSource::Fallback));
    }

    #[test]
    fn missing_defaults_without_speakers_is_offline() {
        let choice = choose_output_device(
            &AudioDevicePref::SystemDefault,
            None,
            &[],
            &["Digital Output (Sound Blaster X4)"],
        );
        assert_eq!(choice, AudioOutputChoice::Offline);
    }

    #[test]
    fn user_named_device_wins_when_present() {
        let pref = AudioDevicePref::Device {
            name: "Digital Output (Sound Blaster X4)".into(),
        };
        let choice = choose_output_device(
            &pref,
            Some("Speakers (Realtek)"),
            &["Speakers (Realtek)"],
            &["Digital Output (Sound Blaster X4)", "Speakers (Realtek)"],
        );
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "Digital Output (Sound Blaster X4)".into(),
                source: AudioDeviceSource::User,
            }
        );
        assert!(should_persist_choice(AudioDeviceSource::User));
    }

    #[test]
    fn missing_user_device_falls_back_to_system_default() {
        let pref = AudioDevicePref::Device {
            name: "Gone Device".into(),
        };
        let choice = choose_output_device(
            &pref,
            Some("Speakers (Realtek)"),
            &[],
            &["Speakers (Realtek)", "Headphones"],
        );
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "Speakers (Realtek)".into(),
                source: AudioDeviceSource::SystemDefault,
            }
        );
    }

    #[test]
    fn empty_inventory_is_offline() {
        assert_eq!(
            choose_output_device(&AudioDevicePref::SystemDefault, None, &[], &[]),
            AudioOutputChoice::Offline
        );
    }

    #[test]
    fn linux_pulse_bridge_beats_first_alsa_hw_card() {
        let choice = choose_output_device(
            &AudioDevicePref::SystemDefault,
            Some("default"),
            &[
                "alsa_output.pci-0000_00_1f.3.analog-stereo",
                "pulse",
                "pipewire",
            ],
            &["hw:0,0", "default", "pulse", "pipewire"],
        );
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "pulse".into(),
                source: AudioDeviceSource::SystemDefault,
            }
        );
    }

    #[test]
    fn macos_coreaudio_name_beats_first_enumerated() {
        let choice = choose_output_device(
            &AudioDevicePref::SystemDefault,
            None,
            &["MacBook Pro Speakers"],
            &["USB Audio Device", "MacBook Pro Speakers"],
        );
        assert_eq!(
            choice,
            AudioOutputChoice::Device {
                name: "MacBook Pro Speakers".into(),
                source: AudioDeviceSource::SystemDefault,
            }
        );
    }

    #[test]
    fn device_list_refresh_only_when_missing_or_ttl_elapsed() {
        let ttl = std::time::Duration::from_secs(2);
        assert!(should_refresh_output_device_list(None, ttl));
        assert!(!should_refresh_output_device_list(
            Some(std::time::Duration::from_millis(10)),
            ttl
        ));
        assert!(should_refresh_output_device_list(
            Some(std::time::Duration::from_secs(2)),
            ttl
        ));
    }
}
