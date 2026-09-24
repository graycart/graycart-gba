use super::super::HostCommandMap;
use super::*;

#[test]
fn escape_cancels() {
    let mut listen = ListenState::default();
    let mut map = KeyboardMap::default();
    listen.begin(ListenTarget::Keyboard(GameBoyButton::A));

    let host = HostCommandMap::default();
    assert!(listen.on_key(KeyCode::Escape, &mut map, &host));
    assert!(!listen.is_active());
    assert_eq!(map.key(GameBoyButton::A), KeyCode::KeyX);
}

#[test]
fn escape_cancels_gamepad_listen() {
    let mut listen = ListenState::default();
    let mut map = KeyboardMap::default();
    listen.begin(ListenTarget::Gamepad(GameBoyButton::A));

    let host = HostCommandMap::default();
    assert!(listen.on_key(KeyCode::Escape, &mut map, &host));
    assert!(!listen.is_active());
}

#[test]
fn reserved_function_keys_are_ignored() {
    let mut listen = ListenState::default();
    let mut map = KeyboardMap::default();
    let host = HostCommandMap::default();
    listen.begin(ListenTarget::Keyboard(GameBoyButton::A));

    for key in [
        KeyCode::F9,
        KeyCode::F11,
        KeyCode::F12,
        KeyCode::F5,
        KeyCode::F8,
    ] {
        assert!(!listen.on_key(key, &mut map, &host));
        assert!(listen.is_active());
    }
    assert_eq!(map.key(GameBoyButton::A), KeyCode::KeyX);
}

#[test]
fn key_commits_with_swap() {
    let mut listen = ListenState::default();
    let mut map = KeyboardMap::default();
    listen.begin(ListenTarget::Keyboard(GameBoyButton::A));

    let host = HostCommandMap::default();
    assert!(listen.on_key(KeyCode::KeyZ, &mut map, &host));
    assert_eq!(map.key(GameBoyButton::A), KeyCode::KeyZ);
    assert_eq!(map.key(GameBoyButton::B), KeyCode::KeyX);
    assert!(!listen.is_active());
}

#[test]
fn pad_commits_with_swap() {
    let mut listen = ListenState::default();
    let mut map = GamepadMap::default();
    listen.begin(ListenTarget::Gamepad(GameBoyButton::A));

    assert!(listen.on_pad_button(PadButton::South, &mut map));
    assert_eq!(map.button(GameBoyButton::A), PadButton::South);
    assert_eq!(map.button(GameBoyButton::B), PadButton::East);
    assert!(!listen.is_active());
}

#[test]
fn source_mismatch_does_not_finish_listen() {
    let mut listen = ListenState::default();
    let mut keyboard = KeyboardMap::default();
    let mut gamepad = GamepadMap::default();
    listen.begin(ListenTarget::Gamepad(GameBoyButton::A));

    let host = HostCommandMap::default();
    assert!(!listen.on_key(KeyCode::KeyQ, &mut keyboard, &host));
    assert!(listen.is_active());
    listen.begin(ListenTarget::Keyboard(GameBoyButton::A));
    assert!(!listen.on_pad_button(PadButton::West, &mut gamepad));
    assert!(listen.is_active());
}
