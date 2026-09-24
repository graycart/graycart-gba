//! Headless CLI. Window path is feature-gated (`frontend`).

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use graycart_gba::Machine;
use graycart_gba::compat::{CGB_AUDIO_UNIMPLEMENTED, machine_line, run_sm83_file};
use graycart_gba::debug::{MachineDebug, cpu_result_line, live_cpu_line, summary_frames};
use graycart_gba::ppu::sprite_count;

#[cfg(feature = "frontend")]
mod play_shell {
    pub use graycart_gba::frontend::shell::run;
}

fn main() -> ExitCode {
    match run(&env::args().skip(1).collect::<Vec<_>>()) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    #[cfg(feature = "frontend")]
    if args.is_empty() {
        return match play_shell::run() {
            Ok(()) => Ok(ExitCode::SUCCESS),
            Err(err) if err == "no file selected" => Ok(ExitCode::SUCCESS),
            Err(err) => Err(err),
        };
    }

    let opts = parse(args)?;
    let carts = list_carts(&opts.path)?;
    let mut lines = Vec::new();
    let mut reports = Vec::new();
    let mut apu_rom_warned = false;
    let mut debug_failed = false;

    for cart in &carts {
        let bytes = fs::read(cart).map_err(|err| err.to_string())?;
        if is_gameboy(cart) {
            let frames = opts.frames.unwrap_or(1);
            let handoff = run_sm83_file(cart, frames)?;
            if opts.path.is_dir() {
                let name = cart
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("?");
                lines.push(format!(
                    "gba-debug: smoke file={name} frames={} fault=none",
                    handoff.frames
                ));
            }
            lines.push(machine_line("sm83", Some(&handoff)));
            lines.push(CGB_AUDIO_UNIMPLEMENTED.to_string());
            lines.push(format!(
                "gba-debug: sm83 frames={} arm_opcodes={}",
                handoff.frames, handoff.arm_opcodes
            ));
            continue;
        }
        if bytes.len() >= 0xC0 {
            let mut machine = Machine::open(cart)?;
            if opts.debug {
                let _passed = machine.run_debug(opts.frames);
            } else {
                machine.run_frames(opts.frames.expect("frames required without --debug"));
            }
            machine.flush_save()?;
            let report_frames = machine.frames_done().max(1);
            let frames = summary_frames(report_frames);
            if opts.path.is_dir() {
                let name = cart
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("?");
                lines.push(format!(
                    "gba-debug: smoke file={name} frames={report_frames} fault=none"
                ));
            }
            let debug = MachineDebug::absent();
            if !apu_rom_warned {
                apu_rom_warned = true;
                if !audio_test_rom_present() {
                    machine
                        .bus
                        .warn_lines
                        .push("gba-debug: warn apu rom skipped".to_string());
                }
            }
            lines.push(machine_line("arm7", None));
            lines.push(CGB_AUDIO_UNIMPLEMENTED.to_string());
            lines.extend(machine.bus.warn_lines.iter().cloned());
            lines.push(format!(
                "gba-debug: save kind={}",
                machine.bus.save_kind().name()
            ));
            for frame in &frames {
                let mut summary = debug.summary_lines(*frame);
                summary[0] = live_cpu_line(
                    *frame,
                    machine.cpu.exec_pc,
                    machine.cpu.cpsr(),
                    machine.idle,
                    machine.bus.halted,
                    machine.bus.irq.ime(),
                    machine.bus.irq.ie(),
                    machine.bus.irq.iff(),
                );
                let mode = machine.bus.dispcnt() & 7;
                let sprites = sprite_count(machine.bus.oam());
                summary[1] = format!("gba-debug: ppu frame={frame} mode={mode} sprites={sprites}");
                summary[2] = machine.bus.dma_debug_line(*frame);
                summary[7] = machine.bus.apu.health_line(*frame, &machine.bus.timers);
                lines.extend(summary);
                lines.push(machine.bus.wait_line(*frame, machine.cycles));
                lines.push(format!(
                    "gba-debug: timer frame={} t0={} c0={} t1={} c1={} t2={} c2={} t3={} c3={}",
                    frame,
                    machine.bus.timers.counter(0),
                    u8::from(machine.bus.timers.cascade(0)),
                    machine.bus.timers.counter(1),
                    u8::from(machine.bus.timers.cascade(1)),
                    machine.bus.timers.counter(2),
                    u8::from(machine.bus.timers.cascade(2)),
                    machine.bus.timers.counter(3),
                    u8::from(machine.bus.timers.cascade(3)),
                ));
                lines.push(format!(
                    "gba-debug: irq frame={} ie=0x{:04X} if=0x{:04X} ime={}",
                    frame,
                    machine.bus.irq.ie(),
                    machine.bus.irq.iff(),
                    u8::from(machine.bus.irq.ime()),
                ));
            }
            let op = machine
                .error
                .as_ref()
                .map(|err| err.to_string())
                .unwrap_or_else(|| machine.cpu.last_op.to_string());
            let result = cpu_result_line(
                machine.cpu.reg(12),
                machine.cpu.reg(7),
                machine.cpu.exec_pc,
                &op,
                machine.idle,
            );
            if opts.debug && result.contains("result=FAIL") {
                debug_failed = true;
            }
            lines.push(result);
            let apu_line = machine.bus.apu.av_line(&machine.bus.timers);
            reports.push(debug.av_report_live(
                report_frames,
                machine.bus.dispcnt(),
                &machine.ppu.pixels,
                machine.ppu.nonzero(),
                &apu_line,
                machine.bus.save_kind().name(),
            ));
        } else {
            let report_frames = opts.frames.unwrap_or(1);
            let frames = summary_frames(report_frames);
            if opts.path.is_dir() {
                let name = cart
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("?");
                lines.push(format!(
                    "gba-debug: smoke file={name} frames={report_frames} fault=none"
                ));
            }
            let debug = MachineDebug::absent();
            for frame in &frames {
                lines.extend(debug.summary_lines(*frame));
            }
            reports.push(debug.av_report(report_frames));
        }
    }

    if opts.debug {
        let mut err = io::stderr().lock();
        for line in &lines {
            writeln!(err, "{line}").map_err(|err| err.to_string())?;
        }
        let mut log = String::new();
        for line in &lines {
            log.push_str(line);
            log.push('\n');
        }
        for report in &reports {
            log.push_str(report);
        }
        fs::write("gba-debug.log", log).map_err(|err| err.to_string())?;

        let mut out = io::stdout().lock();
        for report in &reports {
            write!(out, "{report}").map_err(|err| err.to_string())?;
        }
    }
    if debug_failed {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

struct Opts {
    path: PathBuf,
    /// Cap. `None` runs until the CPU passes, faults, or halts forever.
    frames: Option<u32>,
    debug: bool,
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut path = None;
    let mut frames = None;
    let mut debug = false;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--frames" {
            let value = args
                .get(i + 1)
                .ok_or_else(|| usage("missing --frames <n>"))?;
            let n: u32 = value
                .parse()
                .map_err(|_| usage("frames must be a positive integer"))?;
            if n == 0 {
                return Err(usage("frames must be at least 1"));
            }
            frames = Some(n);
            i += 2;
            continue;
        }
        if arg == "--debug" || arg == "--debug=trace" {
            debug = true;
            i += 1;
            continue;
        }
        if arg.starts_with('-') {
            return Err(usage(&format!("unknown flag {arg}")));
        }
        if path.is_some() {
            return Err(usage("one rom or directory"));
        }
        path = Some(PathBuf::from(arg));
        i += 1;
    }
    let path = path.ok_or_else(|| usage("missing <rom-or-directory>"))?;
    if !debug && frames.is_none() {
        return Err(usage("missing --frames <n>"));
    }
    Ok(Opts {
        path,
        frames,
        debug,
    })
}

