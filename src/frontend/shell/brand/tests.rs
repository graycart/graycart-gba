use super::{NAME, crate_version, window_title};

#[test]
fn crate_version_matches_cargo_package() {
    assert_eq!(crate_version(), env!("CARGO_PKG_VERSION"));
    assert!(!crate_version().is_empty());
}

#[test]
fn window_title_includes_version_and_rom() {
    let v = crate_version();
    assert_eq!(window_title(""), format!("{NAME} {v}"));
    assert_eq!(window_title("Tetris"), format!("{NAME} {v} — Tetris"));
}
