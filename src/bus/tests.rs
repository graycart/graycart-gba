use super::Bus;

#[test]
fn iwram_word_roundtrip() {
    let mut bus = Bus::new(Vec::new());
    bus.write32(0x0300_0100, 0xAABB_CCDD);
    assert_eq!(bus.read32(0x0300_0100), 0xAABB_CCDD);
    assert_eq!(bus.read8(0x0300_0100), 0xDD);
}

#[test]
fn unused_openbus_names_region() {
    let mut bus = Bus::new(Vec::new());
    let _ = bus.read32(0x0100_0000);
    let line = bus
        .warn_lines
        .iter()
        .find(|line| line.contains("openbus"))
        .expect("openbus warn");
    assert!(
        line.contains("addr=0x01000000") && line.contains("region=unused"),
        "{line}"
    );
    let _ = bus.read32(0x0100_0000);
    assert_eq!(
        bus.warn_lines
            .iter()
            .filter(|line| line.contains("openbus"))
            .count(),
        1
    );
}

#[test]
fn sram_mirrors_across_0f() {
    let mut rom = vec![0u8; 0xC0];
    rom.extend_from_slice(b"SRAM_V");
    let mut bus = Bus::new(rom);
    bus.write8(0x0F00_0000, 0x5A);
    assert_eq!(bus.read8(0x0E00_0000), 0x5A);
    assert!(
        !bus.warn_lines.iter().any(|line| line.contains("openbus")),
        "0x0F SRAM must not log openbus: {:?}",
        bus.warn_lines
    );
}

#[test]
fn sram_unwritten_is_ff() {
    let mut rom = vec![0u8; 0xC0];
    rom.extend_from_slice(b"SRAM_V");
    let mut bus = Bus::new(rom);
    assert_eq!(bus.read8(0x0E00_0000), 0xFF);
}

#[test]
fn save_none_reads_ff_ignores_write() {
    let mut bus = Bus::new(vec![0u8; 0xC0]);
    assert_eq!(bus.save_kind().name(), "none");
    bus.write8(0x0E00_0000, 0x5A);
    assert_eq!(bus.read8(0x0E00_0000), 0xFF);
    assert_eq!(bus.read8(0x0F00_0000), 0xFF);
}

#[test]
fn save_none_wide_is_openbus_not_duplicate() {
    let mut bus = Bus::new(vec![0u8; 0xC0]);
    // Prime open-bus latch with a known word, then a 16-bit SRAM-region read
    // must slice that latch — not duplicate 0xFF into 0xFFFF.
    bus.write32(0x0300_0000, 0xA1B2_C3D4);
    let _ = bus.read32(0x0300_0000);
    let got = bus.read16(0x0E00_0000);
    assert_ne!(got, 0xFFFF, "none must not duplicate the erased byte");
    assert!(
        bus.warn_lines.iter().any(|l| l.contains("region=sram")),
        "wide none access must log sram openbus: {:?}",
        bus.warn_lines
    );
}

#[test]
fn sram_halfword_write_programs_one_lane() {
    let mut rom = vec![0u8; 0xC0];
    rom.extend_from_slice(b"SRAM_V");
    let mut bus = Bus::new(rom);
    bus.write16(0x0E00_0020, 0xAABB);
    assert_eq!(bus.read8(0x0E00_0020), 0xBB);
    assert_eq!(bus.read8(0x0E00_0021), 0xFF);
    bus.write16(0x0E00_0021, 0xAABB);
    assert_eq!(bus.read8(0x0E00_0021), 0xAA);
}

#[test]
fn eeprom_dma_nine_halfwords_sets_512() {
    let mut rom = vec![0u8; 0xC0];
    rom.extend_from_slice(b"EEPROM_V");
    let mut bus = Bus::new(rom);
    assert_eq!(bus.save_kind().name(), "eeprom");

    // Read-setup stream: cmd 0b11, 6 address bits, stop — 9 halfwords.
    let iwram = 0x0300_0100u32;
    for (i, bit) in [1u16, 1, 0, 0, 0, 0, 0, 0, 0].iter().enumerate() {
        bus.write16(iwram + (i as u32) * 2, *bit);
    }
    // DMA3 immediate halfword copy into EEPROM region.
    bus.write32(0x0400_00D4, iwram);
    bus.write32(0x0400_00D8, 0x0D00_0000);
    bus.write16(0x0400_00DC, 9);
    bus.write16(0x0400_00DE, 1 << 15);

    let bytes = bus.save_bytes().expect("size should lock after setup DMA");
    assert_eq!(bytes.len(), 512);
}

#[test]
fn rom_mirrors_08_0a_0c_same_offset() {
    let mut rom = vec![0u8; 0x100];
    rom[0x40] = 0x11;
    rom[0x41] = 0x22;
    let mut bus = Bus::new(rom);
    let a = bus.read16(0x0800_0040);
    let b = bus.read16(0x0A00_0040);
    let c = bus.read16(0x0C00_0040);
    assert_eq!(a, 0x2211);
    assert_eq!(a, b);
    assert_eq!(a, c);
}

#[test]
fn gpio_warn_once_when_flag_set() {
    let mut rom = vec![0u8; 0x200];
    rom[0xC4] = 0xAB;
    rom[0xC5] = 0xCD;
    let mut bus = Bus::new(rom);
    bus.set_has_gpio(true);
    let got = bus.read16(0x0800_00C4);
    assert_ne!(got, 0xCDAB, "GPIO touch must not return the ROM halfword");
    let warns: Vec<_> = bus
        .warn_lines
        .iter()
        .filter(|line| line.contains("gpio"))
        .collect();
    assert_eq!(warns.len(), 1);
    assert_eq!(warns[0], "gba-debug: warn cart gpio unsupported");
    let _ = bus.read16(0x0800_00C6);
    bus.write16(0x0800_00C8, 0);
    assert_eq!(
        bus.warn_lines
            .iter()
            .filter(|line| line.contains("gpio"))
            .count(),
        1
    );
}

