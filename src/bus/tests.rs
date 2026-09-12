//! Region / mirror / Bus storage unit tests (P2 regions stream).
//!
//! Cited: GBATEK — GBA Memory Map / Unpredictable Things
//!   https://problemkaputt.de/gbatek-gba-memory-map.htm
//!   https://problemkaputt.de/gbatek-gba-unpredictable-things.htm

use super::{
    mirror::{
        ewram_offset, io_offset, iwram_offset, oam_offset, palette_offset, rom_offset,
        sram_chip_offset, sram_window_offset, vram_offset,
    },
    region::{decode, BusWidth, Region, EWRAM_SIZE, IWRAM_SIZE, VRAM_SIZE},
    Bus, CpuMem, FlatRam,
};

#[test]
fn decode_primary_ranges() {
    assert_eq!(decode(0x0000_0000), Region::Bios);
    assert_eq!(decode(0x0000_3FFF), Region::Bios);
    assert_eq!(decode(0x0000_4000), Region::UnusedLow);
    assert_eq!(decode(0x01FF_FFFF), Region::UnusedLow);
    assert_eq!(decode(0x0200_0000), Region::Ewram);
    assert_eq!(decode(0x02FF_FFFF), Region::Ewram);
    assert_eq!(decode(0x0300_0000), Region::Iwram);
    assert_eq!(decode(0x0400_0000), Region::Io);
    assert_eq!(decode(0x0500_0000), Region::Palette);
    assert_eq!(decode(0x0600_0000), Region::Vram);
    assert_eq!(decode(0x0700_0000), Region::Oam);
    assert_eq!(decode(0x0800_0000), Region::GamePakRomWs0);
    assert_eq!(decode(0x09FF_FFFF), Region::GamePakRomWs0);
    assert_eq!(decode(0x0A00_0000), Region::GamePakRomWs1);
    assert_eq!(decode(0x0C00_0000), Region::GamePakRomWs2);
    assert_eq!(decode(0x0E00_0000), Region::GamePakSram);
    assert_eq!(decode(0x0FFF_FFFF), Region::GamePakSram);
    assert_eq!(decode(0x1000_0000), Region::UnusedHigh);
}

#[test]
fn bus_widths_match_gbatek_table() {
    assert_eq!(Region::Bios.bus_width(), Some(BusWidth::Bits32));
    assert_eq!(Region::Iwram.bus_width(), Some(BusWidth::Bits32));
    assert_eq!(Region::Io.bus_width(), Some(BusWidth::Bits32));
    assert_eq!(Region::Oam.bus_width(), Some(BusWidth::Bits32));
    assert_eq!(Region::Ewram.bus_width(), Some(BusWidth::Bits16));
    assert_eq!(Region::Palette.bus_width(), Some(BusWidth::Bits16));
    assert_eq!(Region::Vram.bus_width(), Some(BusWidth::Bits16));
    assert_eq!(Region::GamePakRomWs0.bus_width(), Some(BusWidth::Bits16));
    assert_eq!(Region::GamePakSram.bus_width(), Some(BusWidth::Bits8));
    assert_eq!(Region::UnusedLow.bus_width(), None);
}

#[test]
fn ewram_mirrors_every_256kib() {
    assert_eq!(ewram_offset(0x0200_0000), 0);
    assert_eq!(ewram_offset(0x0203_FFFF), 0x3_FFFF);
    assert_eq!(ewram_offset(0x0204_0000), 0);
    assert_eq!(ewram_offset(0x0204_0123), 0x123);
    assert_eq!(ewram_offset(0x02FF_FFFF), 0x3_FFFF);
}

#[test]
fn iwram_mirrors_every_32kib() {
    assert_eq!(iwram_offset(0x0300_0000), 0);
    assert_eq!(iwram_offset(0x0300_7FFF), 0x7_FFF);
    assert_eq!(iwram_offset(0x0300_8000), 0);
    assert_eq!(iwram_offset(0x0301_0001), 1);
}

