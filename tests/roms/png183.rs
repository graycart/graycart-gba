//! png183/gba-tests - fork/edge suite (not a page must-gate).
//!
//! Upstream: [png183/gba-tests](https://github.com/png183/gba-tests) @ `87937453`.
//! Fixtures: `tests/fixtures/png183/` (`unsafe/` skipped).

use crate::harness::{
    JsmolkaPass, fixture_root, list_gba_rel, print_row, run_jsmolka, summarize, tally,
};
use std::path::PathBuf;

fn root() -> PathBuf {
    fixture_root().join("png183")
}

fn pass_mode(rel: &str) -> JsmolkaPass {
    if rel.contains("thumb/thumb.gba") || rel.ends_with("/thumb.gba") {
        JsmolkaPass::ThumbR7
    } else {
        JsmolkaPass::R12
    }
}

fn frames_for(rel: &str) -> u32 {
    if rel.contains("save/") {
        90
    } else if rel.contains("prefetcher/") || rel.contains("timer/") {
        240
    } else {
        120
    }
}

#[test]
fn png183_fixtures_present() {
    let roms = list_gba_rel(&root());
    assert!(
        !roms.is_empty(),
        "missing png183 fixtures under tests/fixtures/png183/"
    );
    eprintln!("png183: {} .gba (unsafe excluded)", roms.len());
}

#[test]
#[ignore = "png183 edge matrix; run with --ignored --nocapture"]
fn png183_matrix() {
    let roms = list_gba_rel(&root());
    assert!(!roms.is_empty(), "no png183 ROMs");
    eprintln!();
    eprintln!("{:<56} result", "ROM");
    let mut counts = [0usize; 4];
    for rel in &roms {
        let outcome = run_jsmolka(&root().join(rel), frames_for(rel), pass_mode(rel));
        print_row(rel, &outcome);
        tally(&mut counts, &outcome);
    }
    summarize(counts);
    assert_eq!(counts[3], 0, "png183 hit UNSUPPORTED");
}
