//! Unit tests for timers 0–3 (G3-timer-units).
//!
//! Cited: GBATEK — GBA Timers
//!   https://problemkaputt.de/gbatek-gba-timers.htm
//! Acceptance: reload latch on start 0→1; TMnCNT_L write=reload / read=counter;
//!   prescale F/1,/64,/256,/1024; cascade; overflow → IF bits 3–6; 32-bit
//!   enable+reload order.

use super::*;

fn start(prescale: u16, irq: bool, count_up: bool) -> u16 {
    let mut c = CTRL_START | (prescale & CTRL_PRESCALE);
    if irq {
        c |= CTRL_IRQ;
    }
    if count_up {
        c |= CTRL_COUNT_UP;
    }
    c
}

#[test]
fn cnt_l_write_is_reload_read_is_counter() {
    let mut t = Timers::new();
    t.write_reload(TimerId::Tm0, 0xABCDu16);
    assert_eq!(t.channel(TimerId::Tm0).reload, 0xABCD);
    // Counter still 0 until start edge or overflow.
    assert_eq!(t.read_counter(TimerId::Tm0), 0);
    assert_eq!(t.read_mmio16(0), 0);

    t.write_control(TimerId::Tm0, start(0, false, false));
    assert_eq!(t.read_counter(TimerId::Tm0), 0xABCD);
    assert_eq!(t.read_mmio16(0), 0xABCD);

    // Writing reload while running does not snap the counter.
    t.write_reload(TimerId::Tm0, 0x1111);
    assert_eq!(t.channel(TimerId::Tm0).reload, 0x1111);
    assert_eq!(t.read_counter(TimerId::Tm0), 0xABCD);
}

#[test]
fn start_0_to_1_loads_reload() {
    let mut t = Timers::new();
    t.write_reload(TimerId::Tm1, 0xFFF0);
    t.write_control(TimerId::Tm1, 0); // stopped
    assert_eq!(t.read_counter(TimerId::Tm1), 0);

    t.write_control(TimerId::Tm1, start(0, false, false));
    assert_eq!(t.read_counter(TimerId::Tm1), 0xFFF0);

    // Already running: start stays 1 — no second reload latch.
    t.write_reload(TimerId::Tm1, 0x0001);
    t.write_control(TimerId::Tm1, start(0, false, false));
    assert_eq!(t.read_counter(TimerId::Tm1), 0xFFF0);
}

#[test]
fn write_cnt32_enable_uses_new_reload() {
    let mut t = Timers::new();
    // Stale reload if halfwords were applied wrong-order.
    t.write_reload(TimerId::Tm2, 0x0000);
    t.write_control(TimerId::Tm2, 0);

    let value = u32::from(0xFFFE_u16) | (u32::from(start(0, false, false)) << 16);
    t.write_cnt32(TimerId::Tm2, value);
    assert_eq!(t.channel(TimerId::Tm2).reload, 0xFFFE);
    assert_eq!(t.read_counter(TimerId::Tm2), 0xFFFE);
    assert!(t.channel(TimerId::Tm2).enabled());

    // Same via MMIO 32-bit at TM2 offset 8.
    let mut t2 = Timers::new();
    t2.write_mmio32(8, value);
    assert_eq!(t2.read_counter(TimerId::Tm2), 0xFFFE);
}

#[test]
fn prescale_f1_f64_f256_f1024() {
    for (sel, period) in [(0u16, 1u64), (1, 64), (2, 256), (3, 1024)] {
        let mut t = Timers::new();
        let mut irq = RecordingIrq::default();
        t.write_cnt32(
            TimerId::Tm0,
            u32::from(0xFFFDu16) | (u32::from(start(sel, false, false)) << 16),
        );
        // Need `period` cycles per counter tick; 3 ticks → counter 0xFFFF.
        t.step(period * 2, &mut irq);
        assert_eq!(
            t.read_counter(TimerId::Tm0),
            0xFFFF,
            "prescale sel={sel} after 2 ticks"
        );
        t.step(period, &mut irq);
        // Third tick overflows → reload 0xFFFD.
        assert_eq!(
            t.read_counter(TimerId::Tm0),
            0xFFFD,
            "prescale sel={sel} overflow reload"
        );
        assert_eq!(irq.if_bits, 0, "IRQ disabled");
        assert_eq!(prescale_period(sel), period as u32);
    }
}

#[test]
fn overflow_sets_if_bits_3_through_6() {
    for (id, bit) in [
        (TimerId::Tm0, IRQ_TIMER0),
        (TimerId::Tm1, IRQ_TIMER1),
        (TimerId::Tm2, IRQ_TIMER2),
        (TimerId::Tm3, IRQ_TIMER3),
    ] {
        let mut t = Timers::new();
        let mut irq = RecordingIrq::default();
        t.write_cnt32(
            id,
            u32::from(0xFFFFu16) | (u32::from(start(0, true, false)) << 16),
        );
        t.step(1, &mut irq);
        assert_eq!(
            irq.if_bits, bit,
            "{id:?} overflow must set IF bit {:#x}",
            bit
        );
        assert_eq!(t.read_counter(id), 0xFFFF, "reload after overflow");
    }
}

