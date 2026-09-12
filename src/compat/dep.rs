//! graycart (DMG/CGB) dependency pin note — P10 **G10-dep** / P12 **G12-audit-dep**.
//!
//! Cited: graycart-gba `10-core-api-and-gb-reuse.md` (whole-crate OK until extract)
//!   Project store: `docs/graycart-gba/10-core-api-and-gb-reuse.md`
//! Cited: https://github.com/graycart/graycart-gb (crate name `graycart`)
//! Cited: `docs/supersede-cutover.md` — supersede the **app**, keep this lib dep.
//! Note: prefer machine-only surface (`Cpu`, `Bus`, `Cartridge`, …). Do not
//! reinvent SM83/PPU/APU in this tree; do not port gb into ARM/GBA-native.

/// Git rev pinned in `Cargo.toml` for the interim whole-crate dep.
///
/// Keep in sync when bumping the `graycart` git dependency.
pub const GRAYCART_GIT_REV: &str = "80833e3f8732be8baa1442d3e6bdeb09d7b5f533";

/// Human label for docs / conformance.
pub const GRAYCART_DEP_LABEL: &str = "graycart 0.11.5 (git whole-crate, interim)";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dep_pin_is_nonempty_sha() {
        assert_eq!(GRAYCART_GIT_REV.len(), 40);
        assert!(GRAYCART_GIT_REV.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(GRAYCART_DEP_LABEL.contains("graycart"));
    }
}
