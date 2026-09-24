use super::*;
use crate::frontend::shell::host_input::combine::button_bit;
use graycart::GameBoyButton;

#[test]
fn press_above_threshold_sets_right() {
    let mut s = StickDigital::default();
    let bits = s.update(0.30, 0.0, 0.25);
    assert_ne!(bits & button_bit(GameBoyButton::Right), 0);
}

#[test]
fn hysteresis_holds_in_band_then_releases() {
    let mut s = StickDigital::default();
    assert_ne!(
        s.update(0.30, 0.0, 0.25) & button_bit(GameBoyButton::Right),
        0
    );
    // Between release (0.20) and press (0.25) — still held
    assert_ne!(
        s.update(0.22, 0.0, 0.25) & button_bit(GameBoyButton::Right),
        0
    );
    assert_eq!(
        s.update(0.10, 0.0, 0.25) & button_bit(GameBoyButton::Right),
        0
    );
}

#[test]
fn diagonal_sets_two_bits() {
    let mut s = StickDigital::default();
    let bits = s.update(0.5, 0.5, 0.25); // right + up (Y flipped vs raw gilrs)
    assert_ne!(bits & button_bit(GameBoyButton::Right), 0);
    assert_ne!(bits & button_bit(GameBoyButton::Up), 0);
}
