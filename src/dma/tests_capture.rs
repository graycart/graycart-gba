//! Unit tests for DMA3 video-capture stub + Game Pak allow (P5).
//!
//! Cited: GBATEK -- GBA DMA Transfers (Video Capture)
//!   https://problemkaputt.de/gbatek-gba-dma-transfers.htm

use super::*;
use crate::bus::{CpuMem, FlatRam};
use crate::irq::Irq;

#[test]
fn capture_window_bounds() {
    assert!(!capture_window(0));
    assert!(!capture_window(1));
    assert!(capture_window(2));
    assert!(capture_window(161));
    assert!(!capture_window(162));
}

#[test]
fn capture_runs_inside_window_only() {
    let mut mem = FlatRam::with_base(0x0300_0000, 0x8000);
    mem.write16(0x0300_0100, 0xBEEF);
    let mut dma = Dma::new();
    let mut irq = Irq::new();
    dma.write_sad(ChannelId::Ch3, 0x0300_0100);
    dma.write_dad(ChannelId::Ch3, 0x0300_0200);
    dma.write_count(ChannelId::Ch3, 1);
    let mut ctrl = control_word(
        DestControl::Increment,
        SrcControl::Increment,
        StartTiming::Special,
        false,
        true,
    );
    ctrl |= CONTROL_REPEAT;
    dma.write_control(ChannelId::Ch3, ctrl);

    let outside = dma.on_capture_hblank(&mut mem, &mut irq, 1);
    assert_eq!(outside.units_transferred, 0);
    assert!(dma.channel(ChannelId::Ch3).enabled());

    let inside = dma.on_capture_hblank(&mut mem, &mut irq, 2);
    assert_eq!(inside.units_transferred, 1);
    assert_eq!(mem.read16(0x0300_0200), 0xBEEF);
    assert!(dma.channel(ChannelId::Ch3).enabled());

    let stop = dma.on_capture_hblank(&mut mem, &mut irq, 162);
    assert_eq!(stop.units_transferred, 0);
}

#[test]
fn dma3_rom_source_allowed() {
    let mut rom_mem = FlatRam::with_base(0x0800_0000, 0x200);
    rom_mem.write16(0x0800_0000, 0xCAFE);
    rom_mem.write16(0x0800_0100, 0x0000);

    let mut dma = Dma::new();
    let mut irq = Irq::new();
    dma.write_sad(ChannelId::Ch3, 0x0800_0000);
    dma.write_dad(ChannelId::Ch3, 0x0800_0100);
    dma.write_count(ChannelId::Ch3, 1);
    dma.write_control(
        ChannelId::Ch3,
        control_word(
            DestControl::Increment,
            SrcControl::Increment,
            StartTiming::Immediate,
            false,
            true,
        ),
    );
    let report = dma.run_pending(&mut rom_mem, &mut irq);
    assert_eq!(report.units_transferred, 1);
    assert!(report.rejected.is_empty());
    assert_eq!(rom_mem.read16(0x0800_0100), 0xCAFE);
}

#[test]
fn sram_still_rejected_on_all_channels() {
    let mut mem = FlatRam::new(0x100);
    let mut dma = Dma::new();
    let mut irq = Irq::new();
    dma.write_sad(ChannelId::Ch3, 0x0E00_0000);
    dma.write_dad(ChannelId::Ch3, 0x0200_0000);
    dma.write_count(ChannelId::Ch3, 1);
    dma.write_control(
        ChannelId::Ch3,
        control_word(
            DestControl::Increment,
            SrcControl::Increment,
            StartTiming::Immediate,
            false,
            true,
        ),
    );
    let report = dma.run_pending(&mut mem, &mut irq);
    assert_eq!(report.units_transferred, 0);
    assert!(matches!(
        report.rejected.first(),
        Some(DmaReject::SramWindow {
            channel: ChannelId::Ch3,
            ..
        })
    ));
}
