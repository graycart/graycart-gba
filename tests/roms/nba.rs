//! nba-emu/hw-test - timing / PPU / DMA hardware suite.
//!
//! Upstream: [nba-emu/hw-test](https://codeberg.org/nba-emu/hw-test) @ `fbc99140` (BSD-3-Clause).
//! Fixtures: `tests/fixtures/nba/`. Scored via MGBA debug printf (`congratulations!`).

use crate::harness::{fixture_root, list_gba_rel, print_row, run_mgba_printf, summarize, tally};
use std::path::PathBuf;

fn root() -> PathBuf {
    fixture_root().join("nba")
}

#[test]
fn nba_fixtures_present() {
    let roms = list_gba_rel(&root());
    assert!(
        !roms.is_empty(),
        "missing nba fixtures under tests/fixtures/nba/"
    );
    eprintln!("nba: {} .gba", roms.len());
}

#[test]
#[ignore = "nba hw-test matrix; run with --ignored --nocapture"]
fn nba_matrix() {
    let roms = list_gba_rel(&root());
    assert!(!roms.is_empty(), "no nba ROMs");
    eprintln!();
    eprintln!("{:<56} result", "ROM");
    let mut counts = [0usize; 4];
    for rel in &roms {
        // Most titles draw an on-screen console; keep the cap modest for matrix runs.
        let frames = if rel.contains("archive/") { 240 } else { 180 };
        let outcome = run_mgba_printf(&root().join(rel), frames);
        print_row(rel, &outcome);
        tally(&mut counts, &outcome);
    }
    summarize(counts);
}
