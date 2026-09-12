//! Unit tests for DMA Immediate (P2).
//!
//! Cited: GBATEK -- GBA DMA Transfers
//!   https://problemkaputt.de/gbatek-gba-dma-transfers.htm
//! Acceptance: register file, Immediate copy, CPU halt while active, SRAM reject.
//! VBlank/HBlank/Special are out of scope (P5).

use super::*;
use crate::bus::{CpuMem, FlatRam};

fn iwram_ram() -> FlatRam {
    // Flat view covering IWRAM primary range for unit tests.
    FlatRam::with_base(0x0300_0000, 0x8000)
}

fn seed_halfwords(mem: &mut FlatRam, base: u32, words: &[u16]) {
    for (i, w) in words.iter().enumerate() {
        mem.write16(base.wrapping_add((i as u32) * 2), *w);
    }
}

fn settle(dma: &mut Dma) {
    dma.tick_startup(2);
}

fn read_halfwords(mem: &mut FlatRam, base: u32, n: usize) -> Vec<u16> {
    (0..n)
        .map(|i| mem.read16(base.wrapping_add((i as u32) * 2)))
        .collect()
}

#[test]
fn register_file_masks_and_mmio() {
    let mut dma = Dma::new();

    dma.write_sad(ChannelId::Ch0, 0xFFFF_FFFF);
    assert_eq!(dma.channel(ChannelId::Ch0).sad, 0x07FF_FFFF);

    dma.write_dad(ChannelId::Ch3, 0x1234_5678);
    assert_eq!(dma.channel(ChannelId::Ch3).dad, 0x0234_5678 & 0x0FFF_FFFF);

    dma.write_count(ChannelId::Ch1, 0xFFFF);
    assert_eq!(dma.channel(ChannelId::Ch1).count, 0x3FFF);

    dma.write_count(ChannelId::Ch3, 0xFFFF);
    assert_eq!(dma.channel(ChannelId::Ch3).count, 0xFFFF);

    // MMIO: DMA0 SAD at offset 0, CNT at offset 8.
    dma.write_mmio32(0, 0x0300_0100);
    assert_eq!(dma.channel(ChannelId::Ch0).sad, 0x0300_0100);

    let cnt = control_word(
        DestControl::Increment,
        SrcControl::Increment,
        StartTiming::Immediate,
        false,
        true,
    );
    dma.write_mmio16(10, cnt); // DMA0 CNT_H
    assert!(!dma.channel(ChannelId::Ch0).pending_immediate);
    settle(&mut dma);
    assert!(dma.channel(ChannelId::Ch0).pending_immediate);
    assert_eq!(dma.read_mmio16(10) & CONTROL_ENABLE, CONTROL_ENABLE);
}

#[test]
fn immediate_16bit_copy_increments() {
    let mut mem = iwram_ram();
    let src = 0x0300_0100;
    let dst = 0x0300_0200;
    seed_halfwords(&mut mem, src, &[0x1111, 0x2222, 0x3333, 0x4444]);

    let mut dma = Dma::new();
    dma.write_sad(ChannelId::Ch3, src);
    dma.write_dad(ChannelId::Ch3, dst);
    dma.write_count(ChannelId::Ch3, 4);
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

    assert!(dma.is_busy());
    assert!(!dma.cpu_halted(), "halted only while actively transferring");

    let report = dma.run_immediate(&mut mem);
    assert_eq!(report.units_transferred, 4);
    assert_eq!(report.completed, vec![ChannelId::Ch3]);
    assert!(report.rejected.is_empty());
    assert!(!dma.cpu_halted());
    assert!(!dma.channel(ChannelId::Ch3).enabled());
    assert_eq!(
        read_halfwords(&mut mem, dst, 4),
        vec![0x1111, 0x2222, 0x3333, 0x4444]
    );
    // Visible regs must not walk during transfer.
    assert_eq!(dma.channel(ChannelId::Ch3).sad, src);
    assert_eq!(dma.channel(ChannelId::Ch3).dad, dst);
}

