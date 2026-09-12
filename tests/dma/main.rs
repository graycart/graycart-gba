//! DMA integration / fixture hygiene (P2 Immediate).
//!
//! Cited: jsmolka/gba-tests memory/ (MIT) — P2 bus gate ROM path stub
//!   https://github.com/jsmolka/gba-tests/tree/master/memory
//! Note: memory.gba is vendored (workstream C); execute/oracle still D/E.

use std::path::Path;

#[test]
fn jsmolka_memory_fixture_dir_exists() {
    let dir = Path::new("tests/fixtures/jsmolka/memory");
    assert!(
        dir.is_dir(),
        "missing {dir:?} — expected P2 path stub under jsmolka/"
    );
    let readme = dir.join("README.md");
    assert!(readme.is_file(), "missing {readme:?}");
}

#[test]
fn jsmolka_license_still_covers_memory_stub() {
    let license = Path::new("tests/fixtures/jsmolka/LICENSE");
    assert!(license.is_file());
    let text = std::fs::read_to_string(license).expect("read LICENSE");
    assert!(
        text.contains("Julian Smolka") || text.contains("MIT"),
        "jsmolka LICENSE should retain upstream MIT attribution"
    );
}

#[test]
fn jsmolka_memory_rom_present() {
    let path = Path::new("tests/fixtures/jsmolka/memory/memory.gba");
    assert!(
        path.is_file(),
        "missing {} — vendor from https://github.com/jsmolka/gba-tests",
        path.display()
    );
}