#[test]
fn palette_and_oam_mirrors_every_1kib() {
    assert_eq!(palette_offset(0x0500_0000), 0);
    assert_eq!(palette_offset(0x0500_0400), 0);
    assert_eq!(palette_offset(0x0500_0410), 0x10);
    assert_eq!(oam_offset(0x0700_0000), 0);
    assert_eq!(oam_offset(0x0700_0400), 0);
    assert_eq!(oam_offset(0x0700_03FF), 0x3FF);
}

#[test]
fn vram_128kib_block_with_upper_32k_mirror() {
    assert_eq!(vram_offset(0x0600_0000), 0);
    assert_eq!(vram_offset(0x0600_FFFF), 0xFFFF);
    assert_eq!(vram_offset(0x0601_0000), 0x1_0000);
    assert_eq!(vram_offset(0x0601_7FFF), 0x1_7FFF);
    assert_eq!(vram_offset(0x0601_8000), 0x1_0000);
    assert_eq!(vram_offset(0x0601_FFFF), 0x1_7FFF);
    assert_eq!(vram_offset(0x0602_0000), 0);
    assert_eq!(vram_offset(0x0603_8000), 0x1_0000);
}

#[test]
fn rom_ws_windows_share_32mib_offset() {
    assert_eq!(rom_offset(0x0800_0000), 0);
    assert_eq!(rom_offset(0x0900_1234), 0x0100_1234);
    assert_eq!(rom_offset(0x0A00_0000), 0);
    assert_eq!(rom_offset(0x0C00_ABCD), 0xABCD);
    assert_eq!(rom_offset(0x09FF_FFFF), 0x01FF_FFFF);
}

#[test]
fn sram_64kib_window_and_32kib_chip() {
    assert_eq!(sram_window_offset(0x0E00_0000), 0);
    assert_eq!(sram_window_offset(0x0E00_FFFF), 0xFFFF);
    assert_eq!(sram_window_offset(0x0E01_0000), 0);
    assert_eq!(sram_window_offset(0x0F00_1234), 0x1234);
    assert_eq!(sram_chip_offset(0x0E00_0000), 0);
    assert_eq!(sram_chip_offset(0x0E00_8000), 0);
    assert_eq!(sram_chip_offset(0x0E00_8001), 1);
}

#[test]
fn io_primary_and_imc_mirror_only() {
    assert_eq!(io_offset(0x0400_0000), Some(0));
    assert_eq!(io_offset(0x0400_03FF), Some(0x3FF));
    assert_eq!(io_offset(0x0400_0800), Some(0x800));
    assert_eq!(io_offset(0x0400_0803), Some(0x803));
    assert_eq!(io_offset(0x0401_0800), Some(0x800));
    assert_eq!(io_offset(0x0402_0802), Some(0x802));
    assert_eq!(io_offset(0x0400_0804), None);
    assert_eq!(io_offset(0x0401_0000), None);
    assert_eq!(io_offset(0x0400_0400), None);
    assert_eq!(io_offset(0x0500_0000), None);
}

#[test]
fn bus_allocates_internal_ram_sizes() {
    let bus = Bus::new();
    assert_eq!(bus.ewram.len(), EWRAM_SIZE);
    assert_eq!(bus.iwram.len(), IWRAM_SIZE);
    assert_eq!(bus.vram.len(), VRAM_SIZE);
    assert!(bus.bios.is_empty());
    assert!(bus.rom.is_empty());
}

#[test]
fn bus_ewram_mirror_roundtrip() {
    let mut bus = Bus::new();
    bus.write8(0x0200_0100, 0xAB);
    assert_eq!(bus.read8(0x0200_0100), 0xAB);
    assert_eq!(bus.read8(0x0204_0100), 0xAB);
    assert_eq!(bus.read8(0x02FC_0100), 0xAB);
}

