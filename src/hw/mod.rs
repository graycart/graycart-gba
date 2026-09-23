//! One machine: CPU, bus, scanline timing, and the picture buffer.

use crate::bus::Bus;
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
        }
    }

    pub fn run_frames(&mut self, frames: u32) {
        let budget = u64::from(frames) * CYCLES_PER_FRAME;
        self.run_until(budget);
    }

    /// Advance at most `max` cycles (for unit tests that need a tight bound).
    pub fn run_cycles(&mut self, max: u64) {
        let budget = self.cycles.saturating_add(max);
        self.run_until(budget);
    }

    fn run_until(&mut self, budget: u64) {
        while self.cycles < budget && !self.cpu.idle && self.error.is_none() {
            let frame_before = self.cycles / CYCLES_PER_FRAME;

            if !self.bus.halted {
                match self.cpu.step(&mut self.bus) {
                    Ok(()) => self.cycles += 1,
                    Err(err) => self.error = Some(err),
                }
                self.idle = self.cpu.idle;
            } else {
                self.cycles += 1;
            }

            let mask = self.bus.timers.tick(1);
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
            if self.bus.vblank && !self.prev_vblank && dispstat & (1 << 3) != 0 {
                self.bus.irq.raise(1);
            }
            if self.bus.hblank && !self.prev_hblank && dispstat & (1 << 4) != 0 {
                self.bus.irq.raise(2);
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
                    self.cpu.raise_irq();
                }
            }

            if self.bus.halted && !self.bus.irq.can_wake() {
                self.bus.warn_halt_forever();
            }

            let frame_after = self.cycles / CYCLES_PER_FRAME;
            if frame_after != frame_before {
                self.render_frame();
            }
        }
        self.idle = self.cpu.idle;
        // Idle can land mid-frame; always settle the picture on exit.
        self.render_frame();
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

#[cfg(test)]
mod tests;
