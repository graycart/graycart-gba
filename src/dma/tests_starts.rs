//! Unit tests for VBlank / HBlank DMA starts (P5 G5-vblank / G5-hblank).
//!
//! Cited: GBATEK -- GBA DMA Transfers
//!   https://problemkaputt.de/gbatek-gba-dma-transfers.htm

use super::*;
use crate::bus::{CpuMem, FlatRam};
use crate::irq::{Irq, IRQ_DMA0, IRQ_DMA3};

fn iwram_ram() -> FlatRam {
    FlatRam::with_base(0x0300_0000, 0x8000)
}

fn seed_halfwords(mem: &mut FlatRam, base: u32, words: &[u16]) {
    for (i, w) in words.iter().enumerate() {
        mem.write16(base.wrapping_add((i as u32) * 2), *w);
    }
}

fn read_halfwords(mem: &mut FlatRam, base: u32, n: usize) -> Vec<u16> {
    (0..n)
        .map(|i| mem.read16(base.wrapping_add((i as u32) * 2)))
        .collect()
}

#[test]
fn vblank_start_copies_once() {
    let mut mem = iwram_ram();
    let src = 0x0300_0100;
    let dst = 0x0300_0200;
    seed_halfwords(&mut mem, src, &[0x1111, 0x2222]);

    let mut dma = Dma::new();
    let mut irq = Irq::new();
    dma.write_sad(ChannelId::Ch0, src);
    dma.write_dad(ChannelId::Ch0, dst);
    dma.write_count(ChannelId::Ch0, 2);
    dma.write_control(
        ChannelId::Ch0,
        control_word(
            DestControl::Increment,
            SrcControl::Increment,
            StartTiming::VBlank,
            false,
            true,
        ),
    );
    assert!(!dma.is_busy());

    let report = dma.on_vblank(&mut mem, &mut irq);
    assert_eq!(report.units_transferred, 2);
    assert_eq!(report.completed, vec![ChannelId::Ch0]);
    assert!(!dma.channel(ChannelId::Ch0).enabled());
    assert_eq!(read_halfwords(&mut mem, dst, 2), vec![0x1111, 0x2222]);
}

#[test]
fn vblank_repeat_keeps_enable_and_reloads() {
    let mut mem = iwram_ram();
    seed_halfwords(&mut mem, 0x0300_0100, &[1, 2]);
    let mut dma = Dma::new();
    let mut irq = Irq::new();
    dma.write_sad(ChannelId::Ch3, 0x0300_0100);
    dma.write_dad(ChannelId::Ch3, 0x0300_0200);
    dma.write_count(ChannelId::Ch3, 2);
    let mut ctrl = control_word(
        DestControl::IncrementReload,
        SrcControl::Increment,
        StartTiming::VBlank,
        false,
        true,
    );
    ctrl |= CONTROL_REPEAT;
    dma.write_control(ChannelId::Ch3, ctrl);

    let _ = dma.on_vblank(&mut mem, &mut irq);
    assert!(dma.channel(ChannelId::Ch3).enabled());
    assert_eq!(dma.channel(ChannelId::Ch3).remaining, 2);
    // Inc+Reload restored DAD latch to visible dad.
    assert_eq!(dma.channel(ChannelId::Ch3).latched_dad, 0x0300_0200);

    seed_halfwords(&mut mem, 0x0300_0100, &[3, 4]);
    // SAD walked; reset visible+latch for second burst source in this unit test.
    dma.write_sad(ChannelId::Ch3, 0x0300_0100);
    dma.channel_mut(ChannelId::Ch3).latched_sad = 0x0300_0100;
    let report = dma.on_vblank(&mut mem, &mut irq);
    assert_eq!(report.units_transferred, 2);
    assert_eq!(read_halfwords(&mut mem, 0x0300_0200, 2), vec![3, 4]);
}

#[test]
fn hblank_oam_requires_interval_free() {
    let mut mem = FlatRam::with_base(0x0300_0000, 0x10000);
    // Also map a fake OAM window via FlatRam at 0x0700_0000 — use separate mem for dest.
    // FlatRam is single-base; copy into IWRAM dest proxy then check skip vs run with
    // latched DAD in OAM region (SRAM-style reject not applied to OAM).
    let mut dma = Dma::new();
    let mut irq = Irq::new();
    dma.write_sad(ChannelId::Ch0, 0x0300_0100);
    dma.write_dad(ChannelId::Ch0, OAM_REGION_BASE);
    dma.write_count(ChannelId::Ch0, 1);
    mem.write16(0x0300_0100, 0xABCD);
    dma.write_control(
        ChannelId::Ch0,
        control_word(
            DestControl::Fixed,
            SrcControl::Fixed,
            StartTiming::HBlank,
            false,
            true,
        ),
    );

    let skipped = dma.on_hblank(&mut mem, &mut irq, false);
    assert_eq!(skipped.units_transferred, 0);
    assert!(dma.channel(ChannelId::Ch0).enabled());

    let ran = dma.on_hblank(&mut mem, &mut irq, true);
    assert_eq!(ran.units_transferred, 1);
    assert_eq!(ran.completed, vec![ChannelId::Ch0]);
}

#[test]
fn hblank_non_oam_runs_without_interval_free() {
    let mut mem = iwram_ram();
    seed_halfwords(&mut mem, 0x0300_0100, &[0x55]);
    let mut dma = Dma::new();
    let mut irq = Irq::new();
    dma.write_sad(ChannelId::Ch1, 0x0300_0100);
    dma.write_dad(ChannelId::Ch1, 0x0300_0200);
    dma.write_count(ChannelId::Ch1, 1);
    dma.write_control(
        ChannelId::Ch1,
        control_word(
            DestControl::Increment,
            SrcControl::Increment,
            StartTiming::HBlank,
            false,
            true,
        ),
    );
    let report = dma.on_hblank(&mut mem, &mut irq, false);
    assert_eq!(report.units_transferred, 1);
    assert_eq!(mem.read16(0x0300_0200), 0x55);
}

#[test]
fn completion_irq_bits() {
    let mut mem = iwram_ram();
    seed_halfwords(&mut mem, 0x0300_0100, &[1]);
    let mut dma = Dma::new();
    let mut irq = Irq::new();
    dma.write_sad(ChannelId::Ch0, 0x0300_0100);
    dma.write_dad(ChannelId::Ch0, 0x0300_0200);
    dma.write_count(ChannelId::Ch0, 1);
    let mut ctrl = control_word(
        DestControl::Increment,
        SrcControl::Increment,
        StartTiming::VBlank,
        false,
        true,
    );
    ctrl |= CONTROL_IRQ;
    dma.write_control(ChannelId::Ch0, ctrl);

    let report = dma.on_vblank(&mut mem, &mut irq);
    assert_eq!(report.irq_raised & IRQ_DMA0, IRQ_DMA0);
    assert_ne!(irq.read_if() & IRQ_DMA0, 0);
    assert_eq!(irq.read_if() & IRQ_DMA3, 0);
}
