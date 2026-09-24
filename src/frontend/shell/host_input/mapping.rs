//! Serializable host ↔ GameBoyButton maps (swap-on-conflict).

use graycart::GameBoyButton;
use serde::{Deserialize, Serialize};
use winit::keyboard::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadButton {
    South,
    East,
    West,
    North,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    Select,
    Start,
    LeftShoulder,
    RightShoulder,
}

impl PadButton {
    pub const BINDABLE: [Self; 12] = [
        Self::South,
        Self::East,
        Self::West,
        Self::North,
        Self::DPadUp,
        Self::DPadDown,
        Self::DPadLeft,
        Self::DPadRight,
        Self::Select,
        Self::Start,
        Self::LeftShoulder,
        Self::RightShoulder,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyboardMap {
    /// Parallel to [`GameBoyButton::ALL`] — store as string names for serde.
    keys: [String; 8],
    /// GBA L (KEYINPUT bit 9). Ignored on Game Boy carts.
    #[serde(default = "default_shoulder_l")]
    pub shoulder_l: String,
    /// GBA R (KEYINPUT bit 8). Ignored on Game Boy carts.
    #[serde(default = "default_shoulder_r")]
    pub shoulder_r: String,
}

fn default_shoulder_l() -> String {
    keycode_to_name(KeyCode::KeyA)
}

fn default_shoulder_r() -> String {
    keycode_to_name(KeyCode::KeyS)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GamepadMap {
    buttons: [PadButton; 8],
    /// GBA L (KEYINPUT bit 9). Ignored on Game Boy carts.
    #[serde(default = "default_pad_shoulder_l")]
    pub shoulder_l: PadButton,
    /// GBA R (KEYINPUT bit 8). Ignored on Game Boy carts.
    #[serde(default = "default_pad_shoulder_r")]
    pub shoulder_r: PadButton,
}

fn default_pad_shoulder_l() -> PadButton {
    PadButton::LeftShoulder
}

fn default_pad_shoulder_r() -> PadButton {
    PadButton::RightShoulder
}

fn button_index(b: GameBoyButton) -> usize {
    GameBoyButton::ALL
        .iter()
        .position(|&x| x == b)
        .expect("GameBoyButton::ALL exhaustive")
}

impl Default for KeyboardMap {
    fn default() -> Self {
        Self {
            keys: [
                keycode_to_name(KeyCode::ArrowRight),
                keycode_to_name(KeyCode::ArrowLeft),
                keycode_to_name(KeyCode::ArrowUp),
                keycode_to_name(KeyCode::ArrowDown),
                keycode_to_name(KeyCode::KeyX),       // A
                keycode_to_name(KeyCode::KeyZ),       // B
                keycode_to_name(KeyCode::ShiftRight), // Select (GBA default)
                keycode_to_name(KeyCode::Enter),
            ],
            shoulder_l: default_shoulder_l(),
            shoulder_r: default_shoulder_r(),
        }
    }
}

impl Default for GamepadMap {
    fn default() -> Self {
        Self {
            buttons: [
                PadButton::DPadRight,
                PadButton::DPadLeft,
                PadButton::DPadUp,
                PadButton::DPadDown,
                PadButton::East,  // A
                PadButton::South, // B
                PadButton::Select,
                PadButton::Start,
            ],
            shoulder_l: default_pad_shoulder_l(),
            shoulder_r: default_pad_shoulder_r(),
        }
    }
}

impl KeyboardMap {
    pub fn key(&self, button: GameBoyButton) -> KeyCode {
        keycode_from_name(&self.keys[button_index(button)])
            .unwrap_or_else(|| Self::default().key(button))
    }

    #[allow(dead_code)] // consumed by the input configuration UI
    pub fn bind(&mut self, button: GameBoyButton, key: KeyCode) {
        let name = keycode_to_name(key);
        let idx = button_index(button);
        // Find conflict
        if let Some(other) = self.keys.iter().position(|k| k == &name)
            && other != idx
        {
            self.keys[other] = self.keys[idx].clone();
        }
        if self.shoulder_l == name {
            self.shoulder_l = self.keys[idx].clone();
        }
        if self.shoulder_r == name {
            self.shoulder_r = self.keys[idx].clone();
        }
        self.keys[idx] = name;
    }

    pub fn bind_shoulder_l(&mut self, key: KeyCode) {
        let name = keycode_to_name(key);
        if let Some(other) = self.keys.iter().position(|k| k == &name) {
            self.keys[other] = self.shoulder_l.clone();
        }
        if self.shoulder_r == name {
            self.shoulder_r = self.shoulder_l.clone();
        }
        self.shoulder_l = name;
    }

    pub fn bind_shoulder_r(&mut self, key: KeyCode) {
        let name = keycode_to_name(key);
        if let Some(other) = self.keys.iter().position(|k| k == &name) {
            self.keys[other] = self.shoulder_r.clone();
        }
        if self.shoulder_l == name {
            self.shoulder_l = self.shoulder_r.clone();
        }
        self.shoulder_r = name;
    }

    pub fn restore_defaults(&mut self) {
        *self = Self::default();
    }
}

impl GamepadMap {
    pub fn button(&self, button: GameBoyButton) -> PadButton {
        self.buttons[button_index(button)]
    }

    #[allow(dead_code)] // consumed by the input configuration UI
    pub fn bind(&mut self, button: GameBoyButton, pad: PadButton) {
        let idx = button_index(button);
        if let Some(other) = self.buttons.iter().position(|&b| b == pad)
            && other != idx
        {
            self.buttons[other] = self.buttons[idx];
        }
        if self.shoulder_l == pad {
            self.shoulder_l = self.buttons[idx];
        }
        if self.shoulder_r == pad {
            self.shoulder_r = self.buttons[idx];
        }
        self.buttons[idx] = pad;
    }

    pub fn bind_shoulder_l(&mut self, pad: PadButton) {
        if let Some(other) = self.buttons.iter().position(|&b| b == pad) {
            self.buttons[other] = self.shoulder_l;
        }
        if self.shoulder_r == pad {
            self.shoulder_r = self.shoulder_l;
        }
        self.shoulder_l = pad;
    }

    pub fn bind_shoulder_r(&mut self, pad: PadButton) {
        if let Some(other) = self.buttons.iter().position(|&b| b == pad) {
            self.buttons[other] = self.shoulder_r;
        }
        if self.shoulder_l == pad {
            self.shoulder_l = self.shoulder_r;
        }
        self.shoulder_r = pad;
    }

    pub fn restore_defaults(&mut self) {
        *self = Self::default();
    }
}

pub fn keycode_to_name(code: KeyCode) -> String {
    format!("{code:?}")
}

pub fn keycode_from_name(name: &str) -> Option<KeyCode> {
    // Match Debug names used by keycode_to_name for the keys we care about.
    // Unknown names → None (caller falls back to default for that slot).
    use KeyCode::*;
    Some(match name {
        "Backquote" => Backquote,
        "Backslash" => Backslash,
        "BracketLeft" => BracketLeft,
        "BracketRight" => BracketRight,
        "Comma" => Comma,
        "ArrowRight" => ArrowRight,
        "ArrowLeft" => ArrowLeft,
        "ArrowUp" => ArrowUp,
        "ArrowDown" => ArrowDown,
        "KeyA" => KeyA,
        "KeyB" => KeyB,
        "KeyC" => KeyC,
        "KeyD" => KeyD,
        "KeyE" => KeyE,
        "KeyF" => KeyF,
        "KeyG" => KeyG,
        "KeyH" => KeyH,
        "KeyI" => KeyI,
        "KeyJ" => KeyJ,
        "KeyK" => KeyK,
        "KeyL" => KeyL,
        "KeyM" => KeyM,
        "KeyN" => KeyN,
        "KeyO" => KeyO,
        "KeyP" => KeyP,
        "KeyQ" => KeyQ,
        "KeyR" => KeyR,
        "KeyS" => KeyS,
        "KeyT" => KeyT,
        "KeyU" => KeyU,
        "KeyV" => KeyV,
        "KeyW" => KeyW,
        "KeyX" => KeyX,
        "KeyY" => KeyY,
        "KeyZ" => KeyZ,
        "Minus" => Minus,
        "Period" => Period,
        "Quote" => Quote,
        "Slash" => Slash,
        "Enter" => Enter,
        "Backspace" => Backspace,
        "CapsLock" => CapsLock,
        "ContextMenu" => ContextMenu,
        "Space" => Space,
        "Semicolon" => Semicolon,
        "ShiftLeft" => ShiftLeft,
        "ShiftRight" => ShiftRight,
        "ControlLeft" => ControlLeft,
        "ControlRight" => ControlRight,
        "AltLeft" => AltLeft,
        "AltRight" => AltRight,
        "SuperLeft" => SuperLeft,
        "SuperRight" => SuperRight,
        "Tab" => Tab,
        "Escape" => Escape,
        "Delete" => Delete,
        "End" => End,
        "Home" => Home,
        "Insert" => Insert,
        "PageDown" => PageDown,
        "PageUp" => PageUp,
        "Digit0" => Digit0,
        "Digit1" => Digit1,
        "Digit2" => Digit2,
        "Digit3" => Digit3,
        "Digit4" => Digit4,
        "Digit5" => Digit5,
        "Digit6" => Digit6,
        "Digit7" => Digit7,
        "Digit8" => Digit8,
        "Digit9" => Digit9,
        "Equal" => Equal,
        "Numpad0" => Numpad0,
        "Numpad1" => Numpad1,
        "Numpad2" => Numpad2,
        "Numpad3" => Numpad3,
        "Numpad4" => Numpad4,
        "Numpad5" => Numpad5,
        "Numpad6" => Numpad6,
        "Numpad7" => Numpad7,
        "Numpad8" => Numpad8,
        "Numpad9" => Numpad9,
        "NumpadAdd" => NumpadAdd,
        "NumpadDecimal" => NumpadDecimal,
        "NumpadDivide" => NumpadDivide,
        "NumpadEnter" => NumpadEnter,
        "NumpadEqual" => NumpadEqual,
        "NumpadMultiply" => NumpadMultiply,
        "NumpadSubtract" => NumpadSubtract,
        "F1" => F1,
        "F2" => F2,
        "F3" => F3,
        "F4" => F4,
        "F5" => F5,
        "F6" => F6,
        "F7" => F7,
        "F8" => F8,
        "F9" => F9,
        "F10" => F10,
        "F11" => F11,
        "F12" => F12,
        "F13" => F13,
        "F14" => F14,
        "F15" => F15,
        "F16" => F16,
        "F17" => F17,
        "F18" => F18,
        "F19" => F19,
        "F20" => F20,
        "F21" => F21,
        "F22" => F22,
        "F23" => F23,
        "F24" => F24,
        "F25" => F25,
        "F26" => F26,
        "F27" => F27,
        "F28" => F28,
        "F29" => F29,
        "F30" => F30,
        "F31" => F31,
        "F32" => F32,
        "F33" => F33,
        "F34" => F34,
        "F35" => F35,
        _ => return None,
    })
}

#[allow(dead_code)] // consumed by the input configuration UI
pub fn keycode_label(code: KeyCode) -> String {
    match code {
        KeyCode::ArrowUp => "↑".into(),
        KeyCode::ArrowDown => "↓".into(),
        KeyCode::ArrowLeft => "←".into(),
        KeyCode::ArrowRight => "→".into(),
        KeyCode::Enter => "Enter".into(),
        KeyCode::Backspace => "Backspace".into(),
        KeyCode::Space => "Space".into(),
        other => format!("{other:?}").replace("Key", ""),
    }
}

pub fn pad_button_label(b: PadButton) -> &'static str {
    match b {
        PadButton::South => "South (A/✕)",
        PadButton::East => "East (B/○)",
        PadButton::West => "West (X/□)",
        PadButton::North => "North (Y/△)",
        PadButton::DPadUp => "D-Pad Up",
        PadButton::DPadDown => "D-Pad Down",
        PadButton::DPadLeft => "D-Pad Left",
        PadButton::DPadRight => "D-Pad Right",
        PadButton::Select => "Select",
        PadButton::Start => "Start",
        PadButton::LeftShoulder => "L (LB)",
        PadButton::RightShoulder => "R (RB)",
    }
}

#[cfg(test)]
mod tests;
