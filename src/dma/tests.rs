//! DMA integration tests through Bus / Machine.

use crate::bus::Bus;
use crate::hw::Machine;

const IWRAM: u32 = 0x0300_0000;
const DMA0: u32 = 0x0400_00B0;
const DMA1: u32 = 0x0400_00BC;
const DMA3: u32 = 0x0400_00D4;
const CYCLES_PER_LINE: u64 = 1232;
const HBLANK_START: u64 = 960;

fn enable_immediate() -> u16 {
    1 << 15
}

fn enable_vblank_repeat() -> u16 {
    (1 << 15) | (1 << 12) | (1 << 9)
}

fn enable_hblank() -> u16 {
    (1 << 15) | (2 << 12)
}

fn enable_fifo_repeat() -> u16 {
    (1 << 15) | (3 << 12) | (1 << 9)
}

fn write_channel(bus: &mut Bus, base: u32, src: u32, dst: u32, count: u16, cnt_h: u16) {
    bus.write32(base, src);
    bus.write32(base + 4, dst);
    bus.write16(base + 8, count);
    bus.write16(base + 10, cnt_h);
}

#[test]
fn immediate_copies_iwram_word_and_clears_enable() {
    let mut bus = Bus::new(vec![0; 0xC0]);
    bus.write32(IWRAM, 0xA1B2_C3D4);
    write_channel(
        &mut bus,
        DMA0,
        IWRAM,
        IWRAM + 0x100,
        1,
        enable_immediate() | (1 << 10),
    );
    assert_eq!(bus.read32(IWRAM + 0x100), 0xA1B2_C3D4);
    assert_eq!(bus.read16(DMA0 + 10) & (1 << 15), 0);
}

#[test]
fn vblank_repeat_fires_once_on_rising_edge() {
    let mut machine = Machine::from_rom(vec![0; 0xC0]);
    machine.bus.write32(IWRAM, 0x1111_2222);
    write_channel(
        &mut machine.bus,
        DMA0,
        IWRAM,
        IWRAM + 0x200,
        1,
        enable_vblank_repeat() | (1 << 10),
    );
    assert_eq!(machine.bus.read32(IWRAM + 0x200), 0);
    assert_ne!(machine.bus.read16(DMA0 + 10) & (1 << 15), 0);

    // Reach the rising edge into line 160 (vblank).
    let target = 160 * CYCLES_PER_LINE;
    machine.run_cycles(target - machine.cycles);

    assert_eq!(machine.bus.read32(IWRAM + 0x200), 0x1111_2222);
    assert_ne!(
        machine.bus.read16(DMA0 + 10) & (1 << 15),
        0,
        "repeat leaves enable set"
    );
}

#[test]
fn hblank_fires_once_on_rising_edge() {
    let mut machine = Machine::from_rom(vec![0; 0xC0]);
    machine.bus.write32(IWRAM, 0x3333_4444);
    write_channel(
        &mut machine.bus,
        DMA0,
        IWRAM,
        IWRAM + 0x300,
        1,
        enable_hblank() | (1 << 10),
    );
    assert_eq!(machine.bus.read32(IWRAM + 0x300), 0);

    machine.run_cycles(HBLANK_START - machine.cycles);
    assert_eq!(machine.bus.read32(IWRAM + 0x300), 0x3333_4444);
    assert_eq!(machine.bus.read16(DMA0 + 10) & (1 << 15), 0);
}

#[test]
fn fifo_request_copies_four_words_into_fifo_a() {
    let mut bus = Bus::new(vec![0; 0xC0]);
    for i in 0..4u32 {
        bus.write32(IWRAM + i * 4, 0x10 + i);
    }
    // Dest register points at IWRAM with incrementing control — FIFO timing must
    // still force all four words into 0x040000A0.
    write_channel(
        &mut bus,
        DMA1,
        IWRAM,
        IWRAM + 0x400,
        1,
        enable_fifo_repeat(),
    );
    assert_eq!(bus.read32(IWRAM + 0x400), 0);
    assert_eq!(bus.apu.fifo_a.len(), 0);

    bus.request_fifo(1);

    assert_eq!(
        bus.read32(IWRAM + 0x400),
        0,
        "IWRAM dest must not receive FIFO DMA"
    );
    assert_eq!(bus.apu.fifo_a.len(), 16);
    for i in 0..4u32 {
        let word = 0x10u32 + i;
        for b in word.to_le_bytes() {
            assert_eq!(bus.apu.fifo_a.pop() as u8, b);
        }
    }
    assert_ne!(bus.read16(DMA1 + 10) & (1 << 15), 0);
}