#[test]
fn gpio_off_by_default_reads_rom() {
    let mut rom = vec![0u8; 0x200];
    rom[0xC4] = 0xAB;
    rom[0xC5] = 0xCD;
    let mut bus = Bus::new(rom);
    assert_eq!(bus.read16(0x0800_00C4), 0xCDAB);
    assert!(!bus.warn_lines.iter().any(|line| line.contains("gpio")));
}

#[test]
fn io_past_1k_is_openbus_not_dispcnt_alias() {
    let mut bus = Bus::new(Vec::new());
    bus.write16(0x0400_0000, 0x0404);
    let value = bus.read16(0x0400_0400);
    assert_ne!(value, 0x0404, "0x04000400 must not alias DISPCNT");
    let line = bus
        .warn_lines
        .iter()
        .find(|line| line.contains("openbus"))
        .expect("openbus warn");
    assert!(
        line.contains("addr=0x04000400") && line.contains("region=io"),
        "{line}"
    );
}

#[test]
fn dispstat_high_byte_read_skips_vblank() {
    let mut bus = Bus::new(Vec::new());
    bus.vblank = true;
    // High byte bit 0 clear so a false vblank inject would be visible.
    bus.write16(0x0400_0004, 0x0200);
    assert_eq!(bus.read8(0x0400_0004) & 1, 1);
    assert_eq!(bus.read8(0x0400_0005), 0x02);
    assert_eq!(bus.read8(0x0400_0005) & 1, 0);
}

#[test]
fn vcount_read_returns_live_scanline() {
    let mut bus = Bus::new(Vec::new());
    bus.vcount = 159;
    assert_eq!(bus.read8(0x0400_0006), 159);
    assert_eq!(bus.read8(0x0400_0007), 0);
    assert_eq!(bus.read16(0x0400_0006), 159);
    bus.write16(0x0400_0006, 0xFFFF);
    assert_eq!(bus.read8(0x0400_0006), 159);
    assert_eq!(bus.read8(0x0400_0007), 0);
    assert_eq!(bus.read16(0x0400_0006), 159);
}

#[test]
fn rom_past_end_word_is_two_halfwords() {
    let mut bus = Bus::new(vec![0; 0xC0]);
    let addr = 0x0800_00C0;
    let got = bus.read32(addr);
    let low = ((addr & !3) >> 1) & 0xFFFF;
    let high = (((addr & !3) + 2) >> 1) & 0xFFFF;
    let want = low | (high << 16);
    assert_eq!(got, want);
    assert_ne!(got, low, "must not zero-extend a single halfword");
}

#[test]
fn haltcnt_halfword_lane_zero_does_not_halt() {
    let mut bus = Bus::new(Vec::new());
    bus.write16(0x0400_0300, 0x0001);
    assert!(!bus.halted);
    bus.write8(0x0400_0301, 0);
    assert!(bus.halted);
}

#[test]
fn timer_byte_write_updates_reload_not_counter() {
    let mut bus = Bus::new(Vec::new());
    // Start off; low byte of TM0CNT_L.
    bus.write8(0x0400_0100, 0xAB);
    assert_eq!(bus.read16(0x0400_0100), 0, "live counter stays 0");
    assert_eq!(bus.timers.reload(0), 0x00AB);
    // Start edge loads the reload latch into the counter.
    bus.write16(0x0400_0102, 0x80);
    assert_eq!(bus.timers.counter(0), 0x00AB);
}

#[test]
fn wait_line_at_reset_reports_ws0_n4_s2() {
    let bus = Bus::new(Vec::new());
    let line = bus.wait_line(0, 0);
    assert!(
        line.contains("rom_n=4") && line.contains("rom_s=2") && line.contains("sram=4"),
        "reset WAITCNT must not still print placeholder 1s: {line}"
    );
    assert!(!line.contains("rom_n=1"), "{line}");
}

#[test]
fn rom_sequential_survives_iwram_access() {
    let mut bus = Bus::new(vec![0; 0x100]);

    bus.begin_step();
    let _ = bus.fetch16(0x0800_0000);
    assert_eq!(bus.take_step_cycles(), 4, "first ROM halfword is N");

    bus.begin_step();
    let _ = bus.read32(0x0300_0000);
    assert_eq!(bus.take_step_cycles(), 1, "IWRAM word is 1I");

    bus.begin_step();
    let _ = bus.fetch16(0x0800_0002);
    assert_eq!(
        bus.take_step_cycles(),
        2,
        "cart N/S is address-based; IWRAM must not force N"
    );
}

#[test]
fn sram_access_does_not_force_rom_n_by_address_gap() {
    // Cart N/S stays address-based across 0x0E probes. Flash ROMs idle inside
    // the 60-frame harness. A 32-bit Game Pak access is N+S, not one N or S.
    let mut rom = vec![0u8; 0xC0];
    rom.extend_from_slice(b"SRAM_V");
    let mut bus = Bus::new(rom);

    bus.begin_step();
    let _ = bus.fetch16(0x0800_0000);
    let _ = bus.take_step_cycles();

    bus.begin_step();
    let _ = bus.read8(0x0E00_0000);
    let _ = bus.take_step_cycles();

    bus.begin_step();
    let _ = bus.fetch16(0x0800_0002);
    assert_eq!(bus.take_step_cycles(), 2);
}
