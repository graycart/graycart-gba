//! G4-timing / G4-regs unit gates.
//!
//! Cited: GBATEK — LCD V-Counters / DISPSTAT
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md` §10

use super::regs::LcdRegs;
use super::timing::{Timing, CYCLES_PER_FRAME, CYCLES_PER_LINE, HDRAW_CYCLES, VBLANK_FLAG_LAST};
use crate::irq::{Irq, IRQ_VBLANK};

#[test]
fn line_is_1232_cycles() {
    let mut t = Timing::default();
    let regs = LcdRegs::default();
    let _ = t.step(CYCLES_PER_LINE - 1, &regs);
    assert_eq!(t.vcount, 0);
    let _ = t.step(1, &regs);
    assert_eq!(t.vcount, 1);
    assert_eq!(t.cycle_in_line, 0);
}

#[test]
fn frame_is_228_lines() {
    let mut t = Timing::default();
    let regs = LcdRegs::default();
    let _ = t.step(CYCLES_PER_FRAME, &regs);
    assert_eq!(t.vcount, 0);
}

#[test]
fn vblank_flag_160_to_226_clear_on_227() {
    let mut t = Timing::default();
    let regs = LcdRegs::default();
    // Advance to line 160.
    let _ = t.step(CYCLES_PER_LINE * 160, &regs);
    assert_eq!(t.vcount, 160);
    assert!(t.in_vblank_flag());
    assert_ne!(t.flags & 1, 0);

    // Line 226 still set.
    let _ = t.step(CYCLES_PER_LINE * u32::from(VBLANK_FLAG_LAST - 160), &regs);
    assert_eq!(t.vcount, VBLANK_FLAG_LAST);
    assert!(t.in_vblank_flag());

    // Line 227 clears flag.
    let _ = t.step(CYCLES_PER_LINE, &regs);
    assert_eq!(t.vcount, 227);
    assert!(!t.in_vblank_flag());
    assert_eq!(t.flags & 1, 0);
}

#[test]
fn hblank_flag_after_960() {
    let mut t = Timing::default();
    let regs = LcdRegs::default();
    let _ = t.step(HDRAW_CYCLES - 1, &regs);
    assert!(!t.in_hblank());
    let line = t.step(1, &regs);
    assert_eq!(line, Some(0));
    assert!(t.in_hblank());
    assert_ne!(t.flags & 2, 0);
}

#[test]
fn vblank_irq_on_enter_when_enabled() {
    let mut t = Timing::default();
    let regs = LcdRegs {
        dispstat_w: 1 << 3,
        ..Default::default()
    };
    let mut irq = Irq::default();
    let _ = t.step(CYCLES_PER_LINE * 160, &regs);
    t.service_irqs(&regs, &mut irq);
    assert_ne!(irq.read_if() & IRQ_VBLANK, 0);
}

#[test]
fn dispcnt_bgcnt_roundtrip() {
    let mut regs = LcdRegs::default();
    regs.write16(0x00, 0x0404);
    regs.write16(0x08, 0x0104);
    assert_eq!(regs.bg_mode(), 4);
    assert!(regs.layer_enable(10));
    assert_eq!(regs.read16(0x08, 0, 0), 0x0104);
}

#[test]
fn lyc_match_sets_vcount_flag() {
    let mut t = Timing::default();
    let regs = LcdRegs {
        dispstat_w: 10 << 8,
        ..Default::default()
    };
    let _ = t.step(CYCLES_PER_LINE * 10, &regs);
    assert_eq!(t.vcount, 10);
    assert_ne!(t.flags & 4, 0);
}
