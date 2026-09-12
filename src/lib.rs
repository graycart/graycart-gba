//! graycart-gba library — GBA machine orchestrator (load / soft-boot / step).
//!
//! Module map follows the graycart-gba implementation plan §2.2
//! (Project store: `docs/graycart-gba/08-implementation-plan.md`).
//!
//! Cited: GBATEK — Memory Map / LCD I/O (DISPSTAT VBlank for suite m_vsync)
//!   https://problemkaputt.de/gbatek.htm
//! Cited: graycart-gba test apparatus §4 (harness hooks)
//!   Project store: `docs/graycart-gba/11-test-apparatus.md`
//! Note: cycle counts are crude (1 per insn) until waitstate scheduling; VBlank
//! bit is toggled in the run loop so jsmolka `m_vsync` can exit without a PPU.
//! P3: timers / keypad poll / Halt wake / IRQ sample run each step; MMIO for
//! those ports is dispatched via [`mmio::MachineMem`].

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
pub mod mmio;
pub mod ppu;
pub mod timer;

use bios::BiosMode;
use cpu::{soft_boot, step, ExceptionKind, StepHle, StepOutcome};
use mmio::MachineMem;

/// How a ROM is launched (mirrors harness [`RomLaunchMode`] names).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomLaunchMode {
    /// Soft entry ~`0x08000000` with HLE SWI (default homebrew / jsmolka).
    BiosHle,
    /// Real `gba_bios.bin` mapped (user-provided, never in git).
    BiosLle,
    /// Multiboot entry at `0x02000000` (image must already be in EWRAM).
    Multiboot,
}

/// Thin GBA machine orchestrator.
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
    /// Instructions retired since last reset (crude cycle proxy).
    pub cycles: u64,
}

impl Gba {
    /// Create a power-on placeholder machine.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Load Game Pak ROM bytes into cart + bus ROM window.
    pub fn load_rom(&mut self, bytes: &[u8]) {
        self.cart.load(bytes);
        self.bus.rom = self.cart.rom.clone();
    }

    /// Reset CPU/pipeline and apply launch mode. Does not clear cart ROM.
    ///
    /// `BiosLle` without a mapped BIOS image returns `Err`.
    pub fn reset(&mut self, mode: RomLaunchMode) -> Result<(), String> {
        self.cycles = 0;
        match mode {
            RomLaunchMode::BiosHle => {
                self.bios.mode = BiosMode::Hle;
                soft_boot::apply(&mut self.cpu, soft_boot::CART_ENTRY);
                Ok(())
            }
            RomLaunchMode::Multiboot => {
                self.bios.mode = BiosMode::Hle;
                // Expect caller to have placed the image in EWRAM already.
                soft_boot::apply(&mut self.cpu, soft_boot::MULTIBOOT_ENTRY);
                Ok(())
            }
            RomLaunchMode::BiosLle => {
                if self.bus.bios.is_empty() {
                    return Err("BiosLle requires user-provided gba_bios.bin (never in git)".into());
                }
                self.bios.mode = BiosMode::Lle;
                // LLE: start at BIOS reset vector 0; not used by jsmolka CI path.
                soft_boot::apply(&mut self.cpu, 0);
                Ok(())
            }
        }
    }

    /// Soft-boot convenience for BiosHle homebrew / jsmolka.
    pub fn reset_bios_hle(&mut self) {
        let _ = self.reset(RomLaunchMode::BiosHle);
    }

    fn step_hle(&self) -> StepHle {
        match self.bios.mode {
            BiosMode::Hle => StepHle::bios_hle(),
            BiosMode::Lle => StepHle::default(),
        }
    }

