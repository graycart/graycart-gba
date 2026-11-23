//! alyosha-tas/gba-tests - edge / timing suite (not a page must-gate).
//!
//! Upstream: [alyosha-tas/gba-tests](https://github.com/alyosha-tas/gba-tests) @ `66f5f1d6`.
//! Fixtures: `tests/fixtures/alyosha/` (`unsafe/` skipped).

use crate::harness::{
    JsmolkaPass, fixture_root, list_gba_rel, print_row, run_jsmolka, summarize, tally,
};
use std::path::PathBuf;

fn root() -> PathBuf {
    fixture_root().join("alyosha")
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
    } else if rel.contains("Serial/") || rel.contains("timing/") || rel.contains("prefetcher/") {
        240
    } else {
        120
    }
}

#[test]
fn alyosha_fixtures_present() {
    let roms = list_gba_rel(&root());
    assert!(
        !roms.is_empty(),
        "missing alyosha fixtures under tests/fixtures/alyosha/"
    );
    eprintln!("alyosha: {} .gba (unsafe excluded)", roms.len());
}

#[test]
#[ignore = "alyosha edge matrix; run with --ignored --nocapture"]
fn alyosha_matrix() {
    let roms = list_gba_rel(&root());
    assert!(!roms.is_empty(), "no alyosha ROMs");
    eprintln!();
    eprintln!("{:<56} result", "ROM");
    let mut counts = [0usize; 4];
    for rel in &roms {
        let outcome = run_jsmolka(&root().join(rel), frames_for(rel), pass_mode(rel));
        print_row(rel, &outcome);
        tally(&mut counts, &outcome);
    }
    summarize(counts);
    // Soft gate: every ROM must at least load/run (no UNSUPPORTED from missing files).
    assert_eq!(counts[3], 0, "alyosha hit UNSUPPORTED");
}
