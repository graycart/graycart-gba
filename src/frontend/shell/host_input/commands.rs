//! Serializable host hotkeys (save/load, screenshot, rewind, …) — separate from GB maps.

use super::mapping::{KeyboardMap, keycode_from_name, keycode_to_name};
use graycart::GameBoyButton;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use winit::keyboard::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostCommand {
    QuickSave,
    QuickLoad,
    Screenshot,
    Rewind,
    Fullscreen,
    Monitor,
    Pause,
    FrameAdvance,
    FastForwardHold,
    ToggleFastForward,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct HostCommandKeys {
    #[serde(default = "default_quick_save_key")]
    quick_save: String,
    #[serde(default = "default_quick_load_key")]
    quick_load: String,
    #[serde(default = "default_screenshot_key")]
    screenshot: String,
    #[serde(default = "default_rewind_key")]
    rewind: String,
    #[serde(default = "default_fullscreen_key")]
    fullscreen: String,
    #[serde(default = "default_monitor_key")]
    monitor: String,
    #[serde(default = "default_pause_key")]
    pause: String,
    #[serde(default = "default_frame_advance_key")]
    frame_advance: String,
    #[serde(default = "default_fast_forward_hold_key")]
    fast_forward_hold: String,
    #[serde(default = "default_toggle_fast_forward_key")]
    toggle_fast_forward: String,
}

fn default_quick_save_key() -> String {
    keycode_to_name(KeyCode::F5)
}

fn default_quick_load_key() -> String {
    keycode_to_name(KeyCode::F8)
}

fn default_screenshot_key() -> String {
    keycode_to_name(KeyCode::F6)
}

fn default_rewind_key() -> String {
    keycode_to_name(KeyCode::KeyR)
}

fn default_fullscreen_key() -> String {
    keycode_to_name(KeyCode::F11)
}

fn default_monitor_key() -> String {
    keycode_to_name(KeyCode::F12)
}

fn default_pause_key() -> String {
    keycode_to_name(KeyCode::Space)
}

fn default_frame_advance_key() -> String {
    keycode_to_name(KeyCode::Period)
}

fn default_fast_forward_hold_key() -> String {
    keycode_to_name(KeyCode::Tab)
}

fn default_toggle_fast_forward_key() -> String {
    keycode_to_name(KeyCode::Backslash)
}

impl Default for HostCommandKeys {
    fn default() -> Self {
        Self {
            quick_save: default_quick_save_key(),
            quick_load: default_quick_load_key(),
            screenshot: default_screenshot_key(),
            rewind: default_rewind_key(),
            fullscreen: default_fullscreen_key(),
            monitor: default_monitor_key(),
            pause: default_pause_key(),
            frame_advance: default_frame_advance_key(),
            fast_forward_hold: default_fast_forward_hold_key(),
            toggle_fast_forward: default_toggle_fast_forward_key(),
        }
    }
}

impl HostCommandKeys {
    fn slot(&self, cmd: HostCommand) -> &str {
        match cmd {
            HostCommand::QuickSave => &self.quick_save,
            HostCommand::QuickLoad => &self.quick_load,
            HostCommand::Screenshot => &self.screenshot,
            HostCommand::Rewind => &self.rewind,
            HostCommand::Fullscreen => &self.fullscreen,
            HostCommand::Monitor => &self.monitor,
            HostCommand::Pause => &self.pause,
            HostCommand::FrameAdvance => &self.frame_advance,
            HostCommand::FastForwardHold => &self.fast_forward_hold,
            HostCommand::ToggleFastForward => &self.toggle_fast_forward,
        }
    }

    fn slot_mut(&mut self, cmd: HostCommand) -> &mut String {
        match cmd {
            HostCommand::QuickSave => &mut self.quick_save,
            HostCommand::QuickLoad => &mut self.quick_load,
            HostCommand::Screenshot => &mut self.screenshot,
            HostCommand::Rewind => &mut self.rewind,
            HostCommand::Fullscreen => &mut self.fullscreen,
            HostCommand::Monitor => &mut self.monitor,
            HostCommand::Pause => &mut self.pause,
            HostCommand::FrameAdvance => &mut self.frame_advance,
            HostCommand::FastForwardHold => &mut self.fast_forward_hold,
            HostCommand::ToggleFastForward => &mut self.toggle_fast_forward,
        }
    }

    fn from_legacy(
        [
            quick_save,
            quick_load,
            screenshot,
            rewind,
            fullscreen,
            monitor,
        ]: [String; 6],
    ) -> Self {
        Self {
            quick_save,
            quick_load,
            screenshot,
            rewind,
            fullscreen,
            monitor,
            pause: default_pause_key(),
            frame_advance: default_frame_advance_key(),
            fast_forward_hold: default_fast_forward_hold_key(),
            toggle_fast_forward: default_toggle_fast_forward_key(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HostCommandMap {
    keys: HostCommandKeys,
}

impl<'de> Deserialize<'de> for HostCommandMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Legacy([String; 6]),
            Keyed(HostCommandKeys),
        }

        match Repr::deserialize(deserializer)? {
            Repr::Legacy(arr) => Ok(Self {
                keys: HostCommandKeys::from_legacy(arr),
            }),
            Repr::Keyed(keys) => Ok(Self { keys }),
        }
    }
}

impl Serialize for HostCommandMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.keys.serialize(serializer)
    }
}

impl HostCommandMap {
    pub const ALL: [HostCommand; 10] = [
        HostCommand::QuickSave,
        HostCommand::QuickLoad,
        HostCommand::Screenshot,
        HostCommand::Rewind,
        HostCommand::Fullscreen,
        HostCommand::Monitor,
        HostCommand::Pause,
        HostCommand::FrameAdvance,
        HostCommand::FastForwardHold,
        HostCommand::ToggleFastForward,
    ];

    pub fn key(&self, cmd: HostCommand) -> KeyCode {
        keycode_from_name(self.keys.slot(cmd)).unwrap_or_else(|| Self::default().key(cmd))
    }

    #[allow(dead_code)] // host command configuration UI
    pub fn bind(&mut self, cmd: HostCommand, key: KeyCode) {
        let name = keycode_to_name(key);
        let idx = Self::ALL
            .iter()
            .position(|c| *c == cmd)
            .expect("HostCommand in ALL");
        if let Some(other) = Self::ALL
            .iter()
            .position(|c| self.keys.slot(*c) == name)
            .filter(|other| *other != idx)
        {
            let other_cmd = Self::ALL[other];
            *self.keys.slot_mut(other_cmd) = self.keys.slot(cmd).to_string();
        }
        *self.keys.slot_mut(cmd) = name;
    }

    #[allow(dead_code)] // configure-controls conflict feedback
    pub fn conflicts_with(&self, kb: &KeyboardMap) -> Vec<(HostCommand, GameBoyButton)> {
        let mut out = Vec::new();
        for cmd in Self::ALL {
            let hk = self.key(cmd);
            for button in GameBoyButton::ALL {
                if kb.key(button) == hk {
                    out.push((cmd, button));
                }
            }
        }
        out
    }
}

pub fn is_reserved_host_key(key: KeyCode, host: &HostCommandMap) -> bool {
    HostCommandMap::ALL.iter().any(|c| host.key(*c) == key)
}

#[cfg(test)]
mod tests;
