//! Path / payload scrubbing for issue bodies (no ROM bytes, no username paths).

use std::path::{Component, Path};

/// Keep only the final path component (basename). Works for `/` and `\` separators.
pub fn path_basename(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let p = Path::new(&normalized);
    match p.file_name().and_then(|s| s.to_str()) {
        Some(name) if !name.is_empty() && name != "." => name.to_string(),
        _ => p
            .components()
            .rev()
            .find_map(|c| match c {
                Component::Normal(s) => s.to_str().map(str::to_string),
                _ => None,
            })
            .unwrap_or_default(),
    }
}

/// True if `text` looks like it embeds raw ROM image bytes (nulls / huge binary).
pub fn looks_like_rom_bytes(text: &str) -> bool {
    if text.contains('\0') {
        return true;
    }
    // Heuristic: dense non-text control bytes outside common whitespace.
    let control = text
        .bytes()
        .filter(|b| *b < 0x09 || (*b > 0x0d && *b < 0x20))
        .count();
    control > 32 && control * 20 > text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rom_path_becomes_basename_only() {
        assert_eq!(
            path_basename("/home/alice/Games/POKEMON RED.gb"),
            "POKEMON RED.gb"
        );
        assert_eq!(path_basename(r"C:\Users\bob\carts\zelda.gbc"), "zelda.gbc");
        assert_eq!(path_basename("plain.gb"), "plain.gb");
        assert_eq!(path_basename(""), "");
    }

    #[test]
    fn rejects_null_laden_payload_as_rom_bytes() {
        assert!(looks_like_rom_bytes("header\0\0\0payload"));
        assert!(!looks_like_rom_bytes("normal diagnostic text\nline 2"));
    }
}