#[test]
fn immediate_32bit_fixed_dest() {
    let mut mem = iwram_ram();
    let src = 0x0300_0400;
    let dst = 0x0300_0500;
    mem.write32(src, 0xAABB_CCDD);
    mem.write32(src + 4, 0x1122_3344);

    let mut dma = Dma::new();
    dma.write_sad(ChannelId::Ch0, src);
    dma.write_dad(ChannelId::Ch0, dst);
    dma.write_count(ChannelId::Ch0, 2);
    dma.write_control(
        ChannelId::Ch0,
        control_word(
            DestControl::Fixed,
            SrcControl::Increment,
            StartTiming::Immediate,
            true,
            true,
        ),
    );

    let report = dma.run_immediate(&mut mem);
    assert_eq!(report.units_transferred, 2);
    // Last write wins at fixed dest.
    assert_eq!(mem.read32(dst), 0x1122_3344);
}

#[test]
fn zero_count_means_max_for_channel() {
    let mut dma = Dma::new();
    dma.write_sad(ChannelId::Ch2, 0x0300_0000);
    dma.write_dad(ChannelId::Ch2, 0x0300_1000);
    dma.write_count(ChannelId::Ch2, 0);
    dma.write_control(
        ChannelId::Ch2,
        control_word(
            DestControl::Increment,
            SrcControl::Increment,
            StartTiming::Immediate,
            false,
            true,
        ),
    );
    assert_eq!(dma.channel(ChannelId::Ch2).remaining, 0x4000);

    dma.write_count(ChannelId::Ch3, 0);
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
    assert_eq!(dma.channel(ChannelId::Ch3).remaining, 0x1_0000);
}

#[test]
fn cpu_halted_while_active_transfer() {
    use std::cell::Cell;

    struct HaltProbe<'a> {
        inner: FlatRam,
        halted: &'a Cell<bool>,
        saw_halt_during_access: &'a Cell<bool>,
    }

    impl CpuMem for HaltProbe<'_> {
        fn read8(&mut self, addr: u32) -> u8 {
            if self.halted.get() {
                self.saw_halt_during_access.set(true);
            }
            self.inner.read8(addr)
        }
        fn write8(&mut self, addr: u32, value: u8) {
            if self.halted.get() {
                self.saw_halt_during_access.set(true);
            }
            self.inner.write8(addr, value);
        }
    }

    let halted = Cell::new(false);
    let saw = Cell::new(false);
    let mut mem = HaltProbe {
        inner: iwram_ram(),
        halted: &halted,
        saw_halt_during_access: &saw,
    };
    seed_halfwords(&mut mem.inner, 0x0300_0100, &[1, 2, 3]);

    let mut dma = Dma::new();
    let irq = crate::irq::Irq::new();
    dma.write_sad(ChannelId::Ch1, 0x0300_0100);
    dma.write_dad(ChannelId::Ch1, 0x0300_0200);
    dma.write_count(ChannelId::Ch1, 3);
    dma.write_control(
        ChannelId::Ch1,
        control_word(
            DestControl::Increment,
            SrcControl::Increment,
            StartTiming::Immediate,
            false,
            true,
        ),
    );

    assert!(!dma.cpu_halted());
    // Manually begin_active then transfer units while mirroring halt into Cell.
    let idx = ChannelId::Ch1.index();
    dma.channels[idx].begin_active();
    halted.set(dma.cpu_halted());
    assert!(halted.get());

    let mut units = 0u32;
    while dma.channels[idx].remaining > 0 {
        let (sad, dad, word32) = {
            let ch = &dma.channels[idx];
            (ch.latched_sad, ch.latched_dad, ch.transfer32())
        };
        assert!(!word32);
        let v = mem.read16(sad & !1);
        mem.write16(dad & !1, v);
        dma.channels[idx].step_addrs();
        dma.channels[idx].remaining -= 1;
        units += 1;
    }
    let _ = dma.channels[idx].finish();
    halted.set(dma.cpu_halted());
    assert_eq!(units, 3);
    assert!(saw.get(), "bus accesses must occur while CPU is halted");
    assert!(!dma.cpu_halted());
    let _ = irq;
}