    /// Borrow bus + P3 peripherals as a [`CpuMem`] view (MMIO side effects).
    #[cfg(test)]
    fn machine_mem(&mut self) -> MachineMem<'_> {
        MachineMem {
            bus: &mut self.bus,
            irq: &mut self.irq,
            timer: &mut self.timer,
            input: &mut self.input,
            hw: &mut self.hw,
        }
    }

    /// Sample IE∧IF∧IME∧!CPSR.I → IRQ exception + pipeline refill.
    ///
    /// Returns `true` when an IRQ was taken.
    fn try_service_irq(&mut self) -> bool {
        if self.irq.try_service_cpu(&mut self.cpu).is_none() {
            return false;
        }
        {
            let Self {
                bus,
                irq,
                timer,
                input,
                hw,
                cpu,
                ..
            } = self;
            let mut mem = MachineMem {
                bus,
                irq,
                timer,
                input,
                hw,
            };
            cpu.pipeline.refill(&mut mem);
        }
        // IRQ always enters ARM; keep regs.pc aligned with Decode.
        if let Some(d) = self.cpu.pipeline.decode {
            self.cpu.regs.set_pc(d.addr);
        }
        true
    }

    /// One machine step: timer/keypad/halt → IRQ sample → (optional) CPU insn.
    ///
    /// Crude 1-cycle-per-step proxy (same as pre-P3). Halt/Stop skips CPU
    /// retirement but still advances timers so IE∧IF can wake.
    pub fn step_instruction(&mut self) -> StepOutcome {
        const STEP_CYCLES: u64 = 1;

        self.timer.step(STEP_CYCLES, &mut self.irq);
        self.input.poll_keypad_irq(&mut self.irq);
        self.hw.poll_halt_wake(self.irq.ie_and_if());

        if self.try_service_irq() {
            self.cycles = self.cycles.wrapping_add(STEP_CYCLES);
            return StepOutcome::Exception(ExceptionKind::Irq);
        }

        if self.hw.cpu_sleeping() {
            self.cycles = self.cycles.wrapping_add(STEP_CYCLES);
            return StepOutcome::Ok;
        }

        tick_vblank_hle(&mut self.bus, self.cycles);
        let hle = self.step_hle();
        let outcome = {
            let Self {
                bus,
                irq,
                timer,
                input,
                hw,
                cpu,
                ..
            } = self;
            let mut mem = MachineMem {
                bus,
                irq,
                timer,
                input,
                hw,
            };
            step(cpu, &mut mem, hle)
        };
        self.cycles = self.cycles.wrapping_add(STEP_CYCLES);
        outcome
    }

    /// Run up to `max_steps` instructions (crude cycle budget).
    pub fn run_cycles(&mut self, max_steps: u64) {
        for _ in 0..max_steps {
            self.step_instruction();
        }
    }

    /// Headless frame advance stub (PPU not scheduled yet). Uses a fixed insn budget.
    pub fn run_frames(&mut self, n: u64) {
        // ~280k cycles/frame @ 16.78 MHz / 60 — crude stand-in until PPU owns frames.
        const STEPS_PER_FRAME: u64 = 280_896;
        self.run_cycles(n.saturating_mul(STEPS_PER_FRAME));
    }

    /// Architectural R12 (jsmolka fail# / pass=0 after `m_test_eval`).
    #[must_use]
    pub fn r12(&self) -> u32 {
        self.cpu.regs.get(12)
    }

    /// Peek IWRAM byte (oracle / debug).
    #[must_use]
    pub fn iwram8(&self, offset: usize) -> u8 {
        self.bus.iwram.get(offset).copied().unwrap_or(0)
    }

    /// Peek I/O halfword (e.g. DISPCNT / DISPSTAT / KEYINPUT / IE).
    #[must_use]
    pub fn io16(&self, offset: usize) -> u16 {
        match offset {
            o if (0x100..0x110).contains(&o) => self.timer.read_mmio16(o - 0x100),
            0x130 => self.input.read_keyinput(),
            0x132 => self.input.read_keycnt(),
            0x200 => self.irq.read_ie(),
            0x202 => self.irq.read_if(),
            0x204 => self.hw.read_waitcnt(),
            0x208 => self.irq.read_ime() as u16,
            0x128 => self.hw.read_siocnt(),
            0x134 => self.hw.read_rcnt(),
            0x300 => u16::from(self.hw.read_postflg()),
            _ => {
                let lo = u16::from(self.bus.io.get(offset).copied().unwrap_or(0));
                let hi = u16::from(self.bus.io.get(offset + 1).copied().unwrap_or(0));
                lo | (hi << 8)
            }
        }
    }

    /// Mode 4 VRAM byte (LCD / framebuffer oracle).
    #[must_use]
    pub fn vram8(&self, offset: usize) -> u8 {
        self.bus.vram.get(offset).copied().unwrap_or(0)
    }

    /// PC of the instruction currently in Decode (if any).
    #[must_use]
    pub fn decode_pc(&self) -> Option<u32> {
        self.cpu.pipeline.decode_pc()
    }
}

