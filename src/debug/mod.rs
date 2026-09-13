//! Console diagnostics for black-screen / boot bring-up (GB-style UX).
//!
//! Cited: graycart-gb CLI verbosity / `--trace` / runtime diag env flags
//!   https://github.com/graycart/graycart-gb (src/main.rs, frontend/launch.rs)
//! Cited: graycart-gb Debug Monitor health / ApuDebug peaks (adapted for GBA console)
//!   https://github.com/graycart/graycart-gb (src/frontend/debug/ui/health.rs, src/debug/machine.rs)
//! Cited: GBATEK — cartridge header / IRQ / DMA / LCD / FIFO / memory map (field names only)
//!   https://problemkaputt.de/gbatek.htm
//! Note: Default quiet. `--debug` = summary (problems + aggregates + AV health), not DMA spam.
//!   `--debug=trace` / `--trace` for verbose DMA/IRQ/insn. Prefix `gba-debug:`.
//!   No commercial ROMs required — validate with jsmolka / synthetic fixtures.

#[cfg(test)]
mod tests;

use crate::apu::{fifo_route_label, pwm_rate_hz, ApuHealthPeriod, DC_WARN_ABS};
use crate::bios::{BiosMode, HLE_IRQ_RETURN};
use crate::bus::region::{self, Region};
use crate::cart::detect::SaveKind;
use crate::cart::header::CartHeader;
use crate::cpu::{cpsr, Mode};
use crate::dma::{DmaRunReport, StartTiming};
use crate::hw::PowerMode;
use crate::ppu::health::{
    analyze_framebuffer, backdrop_from_palette, blend_label, blend_mode, layer_enable_label,
    layer_mask, mosaic_active, objwin_active,
};
use crate::ppu::FRAME_CYCLES;
use crate::RomLaunchMode;
use std::collections::BTreeMap;
use std::env;
use std::fmt::Write as _;

/// Env var enabling debug breadcrumbs (`1` / `true` / `yes` / `summary` / `trace`).
pub const ENV_DEBUG: &str = "GRAYCART_DEBUG";
/// Env var enabling trace-level samples (same truthy parsing as [`ENV_DEBUG`]).
pub const ENV_TRACE: &str = "GRAYCART_TRACE";

/// Greppable stderr prefix for all breadcrumbs.
pub const LOG_PREFIX: &str = "gba-debug:";

/// VRAM halfword writes in one period above this → VRAM write-storm warn.
/// (~tile churn beyond ordinary streaming; independent of OAM.)
pub const VIDEO_WRITE_STORM_VRAM: u64 = 12_000;
/// Full OAM is 1 KiB = 512 halfword stores. Commercial titles often rewrite
/// all of OAM every frame (FireRed ≈ 30720 / 60-frame period) — that is normal.
/// Only flag OAM when sustained rate exceeds ~2× full OAM rewrite per frame.
pub const OAM_HALFWORDS: u64 = 512;
pub const OAM_STORM_PER_FRAME: u64 = OAM_HALFWORDS * 2;
/// FIFO empty drains in one period above this → loud warn.
pub const FIFO_EMPTY_STORM: u64 = 64;
/// Host ring underrun events in one report above this → warn.
pub const HOST_UNDERRUN_WARN: u64 = 1;
/// Consecutive all-black frames before `warn ppu all-black`.
pub const BLACK_FRAME_WARN_FRAMES: u64 = 30;
/// Consecutive backdrop-only frames before `warn ppu backdrop-only`.
pub const BACKDROP_ONLY_WARN_FRAMES: u64 = 60;

/// How chatty the breadcrumb stream is (independent of [`Verbosity`] load lines).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum DebugLevel {
    /// No breadcrumbs (default).
    #[default]
    Off,
    /// Bring-up summary: rom/bios, loud faults, blanking, stuck, dma/irq aggregates, AV health.
    Debug,
    /// Summary plus per-event DMA/IRQ (rate-limited) and insn samples / `--trace N`.
    Trace,
}

