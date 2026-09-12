//! Console diagnostics for black-screen / boot bring-up (GB-style UX).
//!
//! Cited: graycart-gb CLI verbosity / `--trace` / runtime diag env flags
//!   https://github.com/graycart/graycart-gb (src/main.rs, frontend/launch.rs)
//! Cited: GBATEK — cartridge header / IRQ / DMA / LCD (field names only)
//!   https://problemkaputt.de/gbatek.htm
//! Note: Default quiet. Breadcrumbs go to stderr with `gba-debug:` prefix.
//!   No commercial ROMs required — validate with jsmolka / synthetic fixtures.

#[cfg(test)]
mod tests;

use crate::bios::BiosMode;
use crate::cart::detect::SaveKind;
use crate::cart::header::CartHeader;
use crate::cpu::{cpsr, Mode};
use crate::dma::DmaRunReport;
use crate::hw::PowerMode;
use crate::ppu::FRAME_CYCLES;
use crate::RomLaunchMode;
use std::env;
use std::fmt::Write as _;

/// Env var enabling debug breadcrumbs (`1` / `true` / `yes` / non-empty).
pub const ENV_DEBUG: &str = "GRAYCART_DEBUG";
/// Env var enabling trace-level samples (same truthy parsing as [`ENV_DEBUG`]).
pub const ENV_TRACE: &str = "GRAYCART_TRACE";

/// Greppable stderr prefix for all breadcrumbs.
pub const LOG_PREFIX: &str = "gba-debug:";

/// How chatty the breadcrumb stream is (independent of [`Verbosity`] load lines).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum DebugLevel {
    /// No breadcrumbs (default).
    #[default]
    Off,
    /// Load/reset/events + periodic CPU/PPU samples + stuck detection.
    Debug,
    /// Debug plus per-instruction samples (rate-limited) / headless `--trace N`.
    Trace,
}

/// GB-style load/boot summary verbosity (stdout), separate from breadcrumbs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verbosity {
    Quiet,
    #[default]
    Normal,
    Verbose,
}

/// Resolved debug / verbosity options (CLI + env).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugConfig {
    pub level: DebugLevel,
    pub verbosity: Verbosity,
    /// When set, headless path dumps this many insn lines then exits (GB `--trace`).
    pub trace_steps: Option<u64>,
    /// Emit a `cpu`/`ppu` sample every N frames while debug is on (default 60).
    pub period_frames: u64,
    /// Same decode PC for this many consecutive frames → `stuck` (default 120).
    pub stuck_frames: u64,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            level: DebugLevel::Off,
            verbosity: Verbosity::Normal,
            trace_steps: None,
            period_frames: 60,
            stuck_frames: 120,
        }
    }
}

impl DebugConfig {
    /// Merge CLI overrides with env (`GRAYCART_DEBUG` / `GRAYCART_TRACE`).
    #[must_use]
    pub fn from_env_and_cli(
        cli_debug: bool,
        cli_trace: bool,
        trace_steps: Option<u64>,
        verbosity: Verbosity,
    ) -> Self {
        let mut cfg = Self {
            verbosity,
            trace_steps,
            ..Self::default()
        };
        if env_flag(ENV_TRACE) || cli_trace || trace_steps.is_some() {
            cfg.level = DebugLevel::Trace;
        } else if env_flag(ENV_DEBUG) || cli_debug {
            cfg.level = DebugLevel::Debug;
        }
        cfg
    }

    #[inline]
    #[must_use]
    pub fn enabled(self) -> bool {
        self.level != DebugLevel::Off
    }
}