fn usage(why: &str) -> String {
    format!(
        "{why}\n\
graycart-gba <rom-or-directory> --frames <n> [--debug]\n\
graycart-gba <rom-or-directory> --debug [--frames <n>]"
    )
}

fn is_gameboy(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "gb" | "gbc"
    )
}

fn list_carts(path: &Path) -> Result<Vec<PathBuf>, String> {
    if !path.exists() {
        return Err(format!("not found: {}", path.display()));
    }
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    let mut carts = Vec::new();
    for entry in fs::read_dir(path).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let file = entry.path();
        if !file.is_file() {
            continue;
        }
        let ext = file
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == "gba" || ext == "gb" || ext == "gbc" {
            carts.push(file);
        }
    }
    carts.sort();
    if carts.is_empty() {
        return Err(format!("no carts in {}", path.display()));
    }
    Ok(carts)
}

/// True when a vendored `gba-audio-test`-style ROM exists under `tests/fixtures`.
fn audio_test_rom_present() -> bool {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    fn walk(dir: &Path) -> bool {
        let Ok(entries) = fs::read_dir(dir) else {
            return false;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if walk(&path) {
                    return true;
                }
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if name.contains("gba-audio-test") || name.contains("gba_audio_test") {
                return true;
            }
        }
        false
    }
    walk(&root)
}
