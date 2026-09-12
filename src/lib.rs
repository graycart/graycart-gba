//! graycart-gba library — empty machine scaffold (P0).
//!
//! Module map follows the graycart-gba implementation plan §2.2
//! (Project store: `docs/graycart-gba/08-implementation-plan.md`).
//! No hardware behavior yet — placeholders only.

pub mod apu;
pub mod bios;
pub mod bus;
pub mod cart;
pub mod compat;
pub mod cpu;
pub mod dma;
pub mod hw;
pub mod input;
pub mod irq;
pub mod ppu;
pub mod timer;

/// Thin GBA machine orchestrator. Subsystems are stubs until later phases.
#[derive(Debug, Default)]
pub struct Gba {
    pub cpu: cpu::Cpu,
    pub bus: bus::Bus,
    pub dma: dma::Dma,
    pub ppu: ppu::Ppu,
    pub apu: apu::Apu,
    pub timer: timer::Timers,
    pub irq: irq::Irq,
    pub input: input::Input,
    pub cart: cart::Cart,
    pub bios: bios::Bios,
    pub hw: hw::Hw,
    pub compat: compat::Compat,
}

impl Gba {
    /// Create a power-on placeholder machine (no silicon model yet).
    pub fn new() -> Self {
        Self::default()
    }

    /// Headless frame advance stub. Returns immediately; no emulation.
    pub fn run_frames(&mut self, _n: u64) {
        // TODO(P1+): schedule CPU / bus / PPU / … per implementation plan §2.2.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_constructs() {
        let mut gba = Gba::new();
        gba.run_frames(0);
    }
}