#[test]
fn overflow_without_irq_enable_does_not_raise() {
    let mut t = Timers::new();
    let mut irq = RecordingIrq::default();
    t.write_cnt32(
        TimerId::Tm0,
        u32::from(0xFFFFu16) | (u32::from(start(0, false, false)) << 16),
    );
    t.step(1, &mut irq);
    assert_eq!(irq.if_bits, 0);
    assert_eq!(t.read_counter(TimerId::Tm0), 0xFFFF);
}

#[test]
fn cascade_counts_previous_overflows() {
    let mut t = Timers::new();
    let mut irq = RecordingIrq::default();

    // TM0: reload 0xFFFF, F/1, IRQ off → every cycle overflows.
    t.write_cnt32(
        TimerId::Tm0,
        u32::from(0xFFFFu16) | (u32::from(start(0, false, false)) << 16),
    );
    // TM1: cascade from TM0, start at 0xFFFD, IRQ on.
    t.write_cnt32(
        TimerId::Tm1,
        u32::from(0xFFFDu16) | (u32::from(start(0, true, true)) << 16),
    );

    // 3 TM0 overflows → TM1 ticks thrice → overflows once (0xFFFD,E,F then ovf).
    t.step(3, &mut irq);
    assert_eq!(t.read_counter(TimerId::Tm0), 0xFFFF);
    assert_eq!(t.read_counter(TimerId::Tm1), 0xFFFD);
    assert_eq!(irq.if_bits, IRQ_TIMER1);
}

#[test]
fn cascade_chain_tm0_tm1_tm2() {
    let mut t = Timers::new();
    let mut irq = RecordingIrq::default();

    t.write_cnt32(
        TimerId::Tm0,
        u32::from(0xFFFFu16) | (u32::from(start(0, false, false)) << 16),
    );
    t.write_cnt32(
        TimerId::Tm1,
        u32::from(0xFFFFu16) | (u32::from(start(0, false, true)) << 16),
    );
    t.write_cnt32(
        TimerId::Tm2,
        u32::from(0xFFFEu16) | (u32::from(start(0, true, true)) << 16),
    );

    // 2 system cycles → 2 TM0 ovf → 2 TM1 ovf → TM2: 0xFFFE→FFFF→ovf→FFFE.
    t.step(2, &mut irq);
    assert_eq!(t.read_counter(TimerId::Tm2), 0xFFFE);
    assert_eq!(irq.if_bits, IRQ_TIMER2);
}

#[test]
fn cascade_ignores_prescale_on_child() {
    let mut t = Timers::new();
    let mut irq = RecordingIrq::default();

    t.write_cnt32(
        TimerId::Tm0,
        u32::from(0xFFFFu16) | (u32::from(start(0, false, false)) << 16),
    );
    // Child claims F/1024 but count-up → each parent overflow is one tick.
    t.write_cnt32(
        TimerId::Tm1,
        u32::from(0xFFFFu16) | (u32::from(start(3, true, true)) << 16),
    );

    t.step(1, &mut irq);
    assert_eq!(irq.if_bits, IRQ_TIMER1);
    assert_eq!(t.read_counter(TimerId::Tm1), 0xFFFF);
}

#[test]
fn stopped_timer_does_not_advance() {
    let mut t = Timers::new();
    let mut irq = RecordingIrq::default();
    t.write_reload(TimerId::Tm0, 0x0000);
    t.write_control(TimerId::Tm0, 0); // stopped; counter still 0
    t.step(1000, &mut irq);
    assert_eq!(t.read_counter(TimerId::Tm0), 0);
}

#[test]
fn mmio_offsets_match_gbatek_map() {
    assert_eq!(TM0CNT_L_ADDR, 0x0400_0100);
    assert_eq!(Timers::mmio_offset(TimerId::Tm0), 0);
    assert_eq!(Timers::mmio_offset(TimerId::Tm1), 4);
    assert_eq!(Timers::mmio_offset(TimerId::Tm2), 8);
    assert_eq!(Timers::mmio_offset(TimerId::Tm3), 12);
    assert_eq!(TimerId::Tm0.irq_bit(), IRQ_TIMER0);
    assert_eq!(TimerId::Tm3.irq_bit(), IRQ_TIMER3);
}

#[test]
fn irq_raise_closure_adapter() {
    let mut t = Timers::new();
    let mut bits = 0u16;
    t.write_cnt32(
        TimerId::Tm3,
        u32::from(0xFFFFu16) | (u32::from(start(0, true, false)) << 16),
    );
    t.step(1, &mut |b: u16| bits |= b);
    assert_eq!(bits, IRQ_TIMER3);
}

#[test]
fn overflow_raises_via_crate_irq_public_api() {
    let mut t = Timers::new();
    let mut irq = crate::irq::Irq::new();
    t.write_cnt32(
        TimerId::Tm0,
        u32::from(0xFFFFu16) | (u32::from(start(0, true, false)) << 16),
    );
    t.step(1, &mut irq);
    assert_eq!(irq.if_flags(), IRQ_TIMER0);
    // IF latches without IME/IE (GBATEK).
    assert!(!irq.ime());
    assert_eq!(irq.ie(), 0);
}
