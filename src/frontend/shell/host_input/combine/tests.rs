use super::*;
use graycart::GameBoyButton;

#[test]
fn or_keeps_right_when_keyboard_releases_but_pad_holds() {
    let mut edges = EdgeTracker::default();
    let kb = button_bit(GameBoyButton::Right);
    let pad = button_bit(GameBoyButton::Right);
    let e = edges.edges(combine(kb, pad));
    assert_eq!(e.pressed, vec![GameBoyButton::Right]);
    let e = edges.edges(combine(0, pad)); // kb released
    assert!(e.released.is_empty());
    assert!(e.pressed.is_empty());
}

#[test]
fn opposite_directions_both_set_no_socd() {
    let mask = combine(
        button_bit(GameBoyButton::Left),
        button_bit(GameBoyButton::Right),
    );
    assert_ne!(mask & button_bit(GameBoyButton::Left), 0);
    assert_ne!(mask & button_bit(GameBoyButton::Right), 0);
}

#[test]
fn pad_disconnect_zero_mask_emits_release() {
    let mut edges = EdgeTracker::default();
    let pad = button_bit(GameBoyButton::A);
    edges.edges(combine(0, pad));
    let e = edges.edges(combine(0, 0));
    assert_eq!(e.released, vec![GameBoyButton::A]);
}
