use super::Timers;

#[test]
fn overflow() {
    let mut timers = Timers::new();
    timers.write16_immediate(0, 0xFFFF);
    timers.write16_immediate(2, 0x80);
    timers.write16_immediate(0, 0);
    let mask = timers.tick(2);
    assert_eq!(mask & 1, 1);
    assert_eq!(timers.counter(0), 0);
}

#[test]
fn cascade() {
    let mut timers = Timers::new();
    timers.write16_immediate(0, 0xFFFF);
    timers.write16_immediate(2, 0x80);
    timers.write16_immediate(4, 0);
    timers.write16_immediate(6, 0x80 | (1 << 2) | 0b11);
    let mask = timers.tick(2);
    assert_eq!(mask & 1, 1);
    assert_eq!(timers.counter(1), 1);
    assert!(timers.cascade(1));
}

#[test]
fn rising_enable_latches_until_next_tick() {
    let mut timers = Timers::new();
    timers.write16(0, 0x00AB);
    timers.write16(2, 0x80);
    assert_eq!(timers.counter(0), 0);
    let _ = timers.tick(1);
    assert_eq!(timers.counter(0), 0x00AB);
}

#[test]
fn enable_while_counter_is_ffff_overflows() {
    let mut timers = Timers::new();
    timers.write16_immediate(0, 0xFFFF);
    timers.write16_immediate(2, 0x80);
    timers.write16_immediate(2, 0);
    assert_eq!(timers.counter(0), 0xFFFF);
    timers.write16(0, 0);
    timers.write16(2, 0xC0);
    let mask = timers.tick(1);
    assert_eq!(mask & 1, 1, "enable at 0xFFFF overflows before the new reload");
    assert_eq!(timers.counter(0), 0);
}

#[test]
fn falling_enable_stops_immediately() {
    let mut timers = Timers::new();
    timers.write16_immediate(0, 0);
    timers.write16_immediate(2, 0x80);
    let _ = timers.tick(1);
    timers.write16_immediate(2, 0);
    assert_eq!(timers.read16(2), 0);
    let before = timers.counter(0);
    let _ = timers.tick(10);
    assert_eq!(timers.counter(0), before);
}
