use super::*;
use graycart::GameBoyButton;
use winit::keyboard::KeyCode;

#[test]
fn keyboard_defaults_cover_all_buttons() {
    let map = KeyboardMap::default();
    for b in GameBoyButton::ALL {
        let _ = map.key(b);
    }
    assert_eq!(map.key(GameBoyButton::A), KeyCode::KeyX);
    assert_eq!(map.key(GameBoyButton::B), KeyCode::KeyZ);
}

#[test]
fn gamepad_defaults_cover_all_and_face_buttons() {
    let map = GamepadMap::default();
    assert_eq!(map.button(GameBoyButton::A), PadButton::East);
    assert_eq!(map.button(GameBoyButton::B), PadButton::South);
    for b in GameBoyButton::ALL {
        let _ = map.button(b);
    }
}

#[test]
fn keyboard_bind_swaps_conflict() {
    let mut map = KeyboardMap::default();
    // Bind A to KeyZ (currently B)
    map.bind(GameBoyButton::A, KeyCode::KeyZ);
    assert_eq!(map.key(GameBoyButton::A), KeyCode::KeyZ);
    assert_eq!(map.key(GameBoyButton::B), KeyCode::KeyX); // swapped
}

#[test]
fn gamepad_bind_swaps_conflict() {
    let mut map = GamepadMap::default();
    map.bind(GameBoyButton::A, PadButton::South);
    assert_eq!(map.button(GameBoyButton::A), PadButton::South);
    assert_eq!(map.button(GameBoyButton::B), PadButton::East);
}

#[test]
fn keycode_name_roundtrip_defaults() {
    for code in [
        KeyCode::ArrowUp,
        KeyCode::KeyZ,
        KeyCode::Enter,
        KeyCode::Backspace,
    ] {
        let name = keycode_to_name(code);
        assert_eq!(keycode_from_name(&name), Some(code));
    }
}

#[test]
fn bindable_key_names_roundtrip_beyond_defaults() {
    for code in [
        KeyCode::Backquote,
        KeyCode::BracketLeft,
        KeyCode::Comma,
        KeyCode::Delete,
        KeyCode::Home,
        KeyCode::Numpad5,
        KeyCode::F10,
        KeyCode::F35,
    ] {
        let name = keycode_to_name(code);
        assert_eq!(keycode_from_name(&name), Some(code), "{name}");
    }
}