impl DebugLevel {
    /// Parse CLI/env level tokens (`summary`/`debug`/`1` → Debug; `trace` → Trace).
    #[must_use]
    pub fn parse_token(raw: &str) -> Option<Self> {
        let t = raw.trim().to_ascii_lowercase();
        match t.as_str() {
            "" | "0" | "false" | "no" | "off" => Some(Self::Off),
            "1" | "true" | "yes" | "on" | "summary" | "debug" => Some(Self::Debug),
            "trace" | "verbose" => Some(Self::Trace),
            _ => None,
        }
    }
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
    /// Emit a `cpu`/`ppu` / dma·irq summary every N frames while debug is on (default 60).
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
    ///
    /// `cli_debug_level`: `Some` when `--debug` / `--debug=…` was passed.
    /// `cli_trace`: bare `--trace` or `--trace N` (implies Trace).
    #[must_use]
    pub fn from_env_and_cli(
        cli_debug_level: Option<DebugLevel>,
        cli_trace: bool,
        trace_steps: Option<u64>,
        verbosity: Verbosity,
    ) -> Self {
        let mut cfg = Self {
            verbosity,
            trace_steps,
            ..Self::default()
        };
        let env_level = env::var(ENV_DEBUG)
            .ok()
            .as_deref()
            .and_then(DebugLevel::parse_token);
        if env_flag(ENV_TRACE)
            || cli_trace
            || trace_steps.is_some()
            || matches!(cli_debug_level, Some(DebugLevel::Trace))
            || matches!(env_level, Some(DebugLevel::Trace))
        {
            cfg.level = DebugLevel::Trace;
        } else if matches!(cli_debug_level, Some(DebugLevel::Debug))
            || matches!(env_level, Some(DebugLevel::Debug))
        {
            cfg.level = DebugLevel::Debug;
        } else if matches!(cli_debug_level, Some(DebugLevel::Off))
            || matches!(env_level, Some(DebugLevel::Off))
        {
            cfg.level = DebugLevel::Off;
        }
        cfg
    }

    #[inline]
    #[must_use]
    pub fn enabled(self) -> bool {
        self.level != DebugLevel::Off
    }

    #[inline]
    #[must_use]
    pub fn is_trace(self) -> bool {
        self.level == DebugLevel::Trace
    }
}

