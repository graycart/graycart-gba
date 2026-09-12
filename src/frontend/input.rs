//! Keyboard → GBA keypad mask + host commands (pause / reset / open).
//!
//! Cited: GBATEK — GBA Keypad Input
//!   https://problemkaputt.de/gbatek-gba-keypad-input.htm
//! Cited: graycart-gb frontend input posture (simplified defaults)
//!   https://github.com/graycart/graycart-gb/tree/main/src/frontend/input

use egui::Key;
use graycart_gba::input::button;

/// Host-level commands (not silicon buttons).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostCommand {
    TogglePause,
    Reset,
    OpenRom,
    Quit,
}

/// Default keyboard → GBA button bit.
#[must_use]
pub fn key_to_button(key: Key) -> Option<u16> {
    Some(match key {
        Key::X => button::A,
        Key::Z => button::B,
        Key::Enter => button::START,
        Key::Backspace => button::SELECT,
        Key::ArrowRight => button::RIGHT,
        Key::ArrowLeft => button::LEFT,
        Key::ArrowUp => button::UP,
        Key::ArrowDown => button::DOWN,
        Key::A => button::L,
        Key::S => button::R,
        _ => return None,
    })
}

/// Hotkeys that are host commands (checked before pad mapping when modifiers allow).
#[must_use]
pub fn key_to_command(key: Key) -> Option<HostCommand> {
    Some(match key {
        Key::P => HostCommand::TogglePause,
        Key::R => HostCommand::Reset,
        Key::O => HostCommand::OpenRom,
        Key::Escape => HostCommand::Quit,
        _ => return None,
    })
}

/// Build a pressed mask from a set of held egui keys.
#[must_use]
pub fn buttons_from_keys<'a>(keys: impl IntoIterator<Item = &'a Key>) -> u16 {
    let mut mask = 0u16;
    for k in keys {
        if let Some(b) = key_to_button(*k) {
            mask |= b;
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_map_covers_face_and_dpad() {
        assert_eq!(key_to_button(Key::X), Some(button::A));
        assert_eq!(key_to_button(Key::Z), Some(button::B));
        assert_eq!(key_to_button(Key::ArrowUp), Some(button::UP));
        assert_eq!(key_to_button(Key::A), Some(button::L));
        assert_eq!(key_to_button(Key::S), Some(button::R));
    }

    #[test]
    fn host_commands_are_distinct_from_pad() {
        assert_eq!(key_to_command(Key::P), Some(HostCommand::TogglePause));
        assert_eq!(key_to_command(Key::R), Some(HostCommand::Reset));
        assert!(key_to_button(Key::P).is_none());
    }

    #[test]
    fn buttons_from_keys_ors_mask() {
        let keys = [Key::X, Key::Z, Key::Enter];
        let m = buttons_from_keys(keys.iter());
        assert_eq!(m, button::A | button::B | button::START);
    }
}
