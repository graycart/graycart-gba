use super::game_image_size;
use crate::frontend::shell::video::DisplayMode;

#[test]
fn integer_scale_uses_floor_multiple() {
    // 640×480 content → 4× (640/160=4, 480/144≈3.33 → min floor = 3)
    let (w, h) = game_image_size(640.0, 480.0, true, DisplayMode::Sharp, 160.0, 144.0);
    assert_eq!((w, h), (480.0, 432.0));
}

#[test]
fn integer_scale_exact_4x() {
    let (w, h) = game_image_size(640.0, 576.0, true, DisplayMode::Sharp, 160.0, 144.0);
    assert_eq!((w, h), (640.0, 576.0));
}

#[test]
fn fill_mode_fractional_fit() {
    let (w, h) = game_image_size(320.0, 288.0, false, DisplayMode::Sharp, 160.0, 144.0);
    assert!((w - 320.0).abs() < 0.01);
    assert!((h - 288.0).abs() < 0.01);
}

#[test]
fn soft_modes_ignore_integer_flag() {
    // Soft prefers linear — fractional fit even when integer_scaling is true.
    let (w, h) = game_image_size(400.0, 300.0, true, DisplayMode::Soft, 160.0, 144.0);
    let scale = (400.0_f32 / 160.0).min(300.0 / 144.0);
    assert!((w - 160.0 * scale).abs() < 0.01);
    assert!((h - 144.0 * scale).abs() < 0.01);
}

#[test]
fn zero_available_yields_zero() {
    assert_eq!(
        game_image_size(0.0, 100.0, true, DisplayMode::Sharp, 160.0, 144.0),
        (0.0, 0.0)
    );
}
