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
    let mut bus = Bus::new(Vec::new());
    bus.write8(0x0F00_0000, 0x5A);
    assert_eq!(bus.read8(0x0E00_0000), 0x5A);
    assert!(
        !bus.warn_lines.iter().any(|line| line.contains("openbus")),
        "0x0F SRAM must not log openbus: {:?}",
        bus.warn_lines
    );
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
