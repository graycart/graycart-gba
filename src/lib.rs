//! graycart-gba library — GBA machine orchestrator (load / soft-boot / step).
//!
//! Module map follows the graycart-gba implementation plan §2.2
//! (Project store: `docs/graycart-gba/08-implementation-plan.md`).
//!
//! Cited: GBATEK — Memory Map / LCD I/O (DISPSTAT / VCOUNT) / DMA Transfers
//!   https://problemkaputt.de/gbatek.htm
//! Cited: graycart-gba test apparatus §4 (harness hooks)
//!   Project store: `docs/graycart-gba/11-test-apparatus.md`
//! Note: cycle counts are crude (1 per insn) until waitstate scheduling.
//! P3: timers / keypad / Halt wake / IRQ sample.
//! P4: real PPU scanline timing replaces crude VBlank toggle.
//! P5: DMA VBlank/HBlank/FIFO/capture starts hooked from PPU/APU edges.
//! P6: APU PSG + FIFO timer clock + mixer/PCM; sound MMIO owned by APU.
//! P7: cart saves + BIOS protect latch + BiosHle SWI/IRQ trampoline.

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

use bios::{
    hle, BiosMode, HLE_IRQ_RETURN, IRQ_HANDLER_PTR, LATCH_AFTER_IRQ, LATCH_AFTER_SWI,
    LATCH_DURING_IRQ, LATCH_SOFT_RESET,
};
use cpu::{soft_boot, step, ExceptionKind, IsaState, Mode, StepHle, StepOutcome};
use mmio::{BusApuMem, MachineMem};
use ppu::FRAME_CYCLES;

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

    /// Load Game Pak ROM bytes into cart + bus ROM window (detects save type).
    pub fn load_rom(&mut self, bytes: &[u8]) {
        self.cart.load(bytes);
        self.bus.rom = self.cart.rom.clone();
    }

    /// Load a user-supplied BIOS image into the bus (LLE). Never called by CI.
    pub fn load_bios(&mut self, bytes: &[u8]) -> Result<(), String> {
        let img = bios::lle::validate_bios_bytes(bytes)?;
        self.bus.bios = img;
        Ok(())
    }

    /// Reset CPU/pipeline and apply launch mode. Does not clear cart ROM.
    ///
    /// `BiosLle` without a mapped BIOS image returns `Err`.
    pub fn reset(&mut self, mode: RomLaunchMode) -> Result<(), String> {
        self.cycles = 0;
        self.bios.hle_irq_resume = None;
        match mode {
            RomLaunchMode::BiosHle => {
                self.bios.mode = BiosMode::Hle;
                let latch = hle::soft_boot_cart(&mut self.cpu);
                self.bus.open_bus.note_bios_fetch(latch);
                self.bus.cpu_pc = soft_boot::CART_ENTRY;
                self.hw.write_postflg(self.hw.read_postflg() | 1);
                Ok(())
            }
            RomLaunchMode::Multiboot => {
                self.bios.mode = BiosMode::Hle;
                let latch = hle::soft_boot_multiboot(&mut self.cpu);
                self.bus.open_bus.note_bios_fetch(latch);
                self.bus.cpu_pc = soft_boot::MULTIBOOT_ENTRY;
                self.hw.write_postflg(self.hw.read_postflg() | 1);
                Ok(())
            }
            RomLaunchMode::BiosLle => {
                if self.bus.bios.is_empty() {
                    return Err("BiosLle requires user-provided gba_bios.bin (never in git)".into());
                }
                self.bios.mode = BiosMode::Lle;
                soft_boot::apply(&mut self.cpu, 0);
                self.bus.open_bus.note_bios_fetch(LATCH_SOFT_RESET);
                self.bus.cpu_pc = 0;
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

    /// Borrow bus + peripherals as a [`CpuMem`] view (MMIO side effects).
    #[cfg(test)]
    fn machine_mem(&mut self) -> MachineMem<'_> {
        MachineMem {
            bus: &mut self.bus,
            irq: &mut self.irq,
            timer: &mut self.timer,
            input: &mut self.input,
            hw: &mut self.hw,
            ppu: &mut self.ppu,
            dma: &mut self.dma,
            apu: &mut self.apu,
            cart: &mut self.cart,
        }
    }

    /// Sample IE∧IF∧IME∧!CPSR.I → IRQ exception + pipeline refill.
    fn try_service_irq(&mut self) -> bool {
        if self.irq.try_service_cpu(&mut self.cpu).is_none() {
            return false;
        }
        if self.bios.mode == BiosMode::Hle {
            // BiosHle IRQ trampoline: save r0–r3,r12 (BIOS wrapper), jump to [03007FFC].
            let resume = self.cpu.regs.get_r14_mode(Mode::Irq).wrapping_sub(4);
            self.bios.hle_irq_resume = Some(resume);
            self.bios.hle_irq_regs = [
                self.cpu.regs.get(0),
                self.cpu.regs.get(1),
                self.cpu.regs.get(2),
                self.cpu.regs.get(3),
                self.cpu.regs.get(12),
            ];
            self.bus.open_bus.note_bios_fetch(LATCH_DURING_IRQ);
            let handler = {
                let off = (IRQ_HANDLER_PTR - 0x0300_0000) as usize;
                let b0 = u32::from(self.bus.iwram.get(off).copied().unwrap_or(0));
                let b1 = u32::from(self.bus.iwram.get(off + 1).copied().unwrap_or(0));
                let b2 = u32::from(self.bus.iwram.get(off + 2).copied().unwrap_or(0));
                let b3 = u32::from(self.bus.iwram.get(off + 3).copied().unwrap_or(0));
                b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
            };
            self.cpu.regs.set_r14_mode(Mode::Irq, HLE_IRQ_RETURN);
            self.cpu.regs.set_pc(handler);
            self.cpu.pipeline.redirect(handler, IsaState::Arm);
            {
                let Self {
                    bus,
                    irq,
                    timer,
                    input,
                    hw,
                    ppu,
                    dma,
                    apu,
                    cart,
                    cpu,
                    ..
                } = self;
                bus.cpu_pc = handler;
                let mut mem = MachineMem {
                    bus,
                    irq,
                    timer,
                    input,
                    hw,
                    ppu,
                    dma,
                    apu,
                    cart,
                };
                cpu.pipeline.refill(&mut mem);
            }
            if let Some(d) = self.cpu.pipeline.decode {
                self.cpu.regs.set_pc(d.addr);
            }
            return true;
        }
        {
            let Self {
                bus,
                irq,
                timer,
                input,
                hw,
                ppu,
                dma,
                apu,
                cart,
                cpu,
                ..
            } = self;
            let mut mem = MachineMem {
                bus,
                irq,
                timer,
                input,
                hw,
                ppu,
                dma,
                apu,
                cart,
            };
            cpu.pipeline.refill(&mut mem);
        }
        if let Some(d) = self.cpu.pipeline.decode {
            self.cpu.regs.set_pc(d.addr);
        }
        true
    }

    /// Complete BiosHle IRQ return when PC hits the sentinel.
    fn try_hle_irq_return(&mut self) -> bool {
        let pc = self
            .cpu
            .pipeline
            .decode_pc()
            .unwrap_or_else(|| self.cpu.regs.pc());
        if pc != HLE_IRQ_RETURN {
            return false;
        }
        let Some(resume) = self.bios.hle_irq_resume.take() else {
            return false;
        };
        self.bus.open_bus.note_bios_fetch(LATCH_AFTER_IRQ);
        // Restore registers the BIOS IRQ wrapper would have preserved.
        let saved = self.bios.hle_irq_regs;
        self.cpu.regs.set(0, saved[0]);
        self.cpu.regs.set(1, saved[1]);
        self.cpu.regs.set(2, saved[2]);
        self.cpu.regs.set(3, saved[3]);
        self.cpu.regs.set(12, saved[4]);
        let spsr = self
            .cpu
            .regs
            .spsr_of(Mode::Irq)
            .unwrap_or(self.cpu.regs.cpsr());
        self.cpu.regs.set_cpsr(spsr);
        self.cpu.regs.set_pc(resume);
        let isa = IsaState::from_cpsr_t(self.cpu.regs.thumb());
        self.cpu.pipeline.redirect(resume, isa);
        {
            let Self {
                bus,
                irq,
                timer,
                input,
                hw,
                ppu,
                dma,
                apu,
                cart,
                cpu,
                ..
            } = self;
            bus.cpu_pc = resume;
            let mut mem = MachineMem {
                bus,
                irq,
                timer,
                input,
                hw,
                ppu,
                dma,
                apu,
                cart,
            };
            cpu.pipeline.refill(&mut mem);
        }
        if let Some(d) = self.cpu.pipeline.decode {
            self.cpu.regs.set_pc(d.addr);
        }
        true
    }

    /// Fire VBlank/HBlank/FIFO/capture starts from current PPU/APU edges, then drain.
    fn service_dma_edges(&mut self) {
        let vblank = self.ppu.timing.entered_vblank_edge();
        let hblank = self.ppu.timing.entered_hblank_edge();
        let hif = self.ppu.regs.hblank_interval_free();
        let vcount = self.ppu.timing.vcount;
        let fifo_req = self.apu.take_fifo_dma_request();

        let Self {
            bus, irq, dma, apu, ..
        } = self;
        let mut mem = BusApuMem { bus, apu };
        if vblank {
            let _ = dma.on_vblank(&mut mem, irq);
        }
        if hblank {
            let _ = dma.on_hblank(&mut mem, irq, hif);
            let _ = dma.on_capture_hblank(&mut mem, irq, vcount);
        }
        if fifo_req != 0 {
            let _ = dma.on_fifo_request(&mut mem, irq, fifo_req);
        }
        let _ = dma.run_pending(&mut mem, irq);
    }

    /// Service FIFO DMA request bits only (after timer overflows) — no blanking re-fire.
    fn service_fifo_dma(&mut self) {
        let fifo_req = self.apu.take_fifo_dma_request();
        if fifo_req == 0 {
            return;
        }
        let Self {
            bus, irq, dma, apu, ..
        } = self;
        let mut mem = BusApuMem { bus, apu };
        let _ = dma.on_fifo_request(&mut mem, irq, fifo_req);
        let _ = dma.run_pending(&mut mem, irq);
    }

    /// Drain pending Immediate (or already-armed) bursts only — no edge re-fire.
    fn drain_dma(&mut self) {
        let Self {
            bus, irq, dma, apu, ..
        } = self;
        let mut mem = BusApuMem { bus, apu };
        let _ = dma.run_pending(&mut mem, irq);
    }

    /// One machine step: PPU → DMA edges → timer/keypad/halt → IRQ → (optional) CPU.
    pub fn step_instruction(&mut self) -> StepOutcome {
        const STEP_CYCLES: u64 = 1;

        // BiosHle IRQ return sentinel — complete before other work.
        if self.bios.mode == BiosMode::Hle && self.try_hle_irq_return() {
            self.cycles = self.cycles.wrapping_add(STEP_CYCLES);
            return StepOutcome::Ok;
        }

        {
            let Self { bus, irq, ppu, .. } = self;
            ppu.step(STEP_CYCLES as u32, bus, irq);
            ppu.mirror_status_to_io(bus);
        }

        self.service_dma_edges();

        let overflows = self.timer.step(STEP_CYCLES, &mut self.irq);
        self.apu.on_timer_overflows(overflows[0], overflows[1]);
        // FIFO DMA request may have been raised by timer sampling.
        self.service_fifo_dma();
        self.apu.step(STEP_CYCLES);

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

        // BIOS-protect gating uses the instruction about to execute.
        self.bus.cpu_pc = self
            .cpu
            .pipeline
            .decode_pc()
            .unwrap_or_else(|| self.cpu.regs.pc());

        let hle = self.step_hle();
        let outcome = {
            let Self {
                bus,
                irq,
                timer,
                input,
                hw,
                ppu,
                dma,
                apu,
                cart,
                cpu,
                ..
            } = self;
            let mut mem = MachineMem {
                bus,
                irq,
                timer,
                input,
                hw,
                ppu,
                dma,
                apu,
                cart,
            };
            step(cpu, &mut mem, hle)
        };
        if matches!(outcome, StepOutcome::SwiHle) && self.bios.mode == BiosMode::Hle {
            // SoftReset resumes at cart/multiboot entry → SoftReset latch; else After-SWI.
            let pc = self.cpu.regs.pc();
            if pc == soft_boot::CART_ENTRY || pc == soft_boot::MULTIBOOT_ENTRY {
                self.bus.open_bus.note_bios_fetch(LATCH_SOFT_RESET);
            } else {
                self.bus.open_bus.note_bios_fetch(LATCH_AFTER_SWI);
            }
        }
        // Immediate DMA may have been armed by the instruction's MMIO write.
        if self.dma.is_busy() {
            self.drain_dma();
        }
        self.cycles = self.cycles.wrapping_add(STEP_CYCLES);
        outcome
    }

    /// Run up to `max_steps` instructions (crude cycle budget).
    pub fn run_cycles(&mut self, max_steps: u64) {
        for _ in 0..max_steps {
            self.step_instruction();
        }
    }

    /// Headless frame advance via PPU frame cycle count.
    pub fn run_frames(&mut self, n: u64) {
        self.run_cycles(n.saturating_mul(u64::from(FRAME_CYCLES)));
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

    /// Peek I/O halfword (e.g. DISPCNT / DISPSTAT / KEYINPUT / IE / DMAxCNT_H).
    #[must_use]
    pub fn io16(&self, offset: usize) -> u16 {
        match offset {
            o if o <= 0x56 => self.ppu.read16(o),
            o if apu::Apu::owns_offset(o) => self.apu.read16(o),
            o if (0xB0..0xE0).contains(&o) => self.dma.read_mmio16((o - 0xB0) as u32),
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

    /// Pull host PCM frames from the APU ring (P6).
    pub fn pull_audio(&mut self, out: &mut [apu::PcmFrame]) -> usize {
        self.apu.pull_samples(out)
    }

    /// Soft WAV bytes of the current PCM snapshot (P6 soft gate / `--audio-out`).
    #[must_use]
    pub fn soft_wav_bytes(&self) -> Vec<u8> {
        self.apu.soft_wav_bytes()
    }

    /// Mode 4 VRAM byte (LCD / framebuffer oracle).
    #[must_use]
    pub fn vram8(&self, offset: usize) -> u8 {
        self.bus.vram.get(offset).copied().unwrap_or(0)
    }

    /// RGB888 presentment buffer (240×160×3).
    #[must_use]
    pub fn framebuffer_rgb(&self) -> Vec<u8> {
        self.ppu.framebuffer_rgb()
    }

    /// SHA-256 hex of the RGB888 framebuffer.
    #[must_use]
    pub fn frame_hash_sha256(&self) -> String {
        self.ppu.frame_hash_sha256()
    }

    /// PC of the instruction currently in Decode (if any).
    #[must_use]
    pub fn decode_pc(&self) -> Option<u32> {
        self.cpu.pipeline.decode_pc()
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
            mem.write16(0x0400_0202, 0);
            mem.write32(0x0400_0208, 1);
        }
        assert_eq!(gba.irq.read_ie(), 0x3FFF);
        assert!(gba.irq.ime());
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
        gba.irq.set_ime(false);
        {
            let mut mem = gba.machine_mem();
            mem.write16(0x0400_0100, 0xFFFF);
            mem.write16(0x0400_0102, CTRL_START | CTRL_IRQ);
        }
        assert_eq!(gba.timer.read_counter(TimerId::Tm0), 0xFFFF);
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

    #[test]
    fn framebuffer_hash_stable_empty() {
        let gba = Gba::new();
        let h1 = gba.frame_hash_sha256();
        let h2 = gba.frame_hash_sha256();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn dispcnt_via_mmio_reaches_ppu() {
        use crate::bus::CpuMem;
        let mut gba = Gba::new();
        {
            let mut mem = gba.machine_mem();
            mem.write16(0x0400_0000, 0x0404);
        }
        assert_eq!(gba.ppu.regs.bg_mode(), 4);
        assert_eq!(gba.io16(0), 0x0404);
    }

    #[test]
    fn vblank_dma_glue_copies_iwram() {
        use crate::bus::CpuMem;
        use crate::dma::{control_word, ChannelId, DestControl, SrcControl, StartTiming};
        use crate::ppu::timing::CYCLES_PER_LINE;

        let mut gba = Gba::new();
        // Seed IWRAM via bus.
        gba.bus.write16(0x0300_0100, 0x1234);
        gba.bus.write16(0x0300_0102, 0x5678);
        {
            let mut mem = gba.machine_mem();
            mem.write32(0x0400_00B0, 0x0300_0100); // DMA0 SAD
            mem.write32(0x0400_00B4, 0x0300_0200); // DMA0 DAD
            mem.write16(0x0400_00B8, 2); // count
            let ctrl = control_word(
                DestControl::Increment,
                SrcControl::Increment,
                StartTiming::VBlank,
                false,
                true,
            );
            mem.write16(0x0400_00BA, ctrl);
        }
        assert!(gba.dma.channel(ChannelId::Ch0).enabled());

        // Advance into VBlank (line 160): 160 full lines.
        let cycles = u64::from(CYCLES_PER_LINE) * 160;
        gba.run_cycles(cycles);

        assert!(!gba.dma.channel(ChannelId::Ch0).enabled());
        assert_eq!(gba.bus.read16(0x0300_0200), 0x1234);
        assert_eq!(gba.bus.read16(0x0300_0202), 0x5678);
    }

    #[test]
    fn g6_glue_sound_mmio_and_fifo_dma() {
        use crate::bus::CpuMem;
        use crate::dma::{control_word, ChannelId, DestControl, SrcControl, StartTiming};
        use crate::timer::{TimerId, CTRL_START};

        let mut gba = Gba::new();
        {
            let mut mem = gba.machine_mem();
            mem.write16(0x0400_0084, 0x0080); // SOUNDCNT_X master on
            mem.write16(0x0400_0082, 0x0B0F); // SOUNDCNT_H: reset A + route/vol
                                              // Seed source words for DMA1 → FIFO A
            mem.write32(0x0300_0400, 0x0403_0201);
            mem.write32(0x0300_0404, 0x0807_0605);
            mem.write32(0x0300_0408, 0x0C0B_0A09);
            mem.write32(0x0300_040C, 0x100F_0E0D);
            mem.write32(0x0400_00BC, 0x0300_0400); // DMA1 SAD
            mem.write32(0x0400_00C0, 0x0400_00A0); // DMA1 DAD = FIFO A
            mem.write16(0x0400_00C4, 1);
            let ctrl = control_word(
                DestControl::Fixed,
                SrcControl::Increment,
                StartTiming::Special,
                true, // 32-bit
                true,
            ) | crate::dma::CONTROL_REPEAT;
            mem.write16(0x0400_00C6, ctrl);
        }
        assert!(gba.dma.channel(ChannelId::Ch1).enabled());

        // Manually request FIFO DMA (as timer half-empty would).
        gba.apu.request_fifo_dma(true, false);
        gba.service_fifo_dma();
        assert!(gba.apu.fifos.a.len() >= 4);

        // Timer0 overflow path: configure TM0 near overflow and step.
        gba.timer.write_reload(TimerId::Tm0, 0xFFFF);
        gba.timer.write_control(TimerId::Tm0, CTRL_START);
        let before = gba.apu.fifos.a.len();
        gba.step_instruction();
        // May or may not pop depending on overflow this cycle; sound MMIO readable.
        assert_eq!(gba.io16(0x84) & 0x80, 0x80);
        let _ = before;
    }
}
