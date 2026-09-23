//! One machine: CPU, bus, scanline timing, and the picture buffer.

use std::path::{Path, PathBuf};

use crate::bus::Bus;
use crate::cart::{parse_header, read_sidecar, sidecar_path, write_sidecar, SaveKind};
use crate::cpu::{Cpu, StepError};
use crate::ppu::Ppu;

const CYCLES_PER_LINE: u64 = 1232;
const HBLANK_START: u64 = 960;
const LINES: u16 = 228;
const CYCLES_PER_FRAME: u64 = CYCLES_PER_LINE * LINES as u64;

pub struct Machine {
    pub cpu: Cpu,
    pub bus: Bus,
    pub ppu: Ppu,
    pub cycles: u64,
    pub idle: bool,
    pub error: Option<StepError>,
    prev_vblank: bool,
    prev_hblank: bool,
    prev_vmatch: bool,
    /// Sidecar path when opened from a file; `None` for [`Self::from_rom`].
    sidecar: Option<PathBuf>,
    /// ARM instructions actually executed. Stays 0 after the Game Boy switch.
    pub arm_steps: u64,
}

impl Machine {
    pub fn from_rom(rom: Vec<u8>) -> Self {
        Self {
            cpu: Cpu::new(),
            bus: Bus::new(rom),
            ppu: Ppu::new(),
            cycles: 0,
            idle: false,
            error: None,
            prev_vblank: false,
            prev_hblank: false,
            prev_vmatch: false,
            sidecar: None,
            arm_steps: 0,
        }
    }

    /// Load a ROM from disk, parse the header, and load `<rom>.sav` when present.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let rom = std::fs::read(path).map_err(|e| e.to_string())?;
        parse_header(&rom)?;
        let mut machine = Self::from_rom(rom);
        let sav = sidecar_path(path);
        let bytes = read_sidecar(&sav)?;
        if !bytes.is_empty() {
            machine.bus.load_save(&bytes);
        }
        machine.sidecar = Some(sav);
        Ok(machine)
    }

    /// Write the save chip to the sidecar (no-op when kind is none or no path).
    pub fn flush_save(&self) -> Result<(), String> {
        let Some(path) = &self.sidecar else {
            return Ok(());
        };
        if self.bus.save_kind() == SaveKind::None {
            return Ok(());
        }
        let Some(bytes) = self.bus.save_bytes() else {
            return Ok(());
        };
        write_sidecar(path, bytes)
    }

    pub fn run_frames(&mut self, frames: u32) {
        let budget = u64::from(frames) * CYCLES_PER_FRAME;
        let _ = self.run_until(budget, false);
    }

    /// Debug run. Stops on a pass, a fail, a fault, a permanent halt, or a stuck loop.
    /// `frames` is only a cap. Returns whether the jsmolka pass rule matched.
    pub fn run_debug(&mut self, frames: Option<u32>) -> bool {
        let budget = frames
            .map(|n| u64::from(n) * CYCLES_PER_FRAME)
            .unwrap_or(u64::MAX);
        self.run_until(budget, true) == DebugEnd::Passed
    }

    /// Frames actually executed. A stop mid-frame counts that frame.
    pub fn frames_done(&self) -> u32 {
        let whole = (self.cycles / CYCLES_PER_FRAME) as u32;
        if self.cycles.is_multiple_of(CYCLES_PER_FRAME) {
            whole
        } else {
            whole + 1
        }
    }

    /// Advance at most `max` cycles (for unit tests that need a tight bound).
    pub fn run_cycles(&mut self, max: u64) {
        let budget = self.cycles.saturating_add(max);
        let _ = self.run_until(budget, false);
    }

    fn passed(&self) -> bool {
        let r7 = self.cpu.reg(7);
        let thumb_fail = (1..=999).contains(&r7);
        self.cpu.idle && self.error.is_none() && self.cpu.reg(12) == 0 && !thumb_fail
    }

    fn run_until(&mut self, budget: u64, watch: bool) -> DebugEnd {
        let mut watch_state = LoopWatch::default();
        let end = loop {
            if self.cycles >= budget {
                break DebugEnd::Capped;
            }
            if self.bus.gb_mode() {
                self.cycles = budget;
                break DebugEnd::Capped;
            }
            if self.cpu.idle {
                break if self.passed() {
                    DebugEnd::Passed
                } else {
                    DebugEnd::Failed
                };
            }
            if self.error.is_some() {
                break DebugEnd::Failed;
            }
            let frame_before = self.cycles / CYCLES_PER_FRAME;

            let advance = if self.bus.dma.stall > 0 {
                self.bus.dma.stall -= 1;
                1
            } else if !self.bus.halted {
                self.bus.begin_step();
                self.arm_steps = self.arm_steps.saturating_add(1);
                match self.cpu.step(&mut self.bus) {
                    Ok(()) => {
                        self.bus.finish_step(self.cpu.fetch_pc);
                        let n = self.bus.take_step_cycles();
                        self.idle = self.cpu.idle;
                        if watch {
                            if let Some(end) = watch_state.note_step(self.cpu.exec_pc) {
                                // Still advance the charged cycles so VCOUNT moves.
                                let _ = self.advance_cycles(n);
                                break end;
                            }
                        }
                        n
                    }
                    Err(err) => {
                        self.error = Some(err);
                        0
                    }
                }
            } else {
                1
            };

            if advance == 0 {
                continue;
            }
            if self.advance_cycles(advance) {
                break DebugEnd::Failed;
            }

            let frame_after = self.cycles / CYCLES_PER_FRAME;
            if frame_after != frame_before {
                self.render_frame();
            }
        };
        self.idle = self.cpu.idle;
        // Idle can land mid-frame; always settle the picture on exit.
        self.render_frame();
        end
    }

    /// Tick timers, APU, scanline, and IRQ edges for `count` CPU cycles.
    /// Returns true when halt can never wake (debug fail).
    fn advance_cycles(&mut self, count: u32) -> bool {
        for _ in 0..count {
            self.cycles += 1;

            let mask = self.bus.timers.tick(1);
            self.bus.tick_apu(mask);
            for index in 0..4u32 {
                if mask & (1 << index) != 0 {
                    let control = self.bus.timers.read16(index * 4 + 2);
                    if control & (1 << 6) != 0 {
                        self.bus.irq.raise(1 << (3 + index));
                    }
                }
            }
            if self.bus.keypad.irq_asserted() {
                self.bus.irq.raise(0x1000);
            }

            let line = (self.cycles / CYCLES_PER_LINE) % u64::from(LINES);
            self.bus.vcount = line as u16;
            self.bus.vblank = self.bus.vcount >= 160;
            self.bus.hblank = (self.cycles % CYCLES_PER_LINE) >= HBLANK_START;
            let vmatch = self.bus.vcount_match();

            let dispstat = self.bus.dispstat_written();
            if self.bus.vblank && !self.prev_vblank {
                self.bus.dma_on_vblank();
                if dispstat & (1 << 3) != 0 {
                    self.bus.irq.raise(1);
                }
            }
            if self.bus.hblank && !self.prev_hblank {
                // HBlank DMA only on visible lines; VBlank lines still raise HBlank IRQ.
                if self.bus.vcount < 160 {
                    self.bus.dma_on_hblank();
                }
                if dispstat & (1 << 4) != 0 {
                    self.bus.irq.raise(2);
                }
            }
            if vmatch && !self.prev_vmatch && dispstat & (1 << 5) != 0 {
                self.bus.irq.raise(4);
            }
            self.prev_vblank = self.bus.vblank;
            self.prev_hblank = self.bus.hblank;
            self.prev_vmatch = vmatch;

            if self.bus.irq.pending() {
                // An enabled pending IRQ leaves halt even when CPSR I blocks entry.
                self.bus.halted = false;
                if self.cpu.cpsr() & 0x80 == 0 {
                    self.cpu.raise_irq(&mut self.bus);
                }
            }

            if self.bus.halted && !self.bus.irq.can_wake() {
                self.bus.warn_halt_forever();
                return true;
            }
        }
        false
    }

    fn render_frame(&mut self) {
        let dispcnt = self.bus.dispcnt();
        self.ppu.render(
            dispcnt,
            self.bus.palette(),
            self.bus.vram(),
            self.bus.oam(),
            self.bus.io(),
        );
    }
}

