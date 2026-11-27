//! jsmolka gba-tests - primary must-gate suite.
//!
//! Upstream: [jsmolka/gba-tests](https://github.com/jsmolka/gba-tests) @ `a7113b67`.
//! Fixtures: `tests/fixtures/jsmolka/` (no `unsafe.gba`).

use crate::harness::{
    JsmolkaPass, Outcome, fixture_root, print_row, run_jsmolka, run_jsmolka_ppu, summarize, tally,
};
use std::path::PathBuf;

const FRAMES_CPU: u32 = 60;
const FRAMES_SAVE: u32 = 90;
const FRAMES_PPU: u32 = 5;

fn root() -> PathBuf {
    fixture_root().join("jsmolka")
}

fn path(rel: &str) -> PathBuf {
    root().join(rel)
}

/// CPU / bus / bios / save - r12 (or thumb r7) idle pass.
const CPU_SUITE: &[(&str, JsmolkaPass, u32)] = &[
    ("arm/arm.gba", JsmolkaPass::R12, FRAMES_CPU),
    ("thumb/thumb.gba", JsmolkaPass::ThumbR7, FRAMES_CPU),
    ("memory/memory.gba", JsmolkaPass::R12, FRAMES_CPU),
    ("bios/bios.gba", JsmolkaPass::R12, FRAMES_CPU),
    ("save/none.gba", JsmolkaPass::R12, FRAMES_SAVE),
    ("save/sram.gba", JsmolkaPass::R12, FRAMES_SAVE),
    ("save/flash64.gba", JsmolkaPass::R12, FRAMES_SAVE),
    ("save/flash128.gba", JsmolkaPass::R12, FRAMES_SAVE),
];

#[test]
fn jsmolka_fixtures_present() {
    let mut missing = Vec::new();
    for (rel, _, _) in CPU_SUITE {
        let p = path(rel);
        if !p.exists() {
            missing.push(p.display().to_string());
        }
    }
    for rel in ["ppu/hello.gba", "ppu/shades.gba", "ppu/stripes.gba"] {
        let p = path(rel);
        if !p.exists() {
            missing.push(p.display().to_string());
        }
    }
    assert!(
        missing.is_empty(),
        "missing jsmolka fixtures (see tests/fixtures/jsmolka/README.md):\n{}",
        missing.join("\n")
    );
}

#[test]
fn jsmolka_must_matrix() {
    eprintln!();
    eprintln!("{:<56} result", "ROM");
    let mut counts = [0usize; 4];

    for &(rel, mode, frames) in CPU_SUITE {
        let outcome = run_jsmolka(&path(rel), frames, mode);
        print_row(rel, &outcome);
        tally(&mut counts, &outcome);
        assert!(
            matches!(outcome, Outcome::Pass),
            "jsmolka must-gate failed: {rel} ? {}",
            outcome.label()
        );
    }

    // PPU: settled frame hashes (same values as tests/ppu_*.rs).
    #[allow(clippy::type_complexity)]
    let ppu: &[(&str, &str, Option<[(usize, u16); 2]>)] = &[
        (
            "ppu/hello.gba",
            "56cd131fb3915fe7e410be228a8c09e99132064799f148583636ca75745bedf7",
            None,
        ),
        (
            "ppu/shades.gba",
            "da5336215e6c56d30146c42d81ad828a819214c307682903a5bcc87be8c0538e",
            Some([(0, 0), (16, 0x0800)]),
        ),
        (
            "ppu/stripes.gba",
            "4600c4f2749300b40f34dea969165d4d08d652543620076fb5cae249426f238d",
            Some([(0, 0x560B), (8, 0x6290)]),
        ),
    ];
    for &(rel, hash, spot) in ppu {
        let outcome = run_jsmolka_ppu(&path(rel), FRAMES_PPU, hash, spot);
        print_row(rel, &outcome);
        tally(&mut counts, &outcome);
        assert!(
            matches!(outcome, Outcome::Pass),
            "jsmolka PPU must-gate failed: {rel} ? {} ({})",
            outcome.label(),
            outcome.detail()
        );
    }

    summarize(counts);
}
