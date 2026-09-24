//! Persistent frontend settings (palette, audio, display, input). Not machine state.

use super::audio::AudioDevicePref;
use super::host_input::{GamepadMap, HostCommandMap, KeyboardMap};
use super::playback::SpeedPreset;
use super::rom::RecentRom;
use super::video::{DisplayMode, Palette, PalettePreset};
use graycart::HostHardwarePref;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

fn default_deadzone() -> f32 {
    0.25
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputSettings {
    #[serde(default)]
    pub keyboard: KeyboardMap,
    #[serde(default)]
    pub gamepads: Vec<GamepadProfile>,
    #[serde(default)]
    pub preferred_controller_profile_id: Option<String>,
    #[serde(default = "default_deadzone")]
    pub stick_deadzone: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamepadProfile {
    pub controller_profile_id: String,
    pub name: String,
    #[serde(default)]
    pub map: GamepadMap,
}

impl Default for InputSettings {
    fn default() -> Self {
        Self {
            keyboard: KeyboardMap::default(),
            gamepads: Vec::new(),
            preferred_controller_profile_id: None,
            stick_deadzone: default_deadzone(),
        }
    }
}

impl InputSettings {
    pub fn clamp_deadzone(&mut self) {
        self.stick_deadzone = self.stick_deadzone.clamp(0.05, 0.95);
    }

    pub fn upsert_profile(&mut self, id: String, name: String) -> &mut GamepadProfile {
        if let Some(idx) = self
            .gamepads
            .iter()
            .position(|p| p.controller_profile_id == id)
        {
            self.gamepads[idx].name = name;
            return &mut self.gamepads[idx];
        }
        self.gamepads.push(GamepadProfile {
            controller_profile_id: id,
            name,
            map: GamepadMap::default(),
        });
        let idx = self.gamepads.len() - 1;
        &mut self.gamepads[idx]
    }

    pub fn profile_mut(&mut self, id: &str) -> Option<&mut GamepadProfile> {
        self.gamepads
            .iter_mut()
            .find(|p| p.controller_profile_id == id)
    }

    pub fn restore_defaults(&mut self, active_profile_id: Option<&str>) {
        self.keyboard.restore_defaults();
        self.stick_deadzone = default_deadzone();
        if let Some(id) = active_profile_id
            && let Some(profile) = self.profile_mut(id)
        {
            profile.map.restore_defaults();
        }
    }
}

const MAX_RECENTS: usize = 12;

fn default_volume_percent() -> u8 {
    10
}

fn default_ff_speed() -> SpeedPreset {
    SpeedPreset::X4
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "FrontendSettingsDe")]
pub struct FrontendSettings {
    pub palette: PalettePreset,
    pub custom_colors: [[u8; 3]; 4],
    pub display_mode: DisplayMode,
    pub integer_scaling: bool,
    #[serde(default = "default_volume_percent")]
    pub volume_percent: u8,
    pub muted: bool,
    #[serde(default)]
    pub skip_boot: bool,
    #[serde(default)]
    pub recent: Vec<RecentRom>,
    pub last_rom_dir: Option<PathBuf>,
    #[serde(default)]
    pub monitor: MonitorSettings,
    #[serde(default)]
    pub input: InputSettings,
    #[serde(default)]
    pub host_commands: HostCommandMap,
    #[serde(default)]
    pub rewind_enabled: bool,
    #[serde(default = "default_ff_speed")]
    pub ff_speed: SpeedPreset,
    #[serde(default)]
    pub pause_when_unfocused: bool,
    #[serde(default)]
    pub hardware_pref: HostHardwarePref,
    #[serde(default)]
    pub audio_output: AudioDevicePref,
    /// User fine-grained GitHub PAT for in-app issue filing (Issues write on graycart-gb).
    #[serde(default)]
    pub github_pat: String,
}

#[derive(Debug, Deserialize)]
struct FrontendSettingsDe {
    palette: PalettePreset,
    custom_colors: [[u8; 3]; 4],
    display_mode: DisplayMode,
    integer_scaling: bool,
    volume_percent: Option<u8>,
    volume: Option<f32>,
    muted: bool,
    #[serde(default, alias = "skip_hle_boot")]
    skip_boot: bool,
    #[serde(default)]
    recent: Vec<RecentRom>,
    last_rom_dir: Option<PathBuf>,
    #[serde(default)]
    monitor: MonitorSettings,
    #[serde(default)]
    input: InputSettings,
    #[serde(default)]
    host_commands: HostCommandMap,
    #[serde(default)]
    rewind_enabled: bool,
    #[serde(default = "default_ff_speed")]
    ff_speed: SpeedPreset,
    #[serde(default)]
    pause_when_unfocused: bool,
    #[serde(default)]
    hardware_pref: HostHardwarePref,
    #[serde(default)]
    audio_output: AudioDevicePref,
    #[serde(default)]
    github_pat: String,
}

impl From<FrontendSettingsDe> for FrontendSettings {
    fn from(de: FrontendSettingsDe) -> Self {
        let volume_percent = match de.volume_percent {
            Some(p) => p.min(100),
            None => de
                .volume
                .map(|v| ((v * 100.0).round() as i32).clamp(0, 100) as u8)
                .unwrap_or_else(default_volume_percent),
        };
        Self {
            palette: de.palette,
            custom_colors: de.custom_colors,
            display_mode: de.display_mode,
            integer_scaling: de.integer_scaling,
            volume_percent,
            muted: de.muted,
            skip_boot: de.skip_boot,
            recent: de.recent,
            last_rom_dir: de.last_rom_dir,
            monitor: de.monitor,
            input: de.input,
            host_commands: de.host_commands,
            rewind_enabled: de.rewind_enabled,
            ff_speed: de.ff_speed,
            pause_when_unfocused: de.pause_when_unfocused,
            hardware_pref: de.hardware_pref,
            audio_output: de.audio_output,
            github_pat: de.github_pat,
        }
    }
}

/// Persisted Machine Monitor window geometry + section collapse state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorSettings {
    pub width: f64,
    pub height: f64,
    pub x: Option<f64>,
    pub y: Option<f64>,
    #[serde(default)]
    pub sections: MonitorSections,
}