#[test]
fn dma3_game_pak_rom_to_iwram() {
    let mut rom = vec![0u8; 0xC0];
    rom[0..4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
    let mut bus = Bus::new(rom);
    write_channel(
        &mut bus,
        DMA3,
        0x0800_0000,
        IWRAM + 0x500,
        1,
        enable_immediate() | (1 << 10),
    );
    assert_eq!(bus.read32(IWRAM + 0x500), 0xDEAD_BEEF);
}

#[test]
fn sram_source_rejected_no_copy() {
    let mut bus = Bus::new(vec![0; 0xC0]);
    bus.write32(IWRAM + 0x600, 0x55AA_55AA);
    write_channel(
        &mut bus,
        DMA3,
        0x0E00_0000,
        IWRAM + 0x600,
        1,
        enable_immediate() | (1 << 10),
    );
    assert_eq!(bus.read32(IWRAM + 0x600), 0x55AA_55AA);
    assert_eq!(bus.read16(DMA3 + 10) & (1 << 15), 0);
    assert_eq!(
        bus.warn_lines,
        vec!["gba-debug: sram-dma: rejected".to_string()]
    );
}

#[test]
fn stall_blocks_cpu_for_copied_units() {
    // mov r0, #1 then b .
    let mut rom = vec![0u8; 0xC0];
    rom[0..4].copy_from_slice(&0xE3A0_0001u32.to_le_bytes());
    rom[4..8].copy_from_slice(&0xEAFF_FFFEu32.to_le_bytes());

    let mut machine = Machine::from_rom(rom);
    machine.bus.write32(IWRAM, 1);
    machine.bus.write32(IWRAM + 4, 2);
    machine.bus.write32(IWRAM + 8, 3);
    machine.bus.write32(IWRAM + 12, 4);
    write_channel(
        &mut machine.bus,
        DMA0,
        IWRAM,
        IWRAM + 0x700,
        4,
        enable_immediate() | (1 << 10),
    );
    assert_eq!(machine.bus.dma.stall, 4);
    assert_eq!(machine.cpu.fetch_pc, 0x0800_0000);

    machine.run_cycles(4);
    assert_eq!(machine.cpu.fetch_pc, 0x0800_0000);
    assert_eq!(machine.cpu.exec_pc, 0x0800_0000);
    assert_eq!(machine.bus.dma.stall, 0);

    machine.run_cycles(1);
    assert_eq!(machine.cpu.exec_pc, 0x0800_0000);
    assert_eq!(machine.cpu.fetch_pc, 0x0800_0004);
    assert_eq!(machine.cpu.reg(0), 1);
}

#[test]
fn immediate_dest_dma_control_does_not_loop() {
    let mut bus = Bus::new(vec![0; 0xC0]);
    // Word destined for DMA1 CNT_L|CNT_H: count 1, enable+immediate+32-bit.
    let poison = u32::from(1u16) | (u32::from(enable_immediate() | (1 << 10)) << 16);
    bus.write32(IWRAM, poison);
    write_channel(
        &mut bus,
        DMA0,
        IWRAM,
        DMA1 + 8,
        1,
        enable_immediate() | (1 << 10),
    );
    // Outer channel finished; nested enable must not re-enter / panic.
    assert_eq!(bus.read16(DMA0 + 10) & (1 << 15), 0);
    assert!(!bus.dma.busy);
}

#[test]
fn hblank_does_not_fire_on_vblank_line() {
    let mut machine = Machine::from_rom(vec![0; 0xC0]);
    // Reach line 160 before arming so visible-line HBlank edges never see the channel.
    machine.run_cycles(160 * CYCLES_PER_LINE);
    assert!(machine.bus.vblank);
    assert_eq!(machine.bus.vcount, 160);

    machine.bus.write32(IWRAM, 0xABCD_EF01);
    write_channel(
        &mut machine.bus,
        DMA0,
        IWRAM,
        IWRAM + 0x800,
        1,
        enable_hblank() | (1 << 10),
    );
    assert_eq!(machine.bus.read32(IWRAM + 0x800), 0);

    machine.run_cycles(HBLANK_START);
    assert_eq!(
        machine.bus.read32(IWRAM + 0x800),
        0,
        "HBlank DMA must not copy during VBlank"
    );
    assert_ne!(
        machine.bus.read16(DMA0 + 10) & (1 << 15),
        0,
        "channel stays armed when the edge is suppressed"
    );
}

#[test]
fn unsupported_special_timing_clears_enable() {
    let mut bus = Bus::new(vec![0; 0xC0]);
    // DMA0 timing=3 (video capture / special): reason is None.
    write_channel(
        &mut bus,
        DMA0,
        IWRAM,
        IWRAM + 0x100,
        1,
        (1 << 15) | (3 << 12) | (1 << 10),
    );
    assert_eq!(bus.read16(DMA0 + 10) & (1 << 15), 0);
    assert_eq!(bus.read32(IWRAM + 0x100), 0);
}

#[test]
fn debug_line_after_immediate_transfer() {
    let mut bus = Bus::new(vec![0; 0xC0]);
    bus.write32(IWRAM, 1);
    bus.write32(IWRAM + 4, 2);
    bus.write32(IWRAM + 8, 3);
    write_channel(
        &mut bus,
        DMA0,
        IWRAM,
        IWRAM + 0x900,
        3,
        enable_immediate() | (1 << 10),
    );
    let line = bus.dma.debug_line(7);
    assert!(line.contains("ch=0"), "{line}");
    assert!(line.contains("words=3"), "{line}");
    assert!(line.contains("reason=immediate"), "{line}");
    assert!(line.contains("frame=7"), "{line}");
}