/// Parse `--debug` or `--debug=<level>`. Returns `None` if `arg` is not a debug flag.
pub fn parse_debug_arg(arg: &str) -> Option<Result<DebugLevel, String>> {
    if arg == "--debug" {
        return Some(Ok(DebugLevel::Debug));
    }
    let rest = arg.strip_prefix("--debug=")?;
    match DebugLevel::parse_token(rest) {
        Some(DebugLevel::Off) => Some(Err("--debug=off is invalid; omit --debug for quiet".into())),
        Some(level) => Some(Ok(level)),
        None => Some(Err(format!(
            "invalid --debug={rest}; use summary|debug|trace"
        ))),
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

#[derive(Debug, Clone, Default)]
struct DmaCounters {
    total: u64,
    /// (channel index, timing discriminant) → count
    by_ch_timing: BTreeMap<(u8, u8), u64>,
}

#[derive(Debug, Clone, Default)]
struct SwiCounters {
    /// SWI number → count of unhandled hits
    by_num: BTreeMap<u8, u64>,
    /// Numbers already announced loudly at least once
    announced: BTreeMap<u8, u64>,
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
    irq_period: u64,
    dma_logged: u64,
    dma_counters: DmaCounters,
    dma_period: u64,
    swi: SwiCounters,
    halt_logged: bool,
    halt_frames: u64,
    halt_forever_announced: bool,
    last_waitcnt: Option<u16>,
    last_power: PowerMode,
    last_forced_blank: Option<bool>,
    last_dispcnt: Option<u16>,
    openbus_announced: bool,
    bios_vector_announced: bool,
    ie_nonzero_announced: bool,
    irq_trap_frames: u64,
    irq_trap_announced: bool,
    /// Remaining insn lines for headless `--trace N` (stdout).
    pub trace_remaining: Option<u64>,
    // --- AV health (summary) ---
    last_bg_mode: Option<u16>,
    last_layer_mask: Option<u8>,
    last_mosaic: Option<bool>,
    last_blend: Option<u16>,
    last_objwin: Option<bool>,
    last_vram_writes: u64,
    last_oam_writes: u64,
    last_palette_writes: u64,
    last_fifo_ua: u64,
    last_fifo_ub: u64,
    last_fifo_oa: u64,
    last_fifo_ob: u64,
    black_frames: u64,
    backdrop_only_frames: u64,
    black_announced: bool,
    host_underruns: u64,
    host_rate_hz: Option<u32>,
    host_rate_announced: bool,
    fifo_empty_announced: bool,
    clip_announced: bool,
    dc_announced: bool,
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
            irq_period: 0,
            dma_logged: 0,
            dma_counters: DmaCounters::default(),
            dma_period: 0,
            swi: SwiCounters::default(),
            halt_logged: false,
            halt_frames: 0,
            halt_forever_announced: false,
            last_waitcnt: None,
            last_power: PowerMode::Run,
            last_forced_blank: None,
            last_dispcnt: None,
            openbus_announced: false,
            bios_vector_announced: false,
            ie_nonzero_announced: false,
            irq_trap_frames: 0,
            irq_trap_announced: false,
            last_bg_mode: None,
            last_layer_mask: None,
            last_mosaic: None,
            last_blend: None,
            last_objwin: None,
            last_vram_writes: 0,
            last_oam_writes: 0,
            last_palette_writes: 0,
            last_fifo_ua: 0,
            last_fifo_ub: 0,
            last_fifo_oa: 0,
            last_fifo_ob: 0,
            black_frames: 0,
            backdrop_only_frames: 0,
            black_announced: false,
            host_underruns: 0,
            host_rate_hz: None,
            host_rate_announced: false,
            fifo_empty_announced: false,
            clip_announced: false,
            dc_announced: false,
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
        if self.enabled() {
            log_line(&format!("error {msg}"));
        }
    }

    /// After DMA drain — summary aggregates always; per-event only at Trace.
    pub fn on_dma_report(&mut self, report: &DmaRunReport, dma: &crate::dma::Dma) {
        if !self.enabled() || (report.completed.is_empty() && report.rejected.is_empty()) {
            return;
        }
        for &id in &report.completed {
            self.dma_logged = self.dma_logged.saturating_add(1);
            self.dma_period = self.dma_period.saturating_add(1);
            let ch = dma.channel(id);
            let timing = ch.start_timing();
            let key = (id.index() as u8, timing as u8);
            *self.dma_counters.by_ch_timing.entry(key).or_insert(0) += 1;
            self.dma_counters.total = self.dma_counters.total.saturating_add(1);

            if self.config.is_trace() {
                // First 4 events, then every 1024th — avoid Special-DMA floods.
                if self.dma_logged <= 4 || self.dma_logged.is_multiple_of(1024) {
                    log_line(&format!(
                        "dma ch={} timing={timing:?} sad=0x{:08X} dad=0x{:08X} count={} (n={})",
                        id.index(),
                        ch.sad,
                        ch.dad,
                        ch.count,
                        self.dma_logged
                    ));
                }
            }
        }
        for rej in &report.rejected {
            log_line(&format!("dma reject {rej:?}"));
        }
    }

    /// Called once per retired instruction quantum with cycle delta.
    pub fn on_step(
        &mut self,
        gba: &mut crate::Gba,
        step_cycles: u64,
        outcome: crate::cpu::StepOutcome,
    ) {
        if !self.enabled() {
            return;
        }

        self.maybe_trace_insn(gba, outcome);
        self.watch_waitcnt(gba);
        self.watch_power(gba);
        self.watch_pc_health(gba);

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
                self.halt_frames = 0;
                self.halt_forever_announced = false;
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

    /// Loud open-bus / unexpected BIOS-vector breadcrumbs (once until recovered).
    fn watch_pc_health(&mut self, gba: &crate::Gba) {
        let pc = gba.decode_pc().unwrap_or_else(|| gba.cpu.regs.pc());
        let region = region::decode(pc);

        let openbus = matches!(
            region,
            Region::UnusedLow
                | Region::UnusedHigh
                | Region::Palette
                | Region::Vram
                | Region::Oam
                | Region::GamePakSram
        );
        if openbus {
            if !self.openbus_announced {
                self.openbus_announced = true;
                log_line(&format!(
                    "warn openbus pc=0x{pc:08X} region={region:?} cpsr=0x{:08X} (PC runaway / non-exec fetch)",
                    gba.cpu.regs.cpsr()
                ));
            }
        } else {
            self.openbus_announced = false;
        }

        // Under BiosHle, executing BIOS ROM (except IRQ return sentinel) usually means
        // an unexpected vector into empty/stub BIOS space.
        let bios_hle = matches!(gba.bios.mode, BiosMode::Hle);
        if bios_hle && region == Region::Bios && pc != HLE_IRQ_RETURN {
            if !self.bios_vector_announced {
                self.bios_vector_announced = true;
                log_line(&format!(
                    "warn bios vector unexpected pc=0x{pc:08X} (BiosHle — empty BIOS fetch?)"
                ));
            }
        } else if region != Region::Bios {
            self.bios_vector_announced = false;
        }
    }

    fn on_frame(&mut self, gba: &mut crate::Gba) {
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

        self.watch_blanking(gba);
        self.watch_video_regs(gba);
        self.watch_frame_pixels(gba);
        self.watch_irq_trap(gba);
        self.watch_halt_forever(gba);

        let period = self.config.period_frames.max(1);
        if self.frames == 1 || self.frames.saturating_sub(self.last_period_frame) >= period {
            self.last_period_frame = self.frames;
            self.log_periodic(gba);
            self.log_dma_summary();
            self.log_irq_summary();
            self.log_swi_summary();
            self.log_apu_health(gba);
            self.log_ppu_health(gba);
        }
    }

    fn watch_blanking(&mut self, gba: &crate::Gba) {
        let blank = gba.ppu.regs.forced_blank();
        let dispcnt = gba.ppu.regs.dispcnt;
        if self.last_forced_blank != Some(blank) {
            let prev = self.last_forced_blank;
            self.last_forced_blank = Some(blank);
            if prev.is_some() || blank {
                // Always note first sample if already blank, and every edge after.
                log_line(&format!(
                    "ppu blank forced_blank={blank} dispcnt=0x{dispcnt:04X} mode={} vcount={}",
                    gba.ppu.regs.bg_mode(),
                    gba.ppu.timing.vcount
                ));
            }
        }
        // Surface first non-zero DISPCNT (LCD programmed) even without blank edge.
        if self.last_dispcnt != Some(dispcnt) {
            let prev = self.last_dispcnt;
            self.last_dispcnt = Some(dispcnt);
            if prev == Some(0) && dispcnt != 0 {
                log_line(&format!(
                    "ppu dispcnt 0x0000→0x{dispcnt:04X} forced_blank={blank} mode={}",
                    gba.ppu.regs.bg_mode()
                ));
            }
        }
    }

    /// Mode / layer / mosaic / blend / OBJWIN edges (summary one-liners).
    fn watch_video_regs(&mut self, gba: &crate::Gba) {
        let regs = &gba.ppu.regs;
        let mode = regs.bg_mode();
        if self.last_bg_mode != Some(mode) {
            let prev = self.last_bg_mode;
            self.last_bg_mode = Some(mode);
            if let Some(p) = prev {
                log_line(&format!(
                    "ppu mode {p}→{mode} dispcnt=0x{:04X} layers={}",
                    regs.dispcnt,
                    layer_enable_label(regs)
                ));
            }
        }
        let layers = layer_mask(regs.dispcnt);
        if self.last_layer_mask != Some(layers) {
            let prev = self.last_layer_mask;
            self.last_layer_mask = Some(layers);
            if prev.is_some() {
                log_line(&format!(
                    "ppu layers {} dispcnt=0x{:04X}",
                    layer_enable_label(regs),
                    regs.dispcnt
                ));
            }
        }
        let mosaic = mosaic_active(regs);
        if self.last_mosaic != Some(mosaic) {
            self.last_mosaic = Some(mosaic);
            if mosaic {
                log_line(&format!("ppu mosaic on mosaic=0x{:04X}", regs.mosaic));
            }
        }
        let blend = blend_mode(regs);
        if self.last_blend != Some(blend) {
            let prev = self.last_blend;
            self.last_blend = Some(blend);
            if prev.is_some() || blend != 0 {
                log_line(&format!(
                    "ppu blend mode={} bldcnt=0x{:04X}",
                    blend_label(blend),
                    regs.bldcnt
                ));
            }
        }
        let objwin = objwin_active(regs);
        if self.last_objwin != Some(objwin) {
            self.last_objwin = Some(objwin);
            if objwin {
                log_line("ppu objwin on");
            }
        }
    }

    /// Backdrop-only / all-black heuristics once per frame (cheap subsample).
    fn watch_frame_pixels(&mut self, gba: &crate::Gba) {
        if gba.ppu.regs.forced_blank() {
            return;
        }
        let bd = backdrop_from_palette(&gba.bus.palette);
        let health = analyze_framebuffer(&gba.ppu.fb, bd);
        if health.is_all_black() {
            self.black_frames = self.black_frames.saturating_add(1);
            if self.black_frames >= BLACK_FRAME_WARN_FRAMES && !self.black_announced {
                self.black_announced = true;
                log_line(&format!(
                    "warn ppu all-black frames={} black_pct={}% layers={} mode={} dispcnt=0x{:04X} (silent black screen?)",
                    self.black_frames,
                    health.black_pct(),
                    layer_enable_label(&gba.ppu.regs),
                    gba.ppu.regs.bg_mode(),
                    gba.ppu.regs.dispcnt
                ));
            }
        } else {
            self.black_frames = 0;
            self.black_announced = false;
        }
        if health.is_backdrop_only() {
            self.backdrop_only_frames = self.backdrop_only_frames.saturating_add(1);
            if self.backdrop_only_frames == BACKDROP_ONLY_WARN_FRAMES {
                log_line(&format!(
                    "warn ppu backdrop-only frames={} bd=0x{bd:04X} layers={} mode={}",
                    self.backdrop_only_frames,
                    layer_enable_label(&gba.ppu.regs),
                    gba.ppu.regs.bg_mode()
                ));
            }
        } else {
            self.backdrop_only_frames = 0;
        }
        // Trace-only denser frame pixel line.
        if self.config.is_trace() && self.frames.is_multiple_of(60) {
            log_line(&format!(
                "ppu pixels black={}% backdrop={}% other={} samples={}",
                health.black_pct(),
                health.backdrop_pct(),
                health.other,
                health.samples
            ));
        }
        let _ = health;
    }

    fn watch_irq_trap(&mut self, gba: &crate::Gba) {
        let ie = gba.irq.read_ie();
        let if_ = gba.irq.read_if();
        let ime = gba.irq.ime();
        if ie != 0 && !self.ie_nonzero_announced {
            self.ie_nonzero_announced = true;
            log_line(&format!(
                "irq ie set ie=0x{ie:04X} if=0x{if_:04X} ime={}",
                ime as u8
            ));
        }
        let pending_masked = (ie & if_) != 0;
        if pending_masked && !ime {
            self.irq_trap_frames = self.irq_trap_frames.saturating_add(1);
            if self.irq_trap_frames >= self.config.stuck_frames && !self.irq_trap_announced {
                self.irq_trap_announced = true;
                log_line(&format!(
                    "warn irq trap ie=0x{ie:04X} if=0x{if_:04X} ime=0 frames={} (IF pending, IME off)",
                    self.irq_trap_frames
                ));
            }
        } else {
            self.irq_trap_frames = 0;
            self.irq_trap_announced = false;
        }
    }

    fn watch_halt_forever(&mut self, gba: &crate::Gba) {
        if matches!(gba.hw.power, PowerMode::Halt | PowerMode::Stop) {
            self.halt_frames = self.halt_frames.saturating_add(1);
            if self.halt_frames >= self.config.stuck_frames && !self.halt_forever_announced {
                self.halt_forever_announced = true;
                log_line(&format!(
                    "warn halt forever frames={} power={:?} ie=0x{:04X} if=0x{:04X} ime={}",
                    self.halt_frames,
                    gba.hw.power,
                    gba.irq.read_ie(),
                    gba.irq.read_if(),
                    gba.irq.ime() as u8
                ));
            }
        } else {
            self.halt_frames = 0;
            self.halt_forever_announced = false;
        }
    }

    fn log_periodic(&self, gba: &crate::Gba) {
        let pc = gba.decode_pc().unwrap_or_else(|| gba.cpu.regs.pc());
        let cpsr_v = gba.cpu.regs.cpsr();
        let mode = Mode::from_bits(cpsr_v & 0x1F)
            .map(|m| format!("{m:?}"))
            .unwrap_or_else(|| "?".into());
        let region = region::decode(pc);
        let mut buf = String::new();
        let _ = write!(
            &mut buf,
            "cpu frame={} pc=0x{pc:08X} region={region:?} cpsr=0x{cpsr_v:08X} mode={mode} thumb={} i_mask={} ime={} ie=0x{:04X} if=0x{:04X} power={:?}",
            self.frames,
            gba.cpu.regs.thumb(),
            (cpsr_v & cpsr::I) != 0,
            gba.irq.ime() as u8,
            gba.irq.read_ie(),
            gba.irq.read_if(),
            gba.hw.power
        );
        log_line(&buf);
        let regs = &gba.ppu.regs;
        log_line(&format!(
            "ppu frame={} mode={} vcount={} dispcnt=0x{:04X} forced_blank={} layers={} mosaic={} blend={} objwin={}",
            self.frames,
            regs.bg_mode(),
            gba.ppu.timing.vcount,
            regs.dispcnt,
            regs.forced_blank(),
            layer_enable_label(regs),
            mosaic_active(regs) as u8,
            blend_label(blend_mode(regs)),
            objwin_active(regs) as u8
        ));
    }

    fn log_apu_health(&mut self, gba: &mut crate::Gba) {
        let mut period = gba.apu.health.take_period();
        let ua = gba.apu.fifos.a.underruns;
        let ub = gba.apu.fifos.b.underruns;
        let oa = gba.apu.fifos.a.overruns;
        let ob = gba.apu.fifos.b.overruns;
        period.underrun_a = ua.saturating_sub(self.last_fifo_ua);
        period.underrun_b = ub.saturating_sub(self.last_fifo_ub);
        period.overrun_a = oa.saturating_sub(self.last_fifo_oa);
        period.overrun_b = ob.saturating_sub(self.last_fifo_ob);
        self.last_fifo_ua = ua;
        self.last_fifo_ub = ub;
        self.last_fifo_oa = oa;
        self.last_fifo_ob = ob;
        let regs = &gba.apu.regs;
        let pwm = pwm_rate_hz(regs.pwm_resolution());
        let master = regs.master_enabled() as u8;
        let (dc_l, dc_r) = period.mean_dc();
        let ch_on = regs.channel_on & 0x0F;
        let psg_vol = regs.soundcnt_l & 0x77;
        log_line(&format!(
            "apu health frame={} master={master} pwm={pwm}Hz fifoA={} underrun={}/{} overrun={}/{} empty={}/{} lag={}/{} dma_req={}/{} peak=[{}..{}] dc≈[{dc_l},{dc_r}] clip={} extreme={} psg_on=0x{ch_on:X} psg_nr50=0x{psg_vol:02X} fifoB={}",
            self.frames,
            fifo_route_label(regs, true),
            period.underrun_a,
            period.underrun_b,
            period.overrun_a,
            period.overrun_b,
            period.empty_drain_a,
            period.empty_drain_b,
            period.refill_lag_a,
            period.refill_lag_b,
            period.dma_req_a,
            period.dma_req_b,
            period.peak_min,
            period.peak_max,
            period.clip_hits,
            period.extreme_hits,
            fifo_route_label(regs, false),
        ));
        if let Some(host) = self.host_rate_hz {
            let mismatch = if host == 0 {
                "host=0".into()
            } else {
                format!(
                    "host={host}Hz ratio={:.3}",
                    f64::from(pwm) / f64::from(host)
                )
            };
            log_line(&format!(
                "apu host frame={} pwm={pwm}Hz {mismatch} underrun_events={}",
                self.frames, self.host_underruns
            ));
        }
        self.emit_apu_warns(period, regs.master_enabled(), ch_on, psg_vol);
    }

    fn emit_apu_warns(&mut self, period: ApuHealthPeriod, master: bool, ch_on: u8, psg_vol: u16) {
        let empty = period.empty_drain_a + period.empty_drain_b;
        let under = period.underrun_a + period.underrun_b;
        if (empty >= FIFO_EMPTY_STORM || under >= FIFO_EMPTY_STORM) && !self.fifo_empty_announced {
            self.fifo_empty_announced = true;
            log_line(&format!(
                "warn apu fifo empty-drain/underrun period empty={empty} underrun={under} lag={}/{} (held last sample — pops/silence/stuck tone)",
                period.refill_lag_a, period.refill_lag_b
            ));
        } else if empty == 0 && under == 0 {
            self.fifo_empty_announced = false;
        }
        if period.overrun_a + period.overrun_b > 0 {
            log_line(&format!(
                "warn apu fifo overrun A={} B={} (FIFO overflow reset — DMA too fast?)",
                period.overrun_a, period.overrun_b
            ));
        }
        if period.clip_hits > 0 && !self.clip_announced {
            self.clip_announced = true;
            log_line(&format!(
                "warn apu clipping hits={} peak=[{}..{}] (loud saw/rumble risk)",
                period.clip_hits, period.peak_min, period.peak_max
            ));
        } else if period.clip_hits == 0 {
            self.clip_announced = false;
        }
        let (dc_l, dc_r) = period.mean_dc();
        if (dc_l.unsigned_abs() >= DC_WARN_ABS as u32 || dc_r.unsigned_abs() >= DC_WARN_ABS as u32)
            && !self.dc_announced
        {
            self.dc_announced = true;
            log_line(&format!(
                "warn apu dc bias L={dc_l} R={dc_r} (stuck latch / rumble)"
            ));
        } else if dc_l.unsigned_abs() < DC_WARN_ABS as u32
            && dc_r.unsigned_abs() < DC_WARN_ABS as u32
        {
            self.dc_announced = false;
        }
        if !master && (period.dma_req_a + period.dma_req_b > 0 || under > 0) {
            log_line(&format!(
                "warn apu master=0 but fifo traffic dma_req={}/{} underrun={under} (SOUNDCNT_X off)",
                period.dma_req_a, period.dma_req_b
            ));
        }
        if master && psg_vol == 0 && ch_on != 0 {
            log_line(&format!(
                "warn apu psg channels on=0x{ch_on:X} but NR50 vol=0 (silent PSG)"
            ));
        }
        if self.host_underruns >= HOST_UNDERRUN_WARN && self.frames.is_multiple_of(60) {
            log_line(&format!(
                "warn apu host underrun_events={} (cpal ring empty — pops)",
                self.host_underruns
            ));
        }
    }

    fn log_ppu_health(&mut self, gba: &crate::Gba) {
        let vram = gba
            .bus
            .vram_write_count
            .saturating_sub(self.last_vram_writes);
        let oam = gba.bus.oam_write_count.saturating_sub(self.last_oam_writes);
        let pal = gba
            .bus
            .palette_write_count
            .saturating_sub(self.last_palette_writes);
        self.last_vram_writes = gba.bus.vram_write_count;
        self.last_oam_writes = gba.bus.oam_write_count;
        self.last_palette_writes = gba.bus.palette_write_count;

        let bd = backdrop_from_palette(&gba.bus.palette);
        let pixels = analyze_framebuffer(&gba.ppu.fb, bd);
        let regs = &gba.ppu.regs;
        let period = self.config.period_frames.max(1);
        // One full OAM rewrite/frame is normal; storm = sustained >2× that.
        let oam_storm = OAM_STORM_PER_FRAME.saturating_mul(period);
        log_line(&format!(
            "ppu health frame={} mode={} layers={} blank={} mosaic={} blend={} objwin={} black={}% backdrop={}% writes vram={vram} oam={oam} pal={pal}",
            self.frames,
            regs.bg_mode(),
            layer_enable_label(regs),
            regs.forced_blank() as u8,
            mosaic_active(regs) as u8,
            blend_label(blend_mode(regs)),
            objwin_active(regs) as u8,
            pixels.black_pct(),
            pixels.backdrop_pct(),
        ));
        if vram >= VIDEO_WRITE_STORM_VRAM || oam > oam_storm {
            let kind = match (vram >= VIDEO_WRITE_STORM_VRAM, oam > oam_storm) {
                (true, true) => "vram+oam",
                (true, false) => "vram",
                _ => "oam",
            };
            log_line(&format!(
                "warn ppu write-storm kind={kind} vram={vram} oam={oam} pal={pal} (tile/OBJ churn)"
            ));
        }
    }

    /// Host/frontend audio telemetry (cpal underruns + device rate).
    pub fn on_host_audio(&mut self, host_hz: u32, underrun_events: u64, queue_frames: usize) {
        if !self.enabled() {
            return;
        }
        self.host_underruns = underrun_events;
        if self.host_rate_hz != Some(host_hz) {
            self.host_rate_hz = Some(host_hz);
            if !self.host_rate_announced {
                self.host_rate_announced = true;
                log_line(&format!(
                    "apu host rate={host_hz}Hz queue≈{queue_frames} (PWM resampled to device)"
                ));
            }
        }
    }

    fn log_dma_summary(&mut self) {
        if self.dma_period == 0 && self.dma_counters.total == 0 {
            return;
        }
        if self.dma_period == 0 {
            return;
        }
        let mut parts = String::new();
        for (&(ch, timing_d), &n) in &self.dma_counters.by_ch_timing {
            let timing = match timing_d {
                0 => StartTiming::Immediate,
                1 => StartTiming::VBlank,
                2 => StartTiming::HBlank,
                _ => StartTiming::Special,
            };
            if !parts.is_empty() {
                parts.push(' ');
            }
            let _ = write!(&mut parts, "ch{ch}/{timing:?}={n}");
        }
        log_line(&format!(
            "dma summary frame={} period={} total={} {parts}",
            self.frames, self.dma_period, self.dma_counters.total
        ));
        self.dma_period = 0;
    }

    fn log_irq_summary(&mut self) {
        if self.irq_period == 0 {
            return;
        }
        log_line(&format!(
            "irq summary frame={} period={} total={}",
            self.frames, self.irq_period, self.irq_logged
        ));
        self.irq_period = 0;
    }

    fn log_swi_summary(&mut self) {
        if self.swi.by_num.is_empty() {
            return;
        }
        let mut parts = String::new();
        for (&num, &n) in &self.swi.by_num {
            if !parts.is_empty() {
                parts.push(',');
            }
            let _ = write!(&mut parts, "0x{num:02X}:{n}");
        }
        // Only re-emit when counts grew since last period — always useful at frame 1+.
        log_line(&format!(
            "swi summary frame={} unhandled=[{parts}]",
            self.frames
        ));
    }

    pub fn on_irq_serviced(&mut self, handler: u32, ie: u16, if_: u16, ime: bool) {
        if !self.enabled() {
            return;
        }
        self.irq_logged = self.irq_logged.saturating_add(1);
        self.irq_period = self.irq_period.saturating_add(1);
        if !self.config.is_trace() {
            return;
        }
        if self.irq_logged > 8 && !self.irq_logged.is_multiple_of(256) {
            return;
        }
        log_line(&format!(
            "irq n={} handler=0x{handler:08X} ie=0x{ie:04X} if=0x{if_:04X} ime={}",
            self.irq_logged, ime as u8
        ));
    }

    /// BiosHle skipped an unimplemented SWI instead of vectoring into empty BIOS.
    ///
    /// First hit per SWI number is always loud; repeats are counted for `swi summary`.
    pub fn on_unhandled_swi(&mut self, number: u8, resume_pc: u32) {
        if !self.enabled() {
            return;
        }
        let count = self.swi.by_num.entry(number).or_insert(0);
        *count = count.saturating_add(1);
        let n = *count;
        let announced = self.swi.announced.entry(number).or_insert(0);
        let first = *announced == 0;
        *announced = announced.saturating_add(1);
        // Always loud on first occurrence of each SWI number; rare repeats stay visible.
        if first || n <= 3 || n.is_multiple_of(256) {
            log_line(&format!(
                "warn swi unhandled num=0x{number:02X} resume_pc=0x{resume_pc:08X} n={n} (BiosHle stub — implement or provide BIOS)"
            ));
        }
    }

    /// Snapshot of unhandled SWI counts for the final AV / SWI report.
    #[must_use]
    pub fn unhandled_swi_counts(&self) -> &BTreeMap<u8, u64> {
        &self.swi.by_num
    }

    /// True when headless `--trace N` has exhausted its dump budget.
    #[must_use]
    pub fn trace_exhausted(&self) -> bool {
        matches!(self.trace_remaining, Some(0))
    }
}

/// GB-inspired structured AV frame report (stdout / agent greppable).
#[must_use]
pub fn format_av_report(gba: &crate::Gba, frames: u64) -> String {
    let regs = &gba.ppu.regs;
    let bd = backdrop_from_palette(&gba.bus.palette);
    let pixels = analyze_framebuffer(&gba.ppu.fb, bd);
    let pwm = pwm_rate_hz(gba.apu.regs.pwm_resolution());
    let (dc_l, dc_r) = gba.apu.health.mean_dc();
    let mut out = String::new();
    let _ = writeln!(&mut out, "=== graycart-gba AV report (frame={frames}) ===");
    let _ = writeln!(
        &mut out,
        "PPU mode={} dispcnt=0x{:04X} blank={} layers={} mosaic=0x{:04X} blend={} objwin={}",
        regs.bg_mode(),
        regs.dispcnt,
        regs.forced_blank(),
        layer_enable_label(regs),
        regs.mosaic,
        blend_label(blend_mode(regs)),
        objwin_active(regs)
    );
    let _ = writeln!(
        &mut out,
        "PPU pixels black={}% backdrop={}% other={} bd=0x{bd:04X} writes vram={} oam={} pal={}",
        pixels.black_pct(),
        pixels.backdrop_pct(),
        pixels.other,
        gba.bus.vram_write_count,
        gba.bus.oam_write_count,
        gba.bus.palette_write_count
    );
    let _ = writeln!(
        &mut out,
        "APU master={} pwm={pwm}Hz fifoA={} fifoB={} underrun={}/{} overrun={}/{} empty_drain={}/{} peak=[{}..{}] dc≈[{dc_l},{dc_r}] clip={}",
        gba.apu.regs.master_enabled(),
        fifo_route_label(&gba.apu.regs, true),
        fifo_route_label(&gba.apu.regs, false),
        gba.apu.fifos.a.underruns,
        gba.apu.fifos.b.underruns,
        gba.apu.fifos.a.overruns,
        gba.apu.fifos.b.overruns,
        gba.apu.health.empty_drain_a,
        gba.apu.health.empty_drain_b,
        if gba.apu.health.samples == 0 {
            0
        } else {
            gba.apu.health.peak_min
        },
        if gba.apu.health.samples == 0 {
            0
        } else {
            gba.apu.health.peak_max
        },
        gba.apu.health.clip_hits
    );
    let swi = gba.debug.unhandled_swi_counts();
    if swi.is_empty() {
        let _ = writeln!(&mut out, "SWI unhandled=(none)");
    } else {
        let mut parts = String::new();
        for (&num, &n) in swi {
            if !parts.is_empty() {
                parts.push(',');
            }
            let _ = write!(&mut parts, "0x{num:02X}:{n}");
        }
        let _ = writeln!(&mut out, "SWI unhandled=[{parts}]");
    }
    out
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
                println!(
                    "debug: --debug (summary) / --debug=trace / GRAYCART_DEBUG; breadcrumbs on stderr"
                );
            }
        }
    }
}
