//! CPU integration tests — pipeline / exception stubs + jsmolka fixture hygiene.
//!
//! Cited: graycart-gba test strategy §4.1 / §5.1 (jsmolka gates, Outcome enum)
//!   Project store: `docs/graycart-gba/07-test-strategy.md`
//! Cited: jsmolka/gba-tests (MIT) — ARM/Thumb P1 exit ROMs
//!   https://github.com/jsmolka/gba-tests

mod harness;

use std::path::Path;

use graycart_gba::bus::{CpuMem, FlatRam};
use graycart_gba::cpu::{
    plan_entry, ExceptionKind, ExceptionModeBits, IsaState, Pipeline, VECTOR_BASE,
};

#[test]
fn exception_then_refill_from_vector_stub() {
    // Simulated BIOS vector: IRQ vector word is a branch placeholder encoding.
    let mut mem = FlatRam::with_base(VECTOR_BASE, 0x40);
    mem.write32(0x18, 0xEA00_0000); // B .+8 style placeholder
    mem.write32(0x1C, 0xE1A0_0000);
    mem.write32(0x20, 0xE1A0_0001);

    let cpsr = u32::from(ExceptionModeBits::User as u8);
    let plan = plan_entry(ExceptionKind::Irq, cpsr, 0x0800_0000);
    assert_eq!(plan.pc, 0x18);

    let mut pipe = Pipeline::new(IsaState::Thumb, 0x0800_0100);
    pipe.apply_exception_vector(plan.pc);
    pipe.refill(&mut mem);

    assert_eq!(pipe.isa, IsaState::Arm);
    assert_eq!(pipe.decode.unwrap().addr, 0x18);
    assert_eq!(pipe.decode.unwrap().raw, 0xEA00_0000);
    assert_eq!(pipe.fetch.unwrap().addr, 0x1C);
    assert_eq!(pipe.fetch_pc, 0x20);
}

#[test]
fn jsmolka_license_stub_present() {
    let license = Path::new("tests/fixtures/jsmolka/LICENSE");
    let readme = Path::new("tests/fixtures/jsmolka/README.md");
    assert!(
        license.is_file(),
        "missing {} — P0/P1 license stub required",
        license.display()
    );
    assert!(readme.is_file(), "missing {}", readme.display());
    let text = std::fs::read_to_string(license).expect("read LICENSE");
    assert!(
        text.contains("Julian Smolka") || text.contains("MIT"),
        "jsmolka LICENSE should retain upstream MIT attribution"
    );
}

#[test]
fn jsmolka_arm_thumb_fixture_dirs_exist() {
    for rel in ["tests/fixtures/jsmolka/arm", "tests/fixtures/jsmolka/thumb"] {
        let p = Path::new(rel);
        assert!(p.is_dir(), "missing fixture dir {rel}");
        assert!(
            p.join("README.md").is_file(),
            "missing {rel}/README.md license/path stub"
        );
    }
}

/// P1 fixture presence — vendored MIT prebuilts (workstream C).
#[test]
fn jsmolka_arm_rom_present() {
    let path = Path::new("tests/fixtures/jsmolka/arm/arm.gba");
    assert!(
        path.is_file(),
        "missing {} — vendor from https://github.com/jsmolka/gba-tests",
        path.display()
    );
}

#[test]
fn jsmolka_thumb_rom_present() {
    let path = Path::new("tests/fixtures/jsmolka/thumb/thumb.gba");
    assert!(
        path.is_file(),
        "missing {} — vendor from https://github.com/jsmolka/gba-tests",
        path.display()
    );
}

#[test]
fn harness_outcome_labels() {
    assert_eq!(harness::Outcome::Pass.label(), "PASS");
    assert_eq!(harness::Outcome::Fail("x".into()).label(), "FAIL");
    assert_eq!(harness::Outcome::Timeout.label(), "TIMEOUT");
    assert_eq!(
        harness::Outcome::Unsupported("no rom".into()).label(),
        "UNSUPPORTED"
    );
    assert_eq!(
        harness::Outcome::Skipped("apparatus".into()).label(),
        "SKIPPED"
    );
    assert_eq!(harness::Outcome::Fail("n".into()).detail(), "n");
    let _ = harness::RomLaunchMode::BiosHle;
    let _ = harness::RomLaunchMode::BiosLle;
    let _ = harness::RomLaunchMode::Multiboot;
}

/// Opt-in pointer to the shared roms harness (D/E). Prefer
/// `cargo test --test roms -- --ignored` for the real matrix.
#[test]
#[ignore = "use tests/roms jsmolka matrix — load+oracle live there"]
fn jsmolka_arm_gate_placeholder() {
    let path = Path::new("tests/fixtures/jsmolka/arm/arm.gba");
    assert!(
        path.is_file(),
        "arm.gba should be vendored; run roms::jsmolka matrix for PASS/FAIL/TIMEOUT"
    );
}
