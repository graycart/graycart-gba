use super::*;
use std::fs;

#[test]
fn screenshot_filename_uses_ascii_title_and_timestamp_shape() {
    let name = screenshot_filename("Pokemon Red");
    assert!(name.starts_with("Pokemon Red_"));
    assert!(name.ends_with(".png"));
    assert!(!name.contains(':'));
}

#[test]
fn write_presented_png_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let rgba = vec![0xFFu8; 160 * 144 * 4];
    let path = write_presented_png_to(dir.path(), "Pokemon Red", &rgba, 160, 144).unwrap();
    assert!(path.exists());
    let header = fs::read(&path).unwrap();
    assert_eq!(&header[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
}
