//! Shared conformance ROM runner (jsmolka-style + PPU hash + MGBA printf).
//!
//! Fixtures live under `tests/fixtures/`. Outcomes match the GB harness:
//! `PASS` / `FAIL` / `TIMEOUT` / `UNSUPPORTED`.

use graycart_gba::{Machine, frame_hash};
use std::path::{Path, PathBuf};

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

/// How a jsmolka-family ROM reports failure.
#[derive(Debug, Clone, Copy)]
pub enum JsmolkaPass {
    /// Idle with `r12 == 0` (arm / memory / bios / save / most forks).
    R12,
    /// Idle with `r7 == 0` (`thumb.gba` stores the failing id in r7).
    ThumbR7,
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn print_row(name: &str, outcome: &Outcome) {
    let detail = outcome.detail();
    if detail.is_empty() {
        eprintln!("{:<56} {}", name, outcome.label());
    } else {
        eprintln!("{:<56} {}  ({detail})", name, outcome.label());
    }
}

pub fn summarize(counts: [usize; 4]) {
    eprintln!();
    eprintln!(
        "summary: PASS={} FAIL={} TIMEOUT={} UNSUPPORTED={}",
        counts[0], counts[1], counts[2], counts[3]
    );
}

pub fn tally(counts: &mut [usize; 4], outcome: &Outcome) {
    match outcome {
        Outcome::Pass => counts[0] += 1,
        Outcome::Fail(_) => counts[1] += 1,
        Outcome::Timeout => counts[2] += 1,
        Outcome::Unsupported(_) => counts[3] += 1,
    }
}

/// List every `.gba` under `root` (relative paths), skipping `unsafe/` trees.
pub fn list_gba_rel(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if !root.is_dir() {
        return out;
    }
    fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.is_dir() {
                if path.file_name().and_then(|s| s.to_str()) == Some("unsafe") {
                    continue;
                }
                walk(&path, base, out);
            } else if path.extension().and_then(|s| s.to_str()) == Some("gba") {
                if let Ok(rel) = path.strip_prefix(base) {
                    out.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    walk(root, root, &mut out);
    out.sort();
    out
}

fn load(path: &Path) -> Result<Machine, String> {
    if !path.exists() {
        return Err(format!("missing {}", path.display()));
    }
    let rom = std::fs::read(path).map_err(|e| e.to_string())?;
    Ok(Machine::from_rom(rom))
}

fn jsmolka_passed(machine: &Machine, mode: JsmolkaPass) -> bool {
    if !machine.idle || machine.error.is_some() || machine.cpu.faults != 0 {
        return false;
    }
    match mode {
        JsmolkaPass::R12 => machine.cpu.reg(12) == 0,
        JsmolkaPass::ThumbR7 => machine.cpu.reg(7) == 0,
    }
}

fn fail_detail(machine: &Machine) -> String {
    format!(
        "idle={} r12={} r7={} pc={:#010X} op={} err={:?}",
        machine.idle,
        machine.cpu.reg(12),
        machine.cpu.reg(7),
        machine.cpu.exec_pc,
        machine.cpu.last_op,
        machine.error.as_ref().map(|e| e.to_string())
    )
}

/// Run until idle or `frames` cap; score jsmolka r12/r7 pass.
pub fn run_jsmolka(path: &Path, frames: u32, mode: JsmolkaPass) -> Outcome {
    let mut machine = match load(path) {
        Ok(m) => m,
        Err(e) => return Outcome::Unsupported(e),
    };
    machine.run_frames(frames);
    if jsmolka_passed(&machine, mode) {
        Outcome::Pass
    } else if machine.error.is_some() || machine.cpu.faults != 0 || machine.idle {
        Outcome::Fail(fail_detail(&machine))
    } else {
        Outcome::Timeout
    }
}

/// jsmolka PPU ROM: idle + optional pixel checks + settled frame hash.
pub fn run_jsmolka_ppu(
    path: &Path,
    frames: u32,
    expected_hash: &str,
    spot: Option<[(usize, u16); 2]>,
) -> Outcome {
    let mut machine = match load(path) {
        Ok(m) => m,
        Err(e) => return Outcome::Unsupported(e),
    };
    machine.run_frames(frames);
    if !machine.idle || machine.error.is_some() || machine.cpu.faults != 0 {
        return if machine.idle {
            Outcome::Fail(fail_detail(&machine))
        } else {
            Outcome::Timeout
        };
    }
    if machine.ppu.nonzero() == 0 {
        return Outcome::Fail("blank framebuffer".into());
    }
    if let Some(spots) = spot {
        for (idx, want) in spots {
            let got = machine.ppu.pixels[idx];
            if got != want {
                return Outcome::Fail(format!("pixel[{idx}]=0x{got:04X} want 0x{want:04X}"));
            }
        }
    }
    let hash = frame_hash(&machine.ppu.pixels);
    if hash == expected_hash {
        Outcome::Pass
    } else {
        Outcome::Fail(format!("hash={hash} want={expected_hash}"))
    }
}

/// Run `frames`, then score MGBA printf when the ROM uses the debug port.
///
/// Many NBA/Hades ROMs use libgba `consoleDemoInit` (on-screen text only). Those
/// report [`Outcome::Unsupported`] until we scrape the tile console.
pub fn run_mgba_printf(path: &Path, frames: u32) -> Outcome {
    let mut machine = match load(path) {
        Ok(m) => m,
        Err(e) => return Outcome::Unsupported(e),
    };
    machine.run_frames(frames);
    if machine.error.is_some() || machine.cpu.faults != 0 {
        return Outcome::Fail(fail_detail(&machine));
    }
    let log = machine.bus.mgba_log.join("\n");
    if log.is_empty() {
        return Outcome::Unsupported(
            "on-screen libgba console (no MGBA debug-port traffic)".into(),
        );
    }
    if log.to_ascii_lowercase().contains("congratulations") {
        return Outcome::Pass;
    }
    // Hades: `Total: n/n`
    for line in machine.bus.mgba_log.iter().rev() {
        if let Some(rest) = line.strip_prefix("Total:") {
            let parts: Vec<_> = rest
                .split(|c: char| c == '/' || c.is_whitespace())
                .filter(|s| !s.is_empty())
                .collect();
            if parts.len() >= 2
                && let (Ok(a), Ok(b)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>())
            {
                return if a == b && b > 0 {
                    Outcome::Pass
                } else {
                    Outcome::Fail(format!("Total {a}/{b}"))
                };
            }
        }
    }
    if log.contains("FAIL") {
        Outcome::Fail(log.chars().take(120).collect())
    } else {
        Outcome::Timeout
    }
}
