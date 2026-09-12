//! Unit tests for DMA startup delay + post-DMA force-N (G8-dma-delay).
//!
//! Cited: GBATEK — GBA DMA Transfers (Enable 0→1 wait 2 cycles)

use super::*;
use crate::bus::{CpuMem, FlatRam};

fn iwram_ram() -> FlatRam {
    FlatRam::with_base(0x0300_0000, 0x8000)
}

#[test]
fn immediate_enable_waits_two_cycles_before_pending() {
    let mut dma = Dma::new();
    dma.write_sad(ChannelId::Ch0, 0x0300_0100);
    dma.write_dad(ChannelId::Ch0, 0x0300_0200);
    dma.write_count(ChannelId::Ch0, 1);
    dma.write_control(
        ChannelId::Ch0,
        control_word(
            DestControl::Increment,
            SrcControl::Increment,
            StartTiming::Immediate,
            false,
            true,
        ),
    );
    assert!(!dma.channel(ChannelId::Ch0).pending_immediate);
    assert_eq!(dma.channel(ChannelId::Ch0).startup_delay, 2);
    assert!(dma.is_busy());

    dma.tick_startup(1);
    assert_eq!(dma.channel(ChannelId::Ch0).startup_delay, 1);
    assert!(!dma.channel(ChannelId::Ch0).pending_immediate);

    dma.tick_startup(1);
    assert_eq!(dma.channel(ChannelId::Ch0).startup_delay, 0);
    assert!(dma.channel(ChannelId::Ch0).pending_immediate);
}

#[test]
fn after_startup_immediate_still_copies() {
    let mut mem = iwram_ram();
    mem.write16(0x0300_0100, 0xCAFE);
    let mut dma = Dma::new();
    dma.write_sad(ChannelId::Ch3, 0x0300_0100);
    dma.write_dad(ChannelId::Ch3, 0x0300_0200);
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
    dma.tick_startup(2);
    let report = dma.run_pending(&mut mem, &mut crate::irq::Irq::new());
    assert_eq!(report.units_transferred, 1);
    assert_eq!(mem.read16(0x0300_0200), 0xCAFE);
}
