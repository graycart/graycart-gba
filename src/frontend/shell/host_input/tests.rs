use super::*;
use crate::frontend::shell::settings::FrontendSettings;
use graycart::GameBoyButton;
use std::collections::HashSet;
use winit::keyboard::KeyCode;

#[test]
fn keyboard_mask_uses_configured_map() {
    let mut map = KeyboardMap::default();
    map.bind(GameBoyButton::A, KeyCode::KeyQ);
    map.bind(GameBoyButton::Left, KeyCode::KeyA);
    let keys = HashSet::from([KeyCode::KeyQ, KeyCode::KeyA]);

    let mask = keyboard_mask(&map, &keys);

    assert_eq!(
        mask,
        button_bit(GameBoyButton::A) | button_bit(GameBoyButton::Left)
    );
}

#[test]
fn keyboard_mask_ignores_unmapped_keys() {
    let map = KeyboardMap::default();
    let keys = HashSet::from([KeyCode::KeyQ]);

    assert_eq!(keyboard_mask(&map, &keys), 0);
}

#[test]
fn poll_skips_edge_tracking_until_deliver() {
    let mut input = InputFrontend::new();
    let mut settings = FrontendSettings::default();
    let a_key = settings.input.keyboard.key(GameBoyButton::A);
    let keys = HashSet::from([a_key]);

    let held = input.poll(&mut settings, &keys, true, false);
    assert!(held.edges.pressed.is_empty());
    assert_ne!(held.effective_mask & button_bit(GameBoyButton::A), 0);

    let first_deliver = input.poll(&mut settings, &keys, true, true);
    assert_eq!(first_deliver.edges.pressed, vec![GameBoyButton::A]);

    let still_held = input.poll(&mut settings, &keys, true, true);
    assert!(still_held.edges.pressed.is_empty());
    assert!(still_held.edges.released.is_empty());
}

#[test]
fn reset_edges_re_emits_press_for_held_key() {
    let mut input = InputFrontend::new();
    let mut settings = FrontendSettings::default();
    let a_key = settings.input.keyboard.key(GameBoyButton::A);
    let keys = HashSet::from([a_key]);

    let _ = input.poll(&mut settings, &keys, true, true);
    let still_held = input.poll(&mut settings, &keys, true, true);
    assert!(still_held.edges.pressed.is_empty());

    input.reset_edges();
    let after_reset = input.poll(&mut settings, &keys, true, true);
    assert_eq!(after_reset.edges.pressed, vec![GameBoyButton::A]);
}

#[test]
fn restore_input_policy_applies_press_edges_to_joypad() {
    use graycart::Joypad;

    let mut input = InputFrontend::new();
    let mut settings = FrontendSettings::default();
    let a_key = settings.input.keyboard.key(GameBoyButton::A);
    let keys = HashSet::from([a_key]);

    let _ = input.poll(&mut settings, &keys, true, true);
    let _ = input.poll(&mut settings, &keys, true, true);

    input.reset_edges();
    let poll = input.poll(&mut settings, &keys, true, true);
    assert_eq!(poll.edges.pressed, vec![GameBoyButton::A]);

    let mut joypad = Joypad::new();
    for button in &poll.edges.pressed {
        joypad.press(*button);
    }
    assert!(joypad.is_pressed(GameBoyButton::A));
}
