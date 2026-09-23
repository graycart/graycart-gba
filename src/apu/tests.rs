//! Page 9 APU integration tests.

use crate::apu::Apu;
use crate::bus::Bus;

const IWRAM: u32 = 0x0300_0000;
const DMA1: u32 = 0x0400_00BC;
const FIFO_A: u32 = 0x0400_00A0;
const SOUNDCNT_L: u32 = 0x0400_0080;
const SOUNDCNT_X: u32 = 0x0400_0084;
const SOUND2CNT_L: u32 = 0x0400_0068;
const SOUND2CNT_H: u32 = 0x0400_006C;

fn enable_fifo_32() -> u16 {
    // Enable | dest fixed | 32-bit | repeat | timing = FIFO (3).
    (1 << 15) | (2 << 5) | (1 << 10) | (1 << 9) | (3 << 12)
}

fn write_channel(bus: &mut Bus, base: u32, src: u32, dst: u32, count: u16, cnt_h: u16) {
    bus.write32(base, src);
    bus.write32(base + 4, dst);
    bus.write16(base + 8, count);
    bus.write16(base + 10, cnt_h);
}

#[test]
fn psg_enable_produces_nonzero_then_master_off_silences() {
    let mut bus = Bus::new(vec![0; 0xC0]);
    // Master on.
    bus.write16(SOUNDCNT_X, 0x0080);
    // Square 2: volume 15, duty 50%, envelope period 0; restart with freq.
    bus.write16(SOUND2CNT_L, 0xF000 | (2 << 6));
    bus.write16(SOUND2CNT_H, 0x8000 | 0x100);
    // Route square 2 to both sides at full PSG volume.
    bus.write16(SOUNDCNT_L, (2 << 8) | (2 << 12) | 7 | (7 << 4));

    for _ in 0..50_000 {
        bus.tick_apu(0);
    }
    assert!(
        bus.apu.pcm().iter().any(|(l, r)| *l != 0 || *r != 0),
        "expected a non-zero mixed sample with master on"
    );

    bus.write16(SOUNDCNT_X, 0);
    let marked = bus.apu.pcm().len();
    for _ in 0..20_000 {
        bus.tick_apu(0);
    }
    assert!(
        bus.apu
            .pcm()
            .iter()
            .skip(marked)
            .all(|(l, r)| *l == 0 && *r == 0),
        "master off must force later mixed samples to 0"
    );
}

#[test]
fn fifo_refill_copies_iwram_into_fifo_a() {
    let mut bus = Bus::new(vec![0; 0xC0]);
    let words = [0x0403_0201u32, 0x0807_0605, 0x0C0B_0A09, 0x100F_0E0D];
    for (i, word) in words.iter().enumerate() {
        bus.write32(IWRAM + (i as u32) * 4, *word);
    }
    write_channel(&mut bus, DMA1, IWRAM, FIFO_A, 1, enable_fifo_32());

    for _ in 0..8 {
        bus.write32(FIFO_A, 0xFFFF_FFFF);
    }
    assert_eq!(bus.apu.fifo_a.len(), 32);

    for _ in 0..16 {
        assert_eq!(bus.apu.fifo_a.pop(), -1);
    }
    assert!(bus.apu.fifo_a.take_dma_request());
    bus.request_fifo(1);
    assert_eq!(bus.apu.fifo_a.len(), 32);

    // Second half of the original fill, then the IWRAM pattern.
    for _ in 0..16 {
        assert_eq!(bus.apu.fifo_a.pop(), -1);
    }
    let mut got = [0u8; 16];
    for slot in &mut got {
        *slot = bus.apu.fifo_a.pop() as u8;
    }
    let mut expect = [0u8; 16];
    for (i, word) in words.iter().enumerate() {
        expect[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    assert_eq!(got, expect);
}

#[test]
fn sram_is_not_a_fifo_source() {
    let mut bus = Bus::new(vec![0; 0xC0]);
    write_channel(&mut bus, DMA1, 0x0E00_0000, FIFO_A, 1, enable_fifo_32());

    for _ in 0..8 {
        bus.write32(FIFO_A, 0x1111_1111);
    }
    let len_before = bus.apu.fifo_a.len();
    for _ in 0..16 {
        let _ = bus.apu.fifo_a.pop();
    }
    assert!(bus.apu.fifo_a.take_dma_request());
    let len_after_pops = bus.apu.fifo_a.len();
    bus.request_fifo(1);
    assert_eq!(
        bus.apu.fifo_a.len(),
        len_after_pops,
        "SRAM DMA must not grow the FIFO (started full at {len_before})"
    );
    // Request stays asserted when the refill does not push.
    assert!(bus.apu.fifo_a.take_dma_request());
    assert!(
        bus.warn_lines
            .iter()
            .any(|line| line == "gba-debug: sram-dma: rejected"),
        "warn_lines={:?}",
        bus.warn_lines
    );
}

#[test]
fn fifo_byte_and_halfword_stores_push() {
    let mut bus = Bus::new(vec![0; 0xC0]);
    bus.write8(FIFO_A, 0x11);
    bus.write8(FIFO_A + 1, 0x22);
    bus.write16(FIFO_A + 2, 0x4433);
    assert_eq!(bus.apu.fifo_a.len(), 4);
    assert_eq!(bus.apu.fifo_a.pop() as u8, 0x11);
    assert_eq!(bus.apu.fifo_a.pop() as u8, 0x22);
    assert_eq!(bus.apu.fifo_a.pop() as u8, 0x33);
    assert_eq!(bus.apu.fifo_a.pop() as u8, 0x44);

    bus.write16(0x0400_00A4, 0xBBAA);
    bus.write8(0x0400_00A6, 0xCC);
    bus.write8(0x0400_00A7, 0xDD);
    assert_eq!(bus.apu.fifo_b.len(), 4);
    assert_eq!(bus.apu.fifo_b.pop() as u8, 0xAA);
    assert_eq!(bus.apu.fifo_b.pop() as u8, 0xBB);
    assert_eq!(bus.apu.fifo_b.pop() as u8, 0xCC);
    assert_eq!(bus.apu.fifo_b.pop() as u8, 0xDD);
}

#[test]
fn apu_new_starts_silent() {
    let apu = Apu::new();
    assert!(apu.pcm().is_empty());
    assert_eq!(apu.cnt_l(), 0);
    assert_eq!(apu.cnt_h(), 0);
}