#[test]
fn reject_sram_source() {
    let mut mem = FlatRam::with_base(0x0200_0000, 0x40000);
    let mut dma = Dma::new();
    dma.write_sad(ChannelId::Ch3, 0x0E00_0000);
    dma.write_dad(ChannelId::Ch3, 0x0200_1000);
    dma.write_count(ChannelId::Ch3, 4);
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

    let report = dma.run_immediate(&mut mem);
    assert_eq!(report.units_transferred, 0);
    assert!(report.completed.is_empty());
    assert_eq!(
        report.rejected,
        vec![DmaReject::SramWindow {
            channel: ChannelId::Ch3,
            addr: 0x0E00_0000,
        }]
    );
    assert!(!dma.channel(ChannelId::Ch3).enabled());
}

#[test]
fn reject_sram_destination() {
    let mut mem = FlatRam::new(0x100);
    let mut dma = Dma::new();
    dma.write_sad(ChannelId::Ch3, 0x0000_0000);
    dma.write_dad(ChannelId::Ch3, 0x0E00_1234);
    dma.write_count(ChannelId::Ch3, 1);
    dma.write_control(
        ChannelId::Ch3,
        control_word(
            DestControl::Increment,
            SrcControl::Fixed,
            StartTiming::Immediate,
            false,
            true,
        ),
    );

    let report = dma.run_immediate(&mut mem);
    assert_eq!(report.units_transferred, 0);
    assert!(matches!(
        report.rejected.first(),
        Some(DmaReject::SramWindow {
            channel: ChannelId::Ch3,
            addr: 0x0E00_1234
        })
    ));
}

#[test]
fn priority_channel0_before_channel3() {
    let mut mem = iwram_ram();
    seed_halfwords(&mut mem, 0x0300_0100, &[0xA, 0xB]);
    seed_halfwords(&mut mem, 0x0300_0300, &[0xC, 0xD]);

    let mut dma = Dma::new();

    // Arm ch3 first, then ch0 — run order must still be 0 then 3.
    dma.write_sad(ChannelId::Ch3, 0x0300_0300);
    dma.write_dad(ChannelId::Ch3, 0x0300_0400);
    dma.write_count(ChannelId::Ch3, 2);
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

    dma.write_sad(ChannelId::Ch0, 0x0300_0100);
    dma.write_dad(ChannelId::Ch0, 0x0300_0200);
    dma.write_count(ChannelId::Ch0, 2);
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

    let report = dma.run_immediate(&mut mem);
    assert_eq!(
        report.completed,
        vec![ChannelId::Ch0, ChannelId::Ch3],
        "priority must drain 0 before 3"
    );
    assert_eq!(read_halfwords(&mut mem, 0x0300_0200, 2), vec![0xA, 0xB]);
    assert_eq!(read_halfwords(&mut mem, 0x0300_0400, 2), vec![0xC, 0xD]);
}

#[test]
fn non_immediate_start_does_not_arm() {
    let mut dma = Dma::new();
    dma.write_sad(ChannelId::Ch1, 0x0300_0000);
    dma.write_dad(ChannelId::Ch1, 0x0300_0100);
    dma.write_count(ChannelId::Ch1, 1);
    dma.write_control(
        ChannelId::Ch1,
        control_word(
            DestControl::Increment,
            SrcControl::Increment,
            StartTiming::VBlank,
            false,
            true,
        ),
    );
    assert!(!dma.channel(ChannelId::Ch1).pending_immediate);
    assert!(dma.channel(ChannelId::Ch1).enabled());
    assert!(!dma.is_busy());
}

#[test]
fn immediate_ignores_repeat_clears_enable() {
    let mut mem = iwram_ram();
    seed_halfwords(&mut mem, 0x0300_0100, &[0x55AA]);
    let mut dma = Dma::new();
    dma.write_sad(ChannelId::Ch0, 0x0300_0100);
    dma.write_dad(ChannelId::Ch0, 0x0300_0200);
    dma.write_count(ChannelId::Ch0, 1);
    let mut ctrl = control_word(
        DestControl::Increment,
        SrcControl::Increment,
        StartTiming::Immediate,
        false,
        true,
    );
    ctrl |= CONTROL_REPEAT;
    dma.write_control(ChannelId::Ch0, ctrl);

    let _ = dma.run_immediate(&mut mem);
    assert!(
        !dma.channel(ChannelId::Ch0).enabled(),
        "Immediate must clear Enable even if Repeat was set"
    );
    assert!(!dma.is_busy());
}
