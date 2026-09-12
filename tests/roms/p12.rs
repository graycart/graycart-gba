//! P12 supersede cutover gates — dual-run plan + intentional dep + no UI coupling.
//!
//! Cited: PHASES P12 · 08 §3.13 · product-decision-supersede-gb.md
//!   Project store: `docs/graycart-gba/PHASES.md`
//! Note: keep jsmolka arm/thumb/memory green; do not invent P13.

use graycart_gba::compat::{GRAYCART_DEP_LABEL, GRAYCART_GIT_REV};
use std::path::{Path, PathBuf};

/// Documented P12 exit: product handoff; lib reuse intentional.
pub const P12_EXIT: &str =
    "P12: graycart-gba is long-term 8-bit+GBA app; dual-run→default-gba; graycart dep stays";

const HOST_ONLY_CRATES: &[&str] = &["eframe", "egui", "cpal", "rfd"];

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn p12_exit_note() {
    assert!(P12_EXIT.contains("P12"));
    assert!(P12_EXIT.contains("default-gba") || P12_EXIT.contains("8-bit"));
    eprintln!("G12-docs: {P12_EXIT}");
    eprintln!("G12-audit-dep: {GRAYCART_DEP_LABEL} @ {GRAYCART_GIT_REV}");
}

#[test]
fn g12_plan_doc_present() {
    let path = crate_root().join("docs/supersede-cutover.md");
    assert!(
        path.is_file(),
        "missing {} — G12-plan dual-run doc required",
        path.display()
    );
    let text = std::fs::read_to_string(&path).expect("read supersede-cutover");
    assert!(
        text.contains("Dual-run") || text.contains("dual-run"),
        "cutover doc must describe dual-run → default-gba"
    );
    assert!(
        text.contains("product-decision") || text.contains("Dave"),
        "cutover doc must cite Dave product decision (G12-dave)"
    );
    assert!(
        text.contains("library") || text.contains("lib"),
        "cutover doc must keep lib reuse intentional"
    );
}

#[test]
fn g12_audit_dep_pin_matches_cargo() {
    assert_eq!(GRAYCART_GIT_REV.len(), 40);
    assert!(GRAYCART_GIT_REV.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(GRAYCART_DEP_LABEL.contains("graycart"));

    let cargo = std::fs::read_to_string(crate_root().join("Cargo.toml")).expect("Cargo.toml");
    assert!(
        cargo.contains("graycart") && cargo.contains(GRAYCART_GIT_REV),
        "Cargo.toml must keep graycart git dep pinned to GRAYCART_GIT_REV (G12-audit-dep)"
    );
    assert!(
        !cargo.contains("gb-core"),
        "interim whole-crate pin expected until S12 extract; do not fake gb-core here"
    );
}

#[test]
fn g12_audit_no_host_ui_in_lib_cores() {
    let src = crate_root().join("src");
    let mut offenders = Vec::new();
    walk_rs(&src, &mut |path: &Path, text: &str| {
        // Host stack lives in frontend/ + main.rs only.
        // Use Path components (not `/`-only string prefix) so Windows CI matches.
        let rel = path.strip_prefix(&src).unwrap_or(path);
        let rel_s = rel.to_string_lossy();
        let in_frontend = rel
            .components()
            .next()
            .is_some_and(|c| c.as_os_str() == "frontend");
        if in_frontend || rel == Path::new("main.rs") {
            return;
        }
        for crate_name in HOST_ONLY_CRATES {
            // Match `use …eframe` / `eframe::` / dependency-style mentions in code.
            let patterns = [
                format!("use {crate_name}"),
                format!("{crate_name}::"),
                format!("extern crate {crate_name}"),
            ];
            for p in &patterns {
                if text.contains(p) {
                    offenders.push(format!("{}: {}", rel_s, p));
                }
            }
        }
    });
    assert!(
        offenders.is_empty(),
        "G12-audit-ui: host UI crates leaked into lib cores:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn g12_conformance_board_mentions_p12() {
    let path = crate_root().join("docs/conformance.md");
    let text = std::fs::read_to_string(&path).expect("conformance.md");
    assert!(
        text.contains("P12") && text.contains("0.1.3"),
        "conformance board must record P12 + crate 0.1.3 (G12-docs)"
    );
}

fn walk_rs(dir: &Path, visit: &mut dyn FnMut(&Path, &str)) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rs(&path, visit);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(text) = std::fs::read_to_string(&path) {
                visit(&path, &text);
            }
        }
    }
}