impl Default for MonitorSettings {
    fn default() -> Self {
        Self {
            width: 1000.0,
            height: 700.0,
            x: None,
            y: None,
            sections: MonitorSections::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorSections {
    pub performance: bool,
    pub audio: bool,
    pub profile: bool,
    pub cpu: bool,
    pub apu: bool,
    pub ppu: bool,
    pub input: bool,
}

impl Default for MonitorSections {
    fn default() -> Self {
        Self {
            performance: true,
            audio: true,
            profile: true,
            cpu: false,
            apu: false,
            ppu: false,
            input: false,
        }
    }
}

impl Default for FrontendSettings {
    fn default() -> Self {
        Self {
            palette: PalettePreset::GameBoyLight,
            custom_colors: PalettePreset::GameBoyLight.palette().colors(),
            display_mode: DisplayMode::Sharp,
            integer_scaling: true,
            volume_percent: default_volume_percent(),
            muted: false,
            skip_boot: false,
            recent: Vec::new(),
            last_rom_dir: None,
            monitor: MonitorSettings::default(),
            input: InputSettings::default(),
            host_commands: HostCommandMap::default(),
            rewind_enabled: false,
            ff_speed: default_ff_speed(),
            pause_when_unfocused: false,
            hardware_pref: HostHardwarePref::Automatic,
            audio_output: AudioDevicePref::SystemDefault,
            github_pat: String::new(),
        }
    }
}

impl FrontendSettings {
    pub fn load() -> Self {
        let Some(path) = settings_path() else {
            return Self::default();
        };
        if path.is_file() {
            return fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or_default();
        }
        // One-time migration from the pre-rename `gb-emu` config directory.
        if let Some(legacy) = legacy_settings_path()
            && legacy.is_file()
            && let Ok(bytes) = fs::read(&legacy)
            && let Ok(settings) = serde_json::from_slice::<Self>(&bytes)
        {
            settings.save();
            return settings;
        }
        Self::default()
    }

    pub fn save(&self) {
        let Some(path) = settings_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(self) {
            let _ = fs::write(path, bytes);
        }
    }

    pub fn active_palette(&self) -> Palette {
        match self.palette {
            PalettePreset::Custom => Palette::from_rgb(self.custom_colors),
            other => other.palette(),
        }
    }

    pub fn reset_custom_palette(&mut self) {
        self.custom_colors = PalettePreset::ClassicDmg.palette().colors();
    }

    pub fn audio_gain(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.volume_percent.min(100) as f32 / 100.0
        }
    }

    pub fn remember_rom(&mut self, path: &Path, title: &str) {
        if let Some(parent) = path.parent() {
            self.last_rom_dir = Some(parent.to_path_buf());
        }
        self.recent.retain(|r| r.path != path);
        self.recent
            .insert(0, RecentRom::new(path.to_path_buf(), title.to_string()));
        self.recent.truncate(MAX_RECENTS);
    }
}

fn settings_path() -> Option<PathBuf> {
    let mut dir = dirs::config_dir()?;
    dir.push("Graycart");
    dir.push("settings.json");
    Some(dir)
}

fn legacy_settings_path() -> Option<PathBuf> {
    let mut dir = dirs::config_dir()?;
    dir.push("gb-emu");
    dir.push("settings.json");
    Some(dir)
}

#[cfg(test)]
mod tests {
    use super::super::host_input::PadButton;
    use super::super::playback::SpeedPreset;
    use super::*;
    use graycart::GameBoyButton;

    #[test]
    fn default_gain_is_10_percent() {
        let s = FrontendSettings::default();
        assert!((s.audio_gain() - 0.1).abs() < f32::EPSILON);
        let mut muted = s;
        muted.muted = true;
        assert_eq!(muted.audio_gain(), 0.0);
    }

    #[test]
    fn volume_percent_round_trip_0_1_10_100() {
        for p in [0u8, 1, 10, 100] {
            let s = FrontendSettings {
                volume_percent: p,
                ..Default::default()
            };
            let json = serde_json::to_vec(&s).unwrap();
            let loaded: FrontendSettings = serde_json::from_slice(&json).unwrap();
            assert_eq!(loaded.volume_percent, p);
            assert!((loaded.audio_gain() - (p as f32 / 100.0)).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn migrates_legacy_volume_float_and_skip_hle_alias() {
        let json = r#"{
            "palette": "classic_dmg",
            "custom_colors": [[0,0,0],[85,85,85],[170,170,170],[255,255,255]],
            "display_mode": "sharp",
            "integer_scaling": true,
            "volume": 0.1,
            "muted": false,
            "skip_hle_boot": true,
            "recent": [],
            "last_rom_dir": null
        }"#;
        let loaded: FrontendSettings = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.volume_percent, 10);
        assert!(loaded.skip_boot);
        let reserialized = serde_json::to_string(&loaded).unwrap();
        assert!(reserialized.contains("\"volume_percent\":10"));
        assert!(!reserialized.contains("\"volume\":"));
    }

    #[test]
    fn legacy_volume_1_becomes_100() {
        let json = r#"{
            "palette": "classic_dmg",
            "custom_colors": [[0,0,0],[85,85,85],[170,170,170],[255,255,255]],
            "display_mode": "sharp",
            "integer_scaling": true,
            "volume": 1.0,
            "muted": false,
            "skip_boot": false,
            "recent": [],
            "last_rom_dir": null
        }"#;
        let loaded: FrontendSettings = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.volume_percent, 100);
    }

    #[test]
    fn migrated_volume_percent_not_reinterpreted() {
        let json = r#"{
            "palette": "classic_dmg",
            "custom_colors": [[0,0,0],[85,85,85],[170,170,170],[255,255,255]],
            "display_mode": "sharp",
            "integer_scaling": true,
            "volume_percent": 1,
            "volume": 1.0,
            "muted": false,
            "skip_boot": false,
            "recent": [],
            "last_rom_dir": null
        }"#;
        let loaded: FrontendSettings = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.volume_percent, 1);
    }

    #[test]
    fn default_is_game_boy_light_and_volume_10() {
        let s = FrontendSettings::default();
        assert_eq!(s.palette, PalettePreset::GameBoyLight);
        assert_eq!(s.volume_percent, 10);
        assert!(!s.skip_boot);
        assert_eq!(
            s.audio_output,
            crate::frontend::shell::audio::AudioDevicePref::SystemDefault
        );
    }

    #[test]
    fn missing_audio_output_field_is_system_default() {
        let json = r#"{
            "palette": "classic_dmg",
            "custom_colors": [[0,0,0],[85,85,85],[170,170,170],[255,255,255]],
            "display_mode": "sharp",
            "integer_scaling": true,
            "volume_percent": 10,
            "muted": false
        }"#;
        let loaded: FrontendSettings = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.audio_output, AudioDevicePref::SystemDefault);
    }

    fn minimal_legacy_json() -> String {
        let mut value = serde_json::to_value(FrontendSettings::default()).unwrap();
        if let serde_json::Value::Object(map) = &mut value {
            map.remove("input");
        }
        serde_json::to_string(&value).unwrap()
    }

    fn legacy_json_without_playback_fields() -> String {
        let mut value = serde_json::to_value(FrontendSettings::default()).unwrap();
        if let serde_json::Value::Object(map) = &mut value {
            map.remove("input");
            map.remove("ff_speed");
            map.remove("pause_when_unfocused");
        }
        serde_json::to_string(&value).unwrap()
    }

    #[test]
    fn settings_without_input_field_get_defaults() {
        let s: FrontendSettings = serde_json::from_str(&minimal_legacy_json()).unwrap();
        assert!((s.input.stick_deadzone - 0.25).abs() < f32::EPSILON);
        assert!(s.input.gamepads.is_empty());
        assert_eq!(s.host_commands, HostCommandMap::default());
        assert!(!s.rewind_enabled);
    }

    #[test]
    fn default_ff_speed_is_x4() {
        let s = FrontendSettings::default();
        assert_eq!(s.ff_speed, SpeedPreset::X4);
    }

    #[test]
    fn pause_when_unfocused_defaults_false() {
        let s = FrontendSettings::default();
        assert!(!s.pause_when_unfocused);
    }

    #[test]
    fn hardware_pref_defaults_to_automatic() {
        let s = FrontendSettings::default();
        assert_eq!(s.hardware_pref, graycart::HostHardwarePref::Automatic);
    }

    #[test]
    fn hardware_pref_serde_round_trip() {
        use graycart::HostHardwarePref;
        for pref in [
            HostHardwarePref::Automatic,
            HostHardwarePref::GameBoy,
            HostHardwarePref::GameBoyColor,
        ] {
            let s = FrontendSettings {
                hardware_pref: pref,
                ..Default::default()
            };
            let json = serde_json::to_vec(&s).unwrap();
            let loaded: FrontendSettings = serde_json::from_slice(&json).unwrap();
            assert_eq!(loaded.hardware_pref, pref);
        }
    }

    #[test]
    fn missing_hardware_pref_migrates_to_automatic() {
        let mut value = serde_json::to_value(FrontendSettings::default()).unwrap();
        if let serde_json::Value::Object(map) = &mut value {
            map.remove("hardware_pref");
        }
        let json = serde_json::to_string(&value).unwrap();
        let s: FrontendSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s.hardware_pref, graycart::HostHardwarePref::Automatic);
    }

    #[test]
    fn playback_settings_serde_round_trip() {
        for ff_speed in SpeedPreset::ALL {
            let s = FrontendSettings {
                ff_speed,
                pause_when_unfocused: true,
                ..Default::default()
            };
            let json = serde_json::to_vec(&s).unwrap();
            let loaded: FrontendSettings = serde_json::from_slice(&json).unwrap();
            assert_eq!(loaded.ff_speed, ff_speed);
            assert!(loaded.pause_when_unfocused);
        }
    }

    #[test]
    fn missing_playback_fields_get_defaults() {
        let s: FrontendSettings =
            serde_json::from_str(&legacy_json_without_playback_fields()).unwrap();
        assert_eq!(s.ff_speed, SpeedPreset::X4);
        assert!(!s.pause_when_unfocused);
    }

    #[test]
    fn upsert_profile_does_not_clobber_custom_map() {
        let mut input = InputSettings::default();
        input.upsert_profile("abc".into(), "Pad".into());
        input.gamepads[0]
            .map
            .bind(GameBoyButton::A, PadButton::South);
        input.upsert_profile("abc".into(), "Pad Renamed".into());
        assert_eq!(input.gamepads[0].name, "Pad Renamed");
        assert_eq!(
            input.gamepads[0].map.button(GameBoyButton::A),
            PadButton::South
        );
    }

    #[test]
    fn remember_rom_dedupes_and_caps() {
        let mut s = FrontendSettings::default();
        for i in 0..14 {
            s.remember_rom(Path::new(&format!("/tmp/rom{i}.gb")), &format!("Title {i}"));
        }
        assert_eq!(s.recent.len(), MAX_RECENTS);
        assert_eq!(s.recent[0].path, PathBuf::from("/tmp/rom13.gb"));
        assert_eq!(s.recent[0].title, "Title 13");
        s.remember_rom(Path::new("/tmp/rom5.gb"), "Title 5");
        assert_eq!(s.recent[0].path, PathBuf::from("/tmp/rom5.gb"));
        assert_eq!(
            s.recent
                .iter()
                .filter(|r| r.path.ends_with("rom5.gb"))
                .count(),
            1
        );
    }
}
