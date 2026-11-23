//! DenSinH/FuzzARM - interactive ARM/Thumb fuzzer ROMs (GPL-3.0).
//!
//! Upstream: [DenSinH/FuzzARM](https://github.com/DenSinH/FuzzARM) @ `a675329c`.
//! Fixtures: `tests/fixtures/fuzzarm/`. These are not pass/fail gates; presence only.

use crate::harness::{fixture_root, list_gba_rel};
use std::path::PathBuf;

fn root() -> PathBuf {
    fixture_root().join("fuzzarm")
}

#[test]
fn fuzzarm_fixtures_present() {
    let roms = list_gba_rel(&root());
    assert!(
        !roms.is_empty(),
        "missing fuzzarm fixtures under tests/fixtures/fuzzarm/"
    );
    eprintln!("fuzzarm: {} .gba (manual / interactive)", roms.len());
}
