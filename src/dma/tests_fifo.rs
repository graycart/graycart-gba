//! Unit tests for DMA1/2 FIFO Special (P5 G5-fifo).
//!
//! Cited: GBATEK -- GBA DMA Transfers § Sound FIFO
//!   https://problemkaputt.de/gbatek-gba-dma-transfers.htm

use super::*;
use crate::bus::{CpuMem, FlatRam};
use crate::irq::Irq;

/// Flat map covering IWRAM + a slice of I/O so FIFO dest writes land somewhere.
fn fifo_mem() -> FlatRam {
    // Base at 0 so both 0x0300_xxxx and 0x0400_00A0 are in range if large enough.
    // Use IWRAM-sized window and remap FIFO dest into IWRAM for the unit test by
    // writing DAD to an IWRAM stand-in — production dest is 0x040000A0.
    FlatRam::with_base(0x0300_0000, 0x8000)
}

#[test]
fn fifo_request_transfers_four_words_ignores_count() {
    let mut mem = fifo_mem();
    let src = 0x0300_0100;
    let dst = 0x0300_0400; // stand-in for FIFO_A (fixed dest)
    for i in 0..8u32 {
        mem.write32(src + i * 4, 0xA000_0000 + i);
    }

    let mut dma = Dma::new();
    let mut irq = Irq::new();
    dma.write_sad(ChannelId::Ch1, src);
    dma.write_dad(ChannelId::Ch1, dst);
    dma.write_count(ChannelId::Ch1, 99); // ignored
    let mut ctrl = control_word(
        DestControl::Fixed,
        SrcControl::Increment,
        StartTiming::Special,
        false, // size bit ignored → forced 32-bit
        true,
    );
    ctrl |= CONTROL_REPEAT;
    dma.write_control(ChannelId::Ch1, ctrl);

    let report = dma.on_fifo_request(&mut mem, &mut irq, 0b01);
    assert_eq!(report.units_transferred, 4);
    assert_eq!(report.completed, vec![ChannelId::Ch1]);
    // Fixed dest: last word wins.
    assert_eq!(mem.read32(dst), 0xA000_0003);
    assert!(dma.channel(ChannelId::Ch1).enabled());
    // SAD walked by 4 words.
    assert_eq!(dma.channel(ChannelId::Ch1).latched_sad, src + 16);
}

#[test]
fn fifo_dma2_bit_select() {
    let mut mem = fifo_mem();
    mem.write32(0x0300_0100, 0x1111_1111);
    mem.write32(0x0300_0104, 0x2222_2222);
    mem.write32(0x0300_0108, 0x3333_3333);
    mem.write32(0x0300_010C, 0x4444_4444);

    let mut dma = Dma::new();
    let mut irq = Irq::new();
    dma.write_sad(ChannelId::Ch2, 0x0300_0100);
    dma.write_dad(ChannelId::Ch2, 0x0300_0500);
    dma.write_count(ChannelId::Ch2, 1);
    let mut ctrl = control_word(
        DestControl::Fixed,
        SrcControl::Increment,
        StartTiming::Special,
        true,
        true,
    );
    ctrl |= CONTROL_REPEAT;
    dma.write_control(ChannelId::Ch2, ctrl);

    // Bit0 only → DMA1 idle; DMA2 not selected.
    let none = dma.on_fifo_request(&mut mem, &mut irq, 0b01);
    assert_eq!(none.units_transferred, 0);

    let report = dma.on_fifo_request(&mut mem, &mut irq, 0b10);
    assert_eq!(report.units_transferred, 4);
    assert_eq!(report.completed, vec![ChannelId::Ch2]);
}

#[test]
fn fifo_special_forces_fixed_dest_even_when_increment_programmed() {
    // Cited: mGBA GBAAudioScheduleFifoDma — dest control forced Fixed + 32-bit.
    let mut mem = fifo_mem();
    let src = 0x0300_0100;
    let dst = 0x0300_0400;
    for i in 0..4u32 {
        mem.write32(src + i * 4, 0xB000_0000 + i);
        mem.write32(dst + i * 4, 0); // poison neighbors
    }

    let mut dma = Dma::new();
    let mut irq = Irq::new();
    dma.write_sad(ChannelId::Ch1, src);
    dma.write_dad(ChannelId::Ch1, dst);
    dma.write_count(ChannelId::Ch1, 1);
    let mut ctrl = control_word(
        DestControl::Increment, // game mistake / non-Fixed — hardware forces Fixed
        SrcControl::Increment,
        StartTiming::Special,
        false,
        true,
    );
    ctrl |= CONTROL_REPEAT;
    dma.write_control(ChannelId::Ch1, ctrl);

    let report = dma.on_fifo_request(&mut mem, &mut irq, 0b01);
    assert_eq!(report.units_transferred, 4);
    // All four words must land on the same FIFO address (last wins).
    assert_eq!(mem.read32(dst), 0xB000_0003);
    assert_eq!(mem.read32(dst + 4), 0, "must not walk dest into neighbor");
    assert_eq!(
        dma.channel(ChannelId::Ch1).latched_dad,
        dst,
        "latched DAD stays at FIFO"
    );
    assert_eq!(
        dma.channel(ChannelId::Ch1).dest_control(),
        DestControl::Fixed
    );
}

#[test]
fn fifo_dma0_special_never_starts() {
    let mut mem = fifo_mem();
    let mut dma = Dma::new();
    let mut irq = Irq::new();
    dma.write_sad(ChannelId::Ch0, 0x0300_0100);
    dma.write_dad(ChannelId::Ch0, 0x0300_0200);
    dma.write_count(ChannelId::Ch0, 4);
    dma.write_control(
        ChannelId::Ch0,
        control_word(
            DestControl::Fixed,
            SrcControl::Increment,
            StartTiming::Special,
            true,
            true,
        ),
    );
    // Even if we request start directly, Special+Ch0 is rejected.
    assert!(!dma
        .channel_mut(ChannelId::Ch0)
        .request_start(StartTiming::Special));
    let report = dma.on_fifo_request(&mut mem, &mut irq, 0b11);
    assert_eq!(report.units_transferred, 0);
}
