//! hades-emu/Hades-Tests - DMA / timer / bios-openbus suite (GPL-2.0).
//!
//! Upstream: [hades-emu/Hades-Tests](https://github.com/hades-emu/Hades-Tests) @ `29274ce3`.
//! Fixtures: `tests/fixtures/hades/`. Scored via MGBA printf (`Total: n/n`).

use crate::harness::{fixture_root, list_gba_rel, print_row, run_mgba_printf, summarize, tally};
use std::path::PathBuf;

fn root() -> PathBuf {
    fixture_root().join("hades")
}

#[test]
fn hades_fixtures_present() {
    let roms = list_gba_rel(&root());
    assert!(
        !roms.is_empty(),
        "missing hades fixtures under tests/fixtures/hades/"
    );
    eprintln!("hades: {} .gba", roms.len());
}

#[test]
#[ignore = "hades matrix; run with --ignored --nocapture"]
fn hades_matrix() {
    let roms = list_gba_rel(&root());
    assert!(!roms.is_empty(), "no hades ROMs");
    eprintln!();
    eprintln!("{:<56} result", "ROM");
    let mut counts = [0usize; 4];
    for rel in &roms {
        let outcome = run_mgba_printf(&root().join(rel), 180);
        print_row(rel, &outcome);
        tally(&mut counts, &outcome);
    }
    summarize(counts);
}