/// DISPSTAT at `0x04000004` — bit 0 = VBlank flag.
const DISPSTAT_IO: usize = 4;

/// Toggle VBlank each step so jsmolka `m_vsync` (wait !VBlank then VBlank) can proceed.
fn tick_vblank_hle(bus: &mut bus::Bus, cycles: u64) {
    let mut v = u16::from(bus.io.get(DISPSTAT_IO).copied().unwrap_or(0))
        | (u16::from(bus.io.get(DISPSTAT_IO + 1).copied().unwrap_or(0)) << 8);
    if (cycles & 1) == 0 {
        v &= !1;
    } else {
        v |= 1;
    }
    bus.io[DISPSTAT_IO] = v as u8;
    bus.io[DISPSTAT_IO + 1] = (v >> 8) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_constructs() {
        let mut gba = Gba::new();
        gba.run_frames(0);
    }

    #[test]
    fn load_rom_copies_to_bus() {
        let mut gba = Gba::new();
        gba.load_rom(&[0xAA, 0xBB, 0xCC, 0xDD]);
        assert_eq!(gba.bus.rom[..4], [0xAA, 0xBB, 0xCC, 0xDD]);
    }

    #[test]
    fn bios_hle_soft_boot_pc() {
        let mut gba = Gba::new();
        gba.load_rom(&[0x00; 0x100]);
        gba.reset_bios_hle();
        assert_eq!(gba.cpu.regs.pc(), soft_boot::CART_ENTRY);
    }

    #[test]
    fn bios_lle_without_image_errors() {
        let mut gba = Gba::new();
        let err = gba.reset(RomLaunchMode::BiosLle).unwrap_err();
        assert!(err.contains("gba_bios.bin"));
    }

    #[test]
    fn mmio_ie_if_ime_roundtrip_via_machine_mem() {
        use crate::bus::CpuMem;
        let mut gba = Gba::new();
        {
            let mut mem = gba.machine_mem();
            mem.write16(0x0400_0200, 0xFFFF);
            mem.write16(0x0400_0202, 0); // no-op ack
            mem.write32(0x0400_0208, 1);
        }
        assert_eq!(gba.irq.read_ie(), 0x3FFF);
        assert!(gba.irq.ime());
        // Raise IF then W1C-ack bit0 only.
        gba.irq.raise(0x0005);
        {
            let mut mem = gba.machine_mem();
            mem.write16(0x0400_0202, 0x0001);
            assert_eq!(mem.read16(0x0400_0202), 0x0004);
        }
        assert_eq!(gba.irq.read_if(), 0x0004);
    }

    #[test]
    fn halt_wakes_on_timer_overflow_ie_if() {
        use crate::bus::CpuMem;
        use crate::timer::{TimerId, CTRL_IRQ, CTRL_START};
        let mut gba = Gba::new();
        gba.hw.enter_halt();
        gba.irq.write_ie(irq::IRQ_TIMER0);
        // IME clear — Halt wake must still occur (IME don't-care).
        gba.irq.set_ime(false);
        {
            let mut mem = gba.machine_mem();
            // Reload 0xFFFF; start + IRQ enable; one more tick overflows.
            mem.write16(0x0400_0100, 0xFFFF);
            mem.write16(0x0400_0102, CTRL_START | CTRL_IRQ);
        }
        assert_eq!(gba.timer.read_counter(TimerId::Tm0), 0xFFFF);
        // One step: timer increments → overflow → IF → Halt wake.
        gba.step_instruction();
        assert!(
            !gba.hw.cpu_sleeping(),
            "Halt should wake on IE∧IF even with IME=0"
        );
        assert_ne!(gba.irq.read_if() & irq::IRQ_TIMER0, 0);
    }

    #[test]
    fn keyinput_readable_via_mmio() {
        use crate::bus::CpuMem;
        use crate::input::button;
        let mut gba = Gba::new();
        gba.input.press(button::A | button::START);
        let mut mem = gba.machine_mem();
        let ki = mem.read16(0x0400_0130);
        assert_eq!(ki & button::A, 0);
        assert_eq!(ki & button::START, 0);
        assert_ne!(ki & button::B, 0);
    }
}
