//! BIOS prefetch latch for the jsmolka bios ROM.
//!
//! A BIOS *data* read returns the last prefetched BIOS opcode word. With no BIOS
//! image those words are set by HLE (post-boot, Sqrt SWI, IRQ enter/exit).
//!
//! Latch words (not a Nintendo BIOS image):
//! - post-boot: `0xE129F000` (BIOS `0xE4`)
//! - after Sqrt SWI `0x08`: `0xE3A02004` (BIOS `0x188`)
//! - during user IRQ handler: `0xE25EF004` (BIOS `0x13C`)
//! - after IRQ stub return: `0xE55EC002` (BIOS `0x144`)
//!
//! Cited: GBATEK BIOS Functions.
//! <https://problemkaputt.de/gbatek.htm>

#[cfg(test)]
mod tests {
    use crate::bus::Bus;
    use crate::cpu::Cpu;
    use crate::hw::Machine;

    const LATCH_POST_BOOT: u32 = 0xE129_F000;
    const LATCH_AFTER_SQRT: u32 = 0xE3A0_2004;
    const LATCH_DURING_IRQ: u32 = 0xE25E_F004;
    const LATCH_AFTER_IRQ: u32 = 0xE55E_C002;

    #[test]
    fn post_boot_latch_on_new_bus() {
        let mut bus = Bus::new(Vec::new());
        assert_eq!(bus.bios_prefetch(), LATCH_POST_BOOT);
        assert_eq!(bus.read32(0), LATCH_POST_BOOT);
    }

    #[test]
    fn bios_opcode_fetch_does_not_wipe_latch() {
        let mut bus = Bus::new(Vec::new());
        assert_eq!(bus.read32(0), LATCH_POST_BOOT);
        assert_eq!(bus.fetch32(0xE4), 0);
        assert_eq!(bus.read32(0), LATCH_POST_BOOT);
    }

    #[test]
    fn sqrt_swi_sets_after_swi_latch() {
        let mut rom = vec![0u8; 0xC0];
        // ARM `swi 0x80000` — comment bits 16..23 = 0x08.
        rom[0..4].copy_from_slice(&0xEF08_0000u32.to_le_bytes());
        let mut bus = Bus::new(rom);
        let mut cpu = Cpu::new();
        cpu.step(&mut bus).unwrap();
        assert_eq!(bus.read32(0), LATCH_AFTER_SQRT);
        assert_eq!(cpu.reg(0), 0);
    }

    #[test]
    fn irq_entry_sets_during_latch_and_jumps_to_handler() {
        let mut bus = Bus::new(vec![0; 0xC0]);
        bus.write32(0x0300_7FFC, 0x0800_0200);
        let mut cpu = Cpu::new();
        cpu.raise_irq(&mut bus);
        assert_eq!(cpu.cpsr() & 0x1F, 0x12);
        assert_eq!(cpu.fetch_pc, 0x0800_0200);
        assert_eq!(cpu.reg(14), 0x138);
        assert_eq!(bus.read32(0), LATCH_DURING_IRQ);
    }

    #[test]
    fn irq_return_stub_sets_after_latch() {
        let mut bus = Bus::new(vec![0; 0xC0]);
        bus.write32(0x0300_7FFC, 0x0800_0200);
        let mut cpu = Cpu::new();
        let game_pc = cpu.fetch_pc;
        cpu.raise_irq(&mut bus);
        assert_eq!(bus.read32(0), LATCH_DURING_IRQ);

        // User ISR: `mov pc, lr` → stub at 0x138.
        cpu.fetch_pc = cpu.reg(14);
        cpu.step(&mut bus).unwrap();
        assert_eq!(bus.read32(0), LATCH_AFTER_IRQ);
        assert_eq!(cpu.cpsr() & 0x1F, 0x1F);
        assert_eq!(cpu.fetch_pc, game_pc);
    }

    #[test]
    fn raise_irq_respects_i_bit() {
        let mut rom = vec![0u8; 0xC0];
        // MSR CPSR_c, #0x9F — System mode with I set.
        rom[0..4].copy_from_slice(&0xE321_F09Fu32.to_le_bytes());
        let mut bus = Bus::new(rom);
        let mut cpu = Cpu::new();
        cpu.step(&mut bus).unwrap();
        assert_ne!(cpu.cpsr() & 0x80, 0);

        bus.write32(0x0300_7FFC, 0x0800_0200);
        let pc_before = cpu.fetch_pc;
        cpu.raise_irq(&mut bus);
        assert_eq!(cpu.fetch_pc, pc_before);
        assert_eq!(bus.bios_prefetch(), LATCH_POST_BOOT);
    }

    #[test]
    fn vblank_intr_wait_halts_until_if_bit_0() {
        let mut machine = Machine::from_rom(vec![0; 0x200]);
        machine.cpu.swi_number_for_test(&mut machine.bus, 0x05);
        assert!(machine.bus.halted);
        machine.bus.irq.raise(1);
        machine.cpu.poll_intr_wait(&mut machine.bus);
        assert!(!machine.bus.halted);
        assert_eq!(machine.bus.read16(0x0300_7FF8), 0);
    }

    #[test]
    fn intr_wait_r0_zero_returns_if_check_already_set() {
        let mut machine = Machine::from_rom(vec![0; 0x200]);
        machine.bus.irq.raise(1);
        machine.cpu.set_reg_for_test(0, 0);
        machine.cpu.set_reg_for_test(1, 1);
        machine.cpu.swi_number_for_test(&mut machine.bus, 0x04);
        assert!(!machine.bus.halted);
        assert_eq!(machine.bus.read16(0x0300_7FF8), 0);
    }

    #[test]
    fn intr_wait_r0_one_ignores_already_set_until_new_raise() {
        let mut machine = Machine::from_rom(vec![0; 0x200]);
        machine.bus.irq.raise(1);
        machine.cpu.set_reg_for_test(0, 1);
        machine.cpu.set_reg_for_test(1, 1);
        machine.cpu.swi_number_for_test(&mut machine.bus, 0x04);
        assert!(machine.bus.halted);
        assert_eq!(machine.bus.read16(0x0300_7FF8), 0);
        machine.bus.irq.raise(1);
        machine.cpu.poll_intr_wait(&mut machine.bus);
        assert!(!machine.bus.halted);
        assert_eq!(machine.bus.read16(0x0300_7FF8), 0);
    }
}
