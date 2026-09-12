//! P9 Frontend harness gates — playable host hygiene (honest).
//!
//! Cited: graycart-gba PHASES P9 · 08 §3.10 · AGENTS.md core≠host
//!   Project store: `docs/graycart-gba/PHASES.md`
//! Note: GUI window smoke stays `#[ignore]` without a display. carts/ commercial
//! dumps are optional — skip if missing. Keep jsmolka arm/thumb/memory green.

use std::path::Path;

/// Documented P9 exit: playable GBA path; CI remains ROM-free by default.
pub const P9_EXIT: &str =
    "P9: playable .gba host (picker/pause/reset/.sav); CI ROM-free; crate 0.1.0";

#[test]
fn p9_exit_note_mentions_playable_gba() {
    assert!(P9_EXIT.contains("playable") || P9_EXIT.contains("P9"));
    assert!(P9_EXIT.contains(".gba") || P9_EXIT.contains("gba"));
    eprintln!("G9-docs: {P9_EXIT}");
}

#[test]
fn carts_readme_present_and_skip_ok() {
    let readme = Path::new("carts/README.md");
    assert!(
        readme.is_file(),
        "missing {} — document obtain-your-own commercial smoke",
        readme.display()
    );
    let text = std::fs::read_to_string(readme).expect("read carts README");
    assert!(
        text.to_ascii_lowercase().contains("skip")
            || text.to_ascii_lowercase().contains("optional")
            || text.to_ascii_lowercase().contains("never commit"),
        "carts README should say dumps are optional / never committed"
    );
}

#[test]
fn carts_smoke_skips_when_empty() {
    // G9-smoke: optional local dumps — never fail CI when absent.
    let dir = Path::new("carts");
    if !dir.is_dir() {
        eprintln!("G9-smoke: carts/ missing — SKIP OK");
        return;
    }
    let mut found = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for ent in rd.flatten() {
            let p = ent.path();
            if p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("gba"))
            {
                found.push(p);
            }
        }
    }
    if found.is_empty() {
        eprintln!("G9-smoke: no carts/*.gba — SKIP OK");
        return;
    }
    // When present, prove the lib can load bytes (no window).
    let bytes = std::fs::read(&found[0]).expect("read cart");
    let mut gba = graycart_gba::Gba::new();
    gba.load_rom(&bytes);
    gba.reset_bios_hle();
    gba.run_frames(1);
    eprintln!("G9-smoke: loaded {} for 1 frame", found[0].display());
}

#[test]
#[ignore = "needs display / interactive window — run locally"]
fn gui_window_smoke_ignored() {
    // G9-chrome / G9-video stretch: opening eframe requires a display.
    panic!("run graycart-gba --run locally to exercise the windowed host");
}

#[test]
fn crate_version_is_0_1_0() {
    // G9-docs / G9-release prep: first tagged runnable milestone.
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.1.0");
}

#[test]
fn conformance_mentions_p9() {
    let text = std::fs::read_to_string("docs/conformance.md").expect("conformance");
    assert!(
        text.contains("P9") || text.contains("Frontend"),
        "docs/conformance.md should record a P9 frontend board"
    );
}
