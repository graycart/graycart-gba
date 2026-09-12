//! P4 PPU harness gates — jsmolka ppu goldens + Tonc hash stubs.
//!
//! Cited: jsmolka/gba-tests (MIT) — ppu/hello, shades, stripes
//!   https://github.com/jsmolka/gba-tests
//! Cited: graycart-gba PHASES P4 · 07/11/12 test docs
//!   Project store: `docs/graycart-gba/PHASES.md`
//! Note: hash format = final-frame SHA-256 of 240×160×3 RGB888.
//! Keep arm/thumb/memory green; never fake PASS.

use crate::harness::{fixture_path, load_gba, GbaTestRom, Outcome, RomLaunchMode, RunBudget};
use graycart_gba::Gba;
use std::path::Path;

/// Settle frames before hashing a visual ROM (idle loop after setup).
const PPU_SETTLE_FRAMES: u64 = 3;

struct JsmolkaPpuRom {
    id: &'static str,
    relative_path: &'static str,
    golden_path: &'static str,
}

impl GbaTestRom for JsmolkaPpuRom {
    fn id(&self) -> &str {
        self.id
    }

    fn relative_path(&self) -> &str {
        self.relative_path
    }

    fn launch_mode(&self) -> RomLaunchMode {
        RomLaunchMode::BiosHle
    }

    fn run(&self, rom: &[u8]) -> Outcome {
        let mut gba = match load_gba(rom, RomLaunchMode::BiosHle) {
            Ok(g) => g,
            Err(o) => return o,
        };
        // Visual ROMs idle in a tight branch; detect idle PC without Mode4 gate.
        run_until_idle_pc(&mut gba, RunBudget::default());
        gba.run_frames(PPU_SETTLE_FRAMES);
        compare_golden(&gba, self.golden_path)
    }
}

/// Step until Decode PC holds, ignoring DISPCNT Mode4 (PPU demos use Mode 0/4).
fn run_until_idle_pc(gba: &mut Gba, budget: RunBudget) {
    let mut last_pc: Option<u32> = None;
    let mut same = 0u32;
    for _ in 0..budget.max_steps {
        gba.step_instruction();
        let pc = gba.decode_pc();
        if pc == last_pc && pc.is_some() {
            same = same.saturating_add(1);
            if same >= budget.idle_hold {
                return;
            }
        } else {
            same = 0;
            last_pc = pc;
        }
    }
}

const HELLO: JsmolkaPpuRom = JsmolkaPpuRom {
    id: "jsmolka/ppu/hello",
    relative_path: "jsmolka/ppu/hello/hello.gba",
    golden_path: "tests/golden/jsmolka-ppu/hello.hash",
};
const SHADES: JsmolkaPpuRom = JsmolkaPpuRom {
    id: "jsmolka/ppu/shades",
    relative_path: "jsmolka/ppu/shades/shades.gba",
    golden_path: "tests/golden/jsmolka-ppu/shades.hash",
};
const STRIPES: JsmolkaPpuRom = JsmolkaPpuRom {
    id: "jsmolka/ppu/stripes",
    relative_path: "jsmolka/ppu/stripes/stripes.gba",
    golden_path: "tests/golden/jsmolka-ppu/stripes.hash",
};

const JSMOLKA_PPU: &[JsmolkaPpuRom] = &[HELLO, SHADES, STRIPES];

fn compare_golden(gba: &Gba, golden_path: &str) -> Outcome {
    let path = Path::new(golden_path);
    if !path.is_file() {
        return Outcome::Skipped(format!("golden absent: {golden_path}"));
    }
    let expected = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => return Outcome::Unsupported(format!("read golden: {e}")),
    };
    let got_hash = gba.frame_hash_sha256();
    let expect_hash = expected
        .lines()
        .find_map(|l| l.strip_prefix("sha256="))
        .unwrap_or("")
        .trim();
    if expect_hash.is_empty() {
        return Outcome::Unsupported("golden missing sha256= line".into());
    }
    if got_hash == expect_hash {
        Outcome::Pass
    } else {
        Outcome::Fail(format!(
            "hash mismatch: got {got_hash}, expect {expect_hash}"
        ))
    }
}

#[test]
fn p4_fixture_stubs_present() {
    assert!(Path::new("tests/fixtures/jsmolka/LICENSE").is_file());
    for rom in JSMOLKA_PPU {
        let dir = fixture_path(rom.relative_path())
            .parent()
            .expect("parent")
            .to_path_buf();
        assert!(dir.is_dir(), "missing {}", dir.display());
        assert!(
            dir.join("README.md").is_file(),
            "missing README in {}",
            dir.display()
        );
        assert!(
            fixture_path(rom.relative_path()).is_file(),
            "missing vendored {}",
            rom.relative_path
        );
    }
    assert!(Path::new("tests/fixtures/tonc/LICENSE").is_file());
    assert!(Path::new("tests/golden/jsmolka-ppu/README.md").is_file());
    assert!(Path::new("tests/golden/tonc/README.md").is_file());
}

/// G4-jsmolka-ppu: FB hash must match committed goldens (default CI).
#[test]
fn p4_jsmolka_ppu_goldens() {
    for rom in JSMOLKA_PPU {
        assert!(
            Path::new(rom.golden_path).is_file(),
            "missing golden {} — bake with --ignored p4_bake",
            rom.golden_path
        );
        let outcome = crate::harness::run_test_rom(rom);
        eprintln!("{:<40} {}  ({})", rom.id, outcome.label(), outcome.detail());
        assert_eq!(
            outcome.label(),
            "PASS",
            "{} expected PASS, got {} ({})",
            rom.id,
            outcome.label(),
            outcome.detail()
        );
    }
}

/// Capture helper — ignored; writes goldens when run with --ignored.
#[test]
#[ignore = "P4 bake goldens: cargo test -p graycart-gba --test roms -- --ignored p4_bake"]
fn p4_bake_jsmolka_ppu_goldens() {
    std::fs::create_dir_all("tests/golden/jsmolka-ppu").expect("mkdir");
    for rom in JSMOLKA_PPU {
        let bytes = std::fs::read(fixture_path(rom.relative_path())).expect("rom");
        let mut gba = load_gba(&bytes, RomLaunchMode::BiosHle).expect("load");
        run_until_idle_pc(&mut gba, RunBudget::default());
        gba.run_frames(PPU_SETTLE_FRAMES);
        let body = gba.ppu.hash_file_body(PPU_SETTLE_FRAMES);
        std::fs::write(rom.golden_path, &body).expect("write golden");
        eprintln!("wrote {} -> {}", rom.golden_path, gba.frame_hash_sha256());
    }
}

/// G4-tonc: ignored until ≥3 CC0 demos + goldens exist.
#[test]
#[ignore = "P4 G4-tonc: demos/goldens not present yet — path stubs only"]
fn p4_tonc_hash_gate() {
    let tonc = Path::new("tests/fixtures/tonc");
    let demos: Vec<_> = std::fs::read_dir(tonc)
        .expect("tonc dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "gba"))
        .collect();
    assert!(
        demos.len() >= 3,
        "need ≥3 Tonc .gba demos under tests/fixtures/tonc/"
    );
    panic!("Tonc hash runner not fully wired — replace when demos land");
}