#[test]
fn bus_iwram_and_vram_mirrors() {
    let mut bus = Bus::new();
    bus.write8(0x0300_0010, 0x11);
    assert_eq!(bus.read8(0x0300_8010), 0x11);

    bus.write16(0x0601_0002, 0xBEEF);
    assert_eq!(bus.read16(0x0601_0002), 0xBEEF);
    assert_eq!(bus.read16(0x0601_8002), 0xBEEF);
    assert_eq!(bus.read16(0x0603_0002), 0xBEEF);
}

#[test]
fn bus_video_strb_oam_ignored_palette_expands() {
    let mut bus = Bus::new();
    // OAM STRB discarded.
    bus.write8(0x0700_0010, 0x01);
    assert_eq!(bus.read32(0x0700_0010), 0);
    // Halfword OAM store still works.
    bus.write16(0x0700_0010, 0x1234);
    assert_eq!(bus.read16(0x0700_0010), 0x1234);

    // Palette STRB → data × 0x0101 at halfword align.
    bus.write8(0x0500_0020, 0x01);
    assert_eq!(bus.read16(0x0500_0020), 0x0101);
}

#[test]
fn bus_video_strb_obj_vram_ignored_by_mode() {
    let mut bus = Bus::new();
    // Mode 0: OBJ VRAM base 0x10000 — STRB ignored.
    bus.io[0] = 0;
    bus.write8(0x0601_0010, 0x02);
    assert_eq!(bus.read8(0x0601_0010), 0);
    // BG VRAM STRB expands.
    bus.write8(0x0600_0010, 0x02);
    assert_eq!(bus.read16(0x0600_0010), 0x0202);

    // Mode 3 bitmap: OBJ base 0x14000.
    bus.io[0] = 3;
    bus.write8(0x0601_4010, 0x03);
    assert_eq!(bus.read8(0x0601_4010), 0);
}

#[test]
fn bus_io_imc_mirror_and_primary() {
    let mut bus = Bus::new();
    bus.write8(0x0400_0004, 0x5A);
    assert_eq!(bus.read8(0x0400_0004), 0x5A);
    // Not mirrored to next 64 KiB page.
    assert_eq!(bus.read8(0x0401_0004), 0);

    bus.write32(0x0400_0800, 0x0D00_0000);
    assert_eq!(bus.read32(0x0400_0800), 0x0D00_0000);
    assert_eq!(bus.read32(0x0401_0800), 0x0D00_0000);
}

#[test]
fn bus_rom_ws_views_share_image() {
    let mut bus = Bus::new();
    bus.rom.resize(0x200, 0);
    bus.rom[0x100] = 0xCD;
    assert_eq!(bus.read8(0x0800_0100), 0xCD);
    assert_eq!(bus.read8(0x0A00_0100), 0xCD);
    assert_eq!(bus.read8(0x0C00_0100), 0xCD);
}

#[test]
fn bus_sram_byte_dup_and_rotate_write() {
    let mut bus = Bus::new();
    bus.sram.resize(0x10000, 0);
    bus.write8(0x0E00_0010, 0x7E);
    assert_eq!(bus.read16(0x0E00_0010), 0x7E7E);
    assert_eq!(bus.read32(0x0E00_0010), 0x7E7E_7E7E);
    // Mirror across 0Fxxxxxx.
    assert_eq!(bus.read8(0x0F00_0010), 0x7E);

    // Halfword write at odd address: ROR by 8 → store high byte of value.
    bus.write16(0x0E00_0011, 0xAABB);
    assert_eq!(bus.read8(0x0E00_0011), 0xAA);
}

#[test]
fn flat_ram_still_works_for_cpu_tests() {
    let mut mem = FlatRam::new(16);
    mem.write32(0, 0x1122_3344);
    assert_eq!(mem.read32(0), 0x1122_3344);
    assert_eq!(Bus::region_of(0x0300_0000), Region::Iwram);
}
