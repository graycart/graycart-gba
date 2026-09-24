use super::{ROM_EXTENSIONS, is_rom_path};
use std::path::Path;

#[test]
fn accepts_gb_gbc_rom_bin_case_insensitive() {
    for ext in ROM_EXTENSIONS {
        let lower = format!("/games/title.{}", ext);
        let upper = format!("/games/title.{}", ext.to_uppercase());
        let mixed = format!("/games/title.{}", capitalize(ext));
        for path in [&lower, &upper, &mixed] {
            assert!(
                is_rom_path(Path::new(path)),
                "expected {path} to be accepted"
            );
        }
    }
}

#[test]
fn rejects_unknown_extensions() {
    for path in [
        "/games/title.txt",
        "/games/title.sav",
        "/games/title",
        "/games/title.",
        "/games/title.gb.bak",
    ] {
        assert!(
            !is_rom_path(Path::new(path)),
            "expected {path} to be rejected"
        );
    }
}

fn capitalize(s: &str) -> String {
    let mut out = s.to_string();
    if let Some(first) = out.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    out
}