fn env_flag(name: &str) -> bool {
    match env::var(name) {
        Ok(v) => {
            let t = v.trim();
            if t.is_empty() {
                return false;
            }
            !matches!(
                t.to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        }
        Err(_) => false,
    }
}

thread_local! {
    static CAPTURED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    static CAPTURING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Emit one breadcrumb line to stderr (or capture buffer under test).
pub fn log_line(msg: &str) {
    let line = format!("{LOG_PREFIX} {msg}");
    if CAPTURING.get() {
        CAPTURED.with(|c| c.borrow_mut().push(line));
    } else {
        eprintln!("{line}");
    }
}

#[cfg(test)]
fn capture_start() {
    CAPTURED.with(|c| c.borrow_mut().clear());
    CAPTURING.set(true);
}

#[cfg(test)]
fn capture_take() -> Vec<String> {
    CAPTURING.set(false);
    CAPTURED.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

/// Runtime counters for periodic / stuck / rate-limited event logs.
#[derive(Debug, Clone)]
pub struct DebugTracker {
    pub config: DebugConfig,
    cycles_in_frame: u64,
    frames: u64,
    last_period_frame: u64,
    last_pc: Option<u32>,
    same_pc_frames: u64,
    stuck_announced: bool,
    irq_logged: u64,
    dma_logged: u64,
    halt_logged: bool,
    last_waitcnt: Option<u16>,
    last_power: PowerMode,
    /// Remaining insn lines for headless `--trace N` (stdout).
    pub trace_remaining: Option<u64>,
}

impl Default for DebugTracker {
    fn default() -> Self {
        Self::new(DebugConfig::default())
    }
}

impl DebugTracker {
    #[must_use]
    pub fn new(config: DebugConfig) -> Self {
        Self {
            trace_remaining: config.trace_steps,
            config,
            cycles_in_frame: 0,
            frames: 0,
            last_period_frame: 0,
            last_pc: None,
            same_pc_frames: 0,
            stuck_announced: false,
            irq_logged: 0,
            dma_logged: 0,
            halt_logged: false,
            last_waitcnt: None,
            last_power: PowerMode::Run,
        }
    }

    #[inline]
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.config.enabled()
    }

    /// Apply config after construction (CLI → machine).
    pub fn set_config(&mut self, config: DebugConfig) {
        self.trace_remaining = config.trace_steps;
        self.config = config;
    }

    pub fn log_rom_load(&self, rom: &[u8], header: Option<&CartHeader>, save: SaveKind) {
        if !self.enabled() {
            return;
        }
        let (title, code, entry, chk) = match header {
            Some(h) => (
                h.title_str(),
                h.game_code_str(),
                format!("{:08X}", h.entry),
                h.checksum_ok(rom),
            ),
            None => ("?".into(), "????".into(), "--------".into(), false),
        };
        log_line(&format!(
            "rom size={} title={title:?} code={code} entry=0x{entry} checksum_ok={chk} save={save:?}",
            rom.len()
        ));
    }

    pub fn log_reset(&self, mode: RomLaunchMode, bios: BiosMode, pc: u32, cpsr_v: u32) {
        if !self.enabled() {
            return;
        }
        let thumb = (cpsr_v & cpsr::T) != 0;
        let cpu_mode = Mode::from_bits(cpsr_v & 0x1F)
            .map(|m| format!("{m:?}"))
            .unwrap_or_else(|| format!("bad({:02X})", cpsr_v & 0x1F));
        log_line(&format!(
            "bios launch={mode:?} bios_mode={bios:?} entry_pc=0x{pc:08X} cpsr=0x{cpsr_v:08X} mode={cpu_mode} thumb={thumb}"
        ));
    }

    pub fn log_error(&self, msg: &str) {
        // Errors always print when debug is on; also useful without debug for host.
        if self.enabled() {
            log_line(&format!("error {msg}"));
        }
    }

    /// After DMA drain — rate-limited.
    pub fn on_dma_report(&mut self, report: &DmaRunReport, dma: &crate::dma::Dma) {
        if !self.enabled() || report.completed.is_empty() {
            return;
        }
        for &id in &report.completed {
            self.dma_logged = self.dma_logged.saturating_add(1);
            if self.dma_logged > 16 && !self.dma_logged.is_multiple_of(256) {
                continue;
            }
            let ch = dma.channel(id);
            log_line(&format!(
                "dma ch={} timing={:?} sad=0x{:08X} dad=0x{:08X} count={}{}",
                id.index(),
                ch.start_timing(),
                ch.sad,
                ch.dad,
                ch.count,
                if self.dma_logged > 16 {
                    format!(" (n={})", self.dma_logged)
                } else {
                    String::new()
                }
            ));
        }
        for rej in &report.rejected {
            log_line(&format!("dma reject {rej:?}"));
        }
    }

    /// Called once per retired instruction quantum with cycle delta.
    pub fn on_step(
        &mut self,
        gba: &crate::Gba,
        step_cycles: u64,
        outcome: crate::cpu::StepOutcome,
    ) {
        if !self.enabled() {
            return;
        }

        self.maybe_trace_insn(gba, outcome);
        self.watch_waitcnt(gba);
        self.watch_power(gba);

        self.cycles_in_frame = self.cycles_in_frame.saturating_add(step_cycles);
        while self.cycles_in_frame >= u64::from(FRAME_CYCLES) {
            self.cycles_in_frame -= u64::from(FRAME_CYCLES);
            self.on_frame(gba);
        }
    }

    fn maybe_trace_insn(&mut self, gba: &crate::Gba, outcome: crate::cpu::StepOutcome) {
        let want_line = match &mut self.trace_remaining {
            Some(n) if *n > 0 => {
                *n -= 1;
                true
            }
            _ if self.config.level == DebugLevel::Trace => {
                // Rate-limit continuous trace: first 64, then every 4096th step.
                let step = gba.cycles;
                step < 64 || step.is_multiple_of(4096)
            }
            _ => false,
        };
        if !want_line {
            return;
        }
        let pc = gba.decode_pc().unwrap_or_else(|| gba.cpu.regs.pc());
        let cpsr_v = gba.cpu.regs.cpsr();
        let line = format!(
            "insn cycles={} pc=0x{pc:08X} cpsr=0x{cpsr_v:08X} thumb={} outcome={outcome:?}",
            gba.cycles,
            gba.cpu.regs.thumb()
        );
        if self.config.trace_steps.is_some() {
            // GB dumps --trace to stdout.
            println!("{LOG_PREFIX} {line}");
        } else {
            log_line(&line);
        }
    }

    fn watch_waitcnt(&mut self, gba: &crate::Gba) {
        let w = gba.hw.read_waitcnt();
        if self.last_waitcnt != Some(w) {
            self.last_waitcnt = Some(w);
            log_line(&format!("waitcnt=0x{w:04X}"));
        }
    }

    fn watch_power(&mut self, gba: &crate::Gba) {
        let p = gba.hw.power;
        if p == self.last_power {
            return;
        }
        let prev = self.last_power;
        self.last_power = p;
        match p {
            PowerMode::Halt => {
                self.halt_logged = true;
                log_line(&format!(
                    "halt enter ie=0x{:04X} if=0x{:04X} ime={}",
                    gba.irq.read_ie(),
                    gba.irq.read_if(),
                    gba.irq.ime() as u8
                ));
            }
            PowerMode::Stop => {
                log_line("stop enter");
            }
            PowerMode::Run if matches!(prev, PowerMode::Halt | PowerMode::Stop) => {
                log_line(&format!(
                    "halt wake from={prev:?} ie=0x{:04X} if=0x{:04X}",
                    gba.irq.read_ie(),
                    gba.irq.read_if()
                ));
            }
            PowerMode::Run => {}
        }
    }

    fn on_frame(&mut self, gba: &crate::Gba) {
        self.frames = self.frames.saturating_add(1);
        let pc = gba.decode_pc().unwrap_or_else(|| gba.cpu.regs.pc());
        if self.last_pc == Some(pc) {
            self.same_pc_frames = self.same_pc_frames.saturating_add(1);
        } else {
            self.last_pc = Some(pc);
            self.same_pc_frames = 0;
            self.stuck_announced = false;
        }
        if self.same_pc_frames >= self.config.stuck_frames && !self.stuck_announced {
            self.stuck_announced = true;
            log_line(&format!(
                "stuck pc=0x{pc:08X} frames={} cpsr=0x{:08X} ie=0x{:04X} if=0x{:04X} ime={} power={:?} dispcnt=0x{:04X} vcount={}",
                self.same_pc_frames,
                gba.cpu.regs.cpsr(),
                gba.irq.read_ie(),
                gba.irq.read_if(),
                gba.irq.ime() as u8,
                gba.hw.power,
                gba.ppu.regs.dispcnt,
                gba.ppu.timing.vcount
            ));
        }

        let period = self.config.period_frames.max(1);
        if self.frames == 1 || self.frames.saturating_sub(self.last_period_frame) >= period {
            self.last_period_frame = self.frames;
            self.log_periodic(gba);
        }
    }

    fn log_periodic(&self, gba: &crate::Gba) {
        let pc = gba.decode_pc().unwrap_or_else(|| gba.cpu.regs.pc());
        let cpsr_v = gba.cpu.regs.cpsr();
        let mode = Mode::from_bits(cpsr_v & 0x1F)
            .map(|m| format!("{m:?}"))
            .unwrap_or_else(|| "?".into());
        let mut buf = String::new();
        let _ = write!(
            &mut buf,
            "cpu frame={} pc=0x{pc:08X} cpsr=0x{cpsr_v:08X} mode={mode} thumb={} i_mask={} ime={} ie=0x{:04X} if=0x{:04X} power={:?}",
            self.frames,
            gba.cpu.regs.thumb(),
            (cpsr_v & cpsr::I) != 0,
            gba.irq.ime() as u8,
            gba.irq.read_ie(),
            gba.irq.read_if(),
            gba.hw.power
        );
        log_line(&buf);
        log_line(&format!(
            "ppu frame={} mode={} vcount={} dispcnt=0x{:04X} forced_blank={}",
            self.frames,
            gba.ppu.regs.bg_mode(),
            gba.ppu.timing.vcount,
            gba.ppu.regs.dispcnt,
            gba.ppu.regs.forced_blank()
        ));
    }

    pub fn on_irq_serviced(&mut self, handler: u32, ie: u16, if_: u16, ime: bool) {
        if !self.enabled() {
            return;
        }
        self.irq_logged = self.irq_logged.saturating_add(1);
        if self.irq_logged > 8 && !self.irq_logged.is_multiple_of(64) {
            return;
        }
        log_line(&format!(
            "irq n={} handler=0x{handler:08X} ie=0x{ie:04X} if=0x{if_:04X} ime={}",
            self.irq_logged, ime as u8
        ));
    }

    /// True when headless `--trace N` has exhausted its dump budget.
    #[must_use]
    pub fn trace_exhausted(&self) -> bool {
        matches!(self.trace_remaining, Some(0))
    }
}

/// Format a short stdout load summary (GB Quiet/Normal/Verbose parity).
pub fn print_load_summary(
    verbosity: Verbosity,
    path: &str,
    rom: &[u8],
    header: Option<&CartHeader>,
    save: SaveKind,
    launch: RomLaunchMode,
) {
    match verbosity {
        Verbosity::Quiet => {
            let title = header.map(CartHeader::title_str).unwrap_or_default();
            println!("loaded {}", if title.is_empty() { path } else { &title });
        }
        Verbosity::Normal | Verbosity::Verbose => {
            println!("loaded {path}");
            println!("size: {} bytes", rom.len());
            if let Some(h) = header {
                println!(
                    "title: {}  code: {}  entry: 0x{:08X}  checksum_ok: {}",
                    h.title_str(),
                    h.game_code_str(),
                    h.entry,
                    h.checksum_ok(rom)
                );
            }
            println!("save: {save:?}");
            println!("boot: {launch:?}");
            if verbosity == Verbosity::Verbose {
                println!("debug: breadcrumbs on stderr when --debug / GRAYCART_DEBUG");
            }
        }
    }
}
