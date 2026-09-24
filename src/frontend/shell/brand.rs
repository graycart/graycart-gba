//! Product name and crate SemVer for chrome / About (not machine state).

pub const NAME: &str = "Graycart";

pub fn crate_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Window title: `Graycart <version>` or `Graycart <version> — <rom>`.
pub fn window_title(rom_title: &str) -> String {
    let v = crate_version();
    if rom_title.is_empty() {
        format!("{NAME} {v}")
    } else {
        format!("{NAME} {v} — {rom_title}")
    }
}

#[cfg(test)]
mod tests;
