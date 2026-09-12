//! Shared Blargg / Mooneye runners through [`CompatMachine`] (P11).
//!
//! Cited: graycart-gb `tests/roms/harness.rs` (serial + RAM oracles)
//!   https://github.com/graycart/graycart-gb/blob/main/tests/roms/harness.rs
//! Cited: Blargg / Mooneye via 09-dmg-cgb-compatibility.md §9
//!   Project store: `docs/graycart-gba/09-dmg-cgb-compatibility.md`
//! Note: outcomes never silent-pass on missing fixtures.

use graycart_gba::compat::{CompatMachine, CompatSilicon};
use std::path::Path;

/// Result of one conformance ROM run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail(String),
    Timeout,
    Unsupported(String),
}

impl Outcome {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail(_) => "FAIL",
            Self::Timeout => "TIMEOUT",
            Self::Unsupported(_) => "UNSUPPORTED",
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            Self::Pass | Self::Timeout => "",
            Self::Fail(s) | Self::Unsupported(s) => s,
        }
    }
}

/// Run a Blargg ROM under the given silicon preference.
pub fn run_blargg(path: &Path, step_limit: u32, silicon: CompatSilicon) -> Outcome {
    if !path.is_file() {
        return Outcome::Unsupported(format!("missing {}", path.display()));
    }
    let mut m = CompatMachine::new();
    if let Err(e) = m.load_rom_path_as(path, silicon) {
        return Outcome::Unsupported(e);
    }
    for i in 0..step_limit {
        if let Some(o) = blargg_result(&m) {
            return o;
        }
        if let Err(e) = m.step_instruction() {
            return Outcome::Unsupported(format!("step {i}: {e}"));
        }
    }
    blargg_result(&m).unwrap_or(Outcome::Timeout)
}

/// Mooneye fibonacci register oracle (same contract as graycart-gb).
pub fn run_mooneye(path: &Path, step_limit: u32, silicon: CompatSilicon) -> Outcome {
    if !path.is_file() {
        return Outcome::Unsupported(format!("missing {}", path.display()));
    }
    let mut m = CompatMachine::new();
    if let Err(e) = m.load_rom_path_as(path, silicon) {
        return Outcome::Unsupported(e);
    }
    let mut same_pc = 0u32;
    let mut last_pc = m.cpu().pc;
    for i in 0..step_limit {
        if mooneye_success(&m) {
            return Outcome::Pass;
        }
        if let Err(e) = m.step_instruction() {
            return Outcome::Unsupported(format!("step {i}: {e}"));
        }
        let cpu = m.cpu();
        if cpu.pc == last_pc {
            same_pc = same_pc.saturating_add(1);
        } else {
            same_pc = 0;
            last_pc = cpu.pc;
        }
        if !cpu.halted && same_pc >= 64 && !mooneye_success(&m) {
            return Outcome::Fail(format!(
                "loop PC=${:04X} BC={:02X}{:02X} DE={:02X}{:02X} HL={:02X}{:02X}",
                cpu.pc, cpu.b, cpu.c, cpu.d, cpu.e, cpu.h, cpu.l
            ));
        }
        let text = m.serial_text();
        if text.contains("Passed") {
            return Outcome::Pass;
        }
        if text.contains("Failed") {
            return Outcome::Fail(summarize_serial(&text));
        }
    }
    if mooneye_success(&m) {
        Outcome::Pass
    } else {
        Outcome::Timeout
    }
}

fn blargg_result(m: &CompatMachine) -> Option<Outcome> {
    let text = m.serial_text();
    if text.contains("Passed") {
        return Some(Outcome::Pass);
    }
    if text.contains("Failed") {
        return Some(Outcome::Fail(summarize_serial(&text)));
    }
    if m.read8(0xA001) == 0xDE && m.read8(0xA002) == 0xB0 && m.read8(0xA003) == 0x61 {
        let status = m.read8(0xA000);
        if status == 0x80 {
            return None;
        }
        if status == 0 {
            return Some(Outcome::Pass);
        }
        return Some(Outcome::Fail(format!("ram status={status:#04x}")));
    }
    None
}

fn mooneye_success(m: &CompatMachine) -> bool {
    let cpu = m.cpu();
    cpu.b == 3 && cpu.c == 5 && cpu.d == 8 && cpu.e == 13 && cpu.h == 21 && cpu.l == 34
}

fn summarize_serial(text: &str) -> String {
    let t = text.trim();
    if t.len() <= 80 {
        t.to_string()
    } else {
        format!("{}…", &t[..80])
    }
}
