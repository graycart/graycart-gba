//! Unit tests for Game Pak prefetch FSM (G8-prefetch).
//!
//! Cited: GBATEK — GBA GamePak Prefetch
//!   https://problemkaputt.de/gbatek-gba-gamepak-prefetch.htm

use super::prefetch::{PrefetchBuffer, PREFETCH_CAPACITY};
use super::wait::{AccessKind, RomWindow, WaitTables};
use super::waitcnt::WaitCnt;

fn commercial_tables() -> WaitTables {
    WaitTables::from_waitcnt(WaitCnt::from_u16(0x4317))
}

#[test]
fn disabled_by_default_and_drains_on_disable() {
    let mut p = PrefetchBuffer::new();
    assert!(!p.enabled());
    assert!(p.is_empty());
    p.set_enabled(true);
    p.restart(0x0800_0000, commercial_tables());
    let rom = [0x1111u16, 0x2222, 0x3333, 0x4444];
    p.tick_idle(16, commercial_tables(), |addr| {
        let i = ((addr - 0x0800_0000) / 2) as usize;
        rom[i % rom.len()]
    });
    assert!(!p.is_empty());
    p.set_enabled(false);
    assert!(!p.enabled());
    assert!(p.is_empty());
}

#[test]
fn fills_up_to_capacity_then_hits() {
    let mut p = PrefetchBuffer::new();
    p.set_enabled(true);
    let tables = commercial_tables();
    // WS0 S waits = 1 → 2 cycles/half after first N (waits=3 → 4 cycles).
    p.restart(0x0800_0100, tables);
    let mut img = [0u16; 16];
    for (i, slot) in img.iter_mut().enumerate() {
        *slot = 0xA000 + i as u16;
    }
    // Plenty of idle cycles to fill 8 halfwords.
    p.tick_idle(200, tables, |addr| {
        let i = ((addr - 0x0800_0100) / 2) as usize;
        img[i]
    });
    assert_eq!(p.len(), PREFETCH_CAPACITY);
    assert!(!p.is_filling());

    for i in 0..PREFETCH_CAPACITY {
        let got = p.try_fetch_half(0x0800_0100 + (i as u32) * 2);
        assert_eq!(got, Some(0xA000 + i as u16));
    }
    assert!(p.is_empty());
}

#[test]
fn miss_drains_buffer() {
    let mut p = PrefetchBuffer::new();
    p.set_enabled(true);
    let tables = WaitTables::power_on();
    p.restart(0x0800_0000, tables);
    p.tick_idle(40, tables, |_| 0xBEEF);
    assert!(!p.is_empty());
    assert!(p.try_fetch_half(0x0800_0100).is_none());
    assert!(p.is_empty());
}

#[test]
fn hit_cycles_are_one() {
    assert_eq!(PrefetchBuffer::hit_cycles(), 1);
    let _ = AccessKind::Seq;
    let _ = RomWindow::Ws0;
}
