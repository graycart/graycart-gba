//! mgba-emu/suite - timing/DMA stretch (source only until `suite.gba` is built).
//!
//! Upstream: [mgba-emu/suite](https://github.com/mgba-emu/suite) @ `e6942030`.
//! Fixtures: `tests/fixtures/mgba-suite/`. A skip is not a pass.

use crate::harness::fixture_root;
use std::path::PathBuf;

fn root() -> PathBuf {
    fixture_root().join("mgba-suite")
}

#[test]
fn mgba_suite_source_present() {
    let dir = root();
    assert!(
        dir.is_dir(),
        "missing mgba-suite source under tests/fixtures/mgba-suite/"
    );
    let suite_gba = dir.join("suite.gba");
    if suite_gba.exists() {
        eprintln!("mgba-suite: suite.gba present - wire a matrix runner next");
    } else {
        eprintln!("mgba-suite: source only (no suite.gba); harness skips scoring");
    }
}

#[test]
#[ignore = "mgba suite.gba absent; enable when built"]
fn mgba_suite_matrix() {
    let suite = root().join("suite.gba");
    assert!(
        suite.exists(),
        "build suite.gba from tests/fixtures/mgba-suite/ before enabling this matrix"
    );
}