/// Why a debug run stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DebugEnd {
    Passed,
    Failed,
    Capped,
}

/// Same idea as the Game Boy Mooneye runner: a pass returns immediately, and a
/// PC that is not the pass signature is a fail. A one-instruction spin fails
/// at 64 hits. A short poll (the PC repeats within the last 16 steps) fails
/// after half a million steps, which is longer than a vblank wait.
struct LoopWatch {
    hist: [u32; 32],
    n: u32,
    last_pc: u32,
    same_pc: u32,
    streak: u32,
}

impl Default for LoopWatch {
    fn default() -> Self {
        Self {
            hist: [0; 32],
            n: 0,
            last_pc: u32::MAX,
            same_pc: 0,
            streak: 0,
        }
    }
}

impl LoopWatch {
    fn note_step(&mut self, pc: u32) -> Option<DebugEnd> {
        if pc == self.last_pc {
            self.same_pc = self.same_pc.saturating_add(1);
            if self.same_pc >= 64 {
                return Some(DebugEnd::Failed);
            }
        } else {
            self.same_pc = 0;
            self.last_pc = pc;
        }

        let idx = (self.n as usize) % self.hist.len();
        self.hist[idx] = pc;
        self.n = self.n.wrapping_add(1);

        if self.n > 16 {
            let mut looping = false;
            for period in 1..=16 {
                let prev = self.hist[(self.n as usize - 1 - period) % self.hist.len()];
                if prev == pc {
                    looping = true;
                    break;
                }
            }
            if looping {
                self.streak = self.streak.saturating_add(1);
                if self.streak >= 500_000 {
                    return Some(DebugEnd::Failed);
                }
            } else {
                self.streak = 0;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
