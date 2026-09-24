use super::Timers;

#[test]
fn overflow() {
    let mut timers = Timers::new();
    // Live counter loads 0xFFFF on start; reload is then set to 0 so overflow
    // copies 0 (writing reload never changes the live counter directly).
    timers.write16(0, 0xFFFF);
    timers.write16(2, 0x80); // start, prescaler F/1
    timers.write16(0, 0);
    // Start edge burns one latency cycle before the first increment.
    let mask = timers.tick(2);
    assert_eq!(mask & 1, 1);
    assert_eq!(timers.counter(0), 0);
}

#[test]
fn cascade() {
    let mut timers = Timers::new();
    // Timer 0: F/1, started at 0xFFFF — latency + one cycle overflows.
    timers.write16(0, 0xFFFF);
    timers.write16(2, 0x80);
    // Timer 1: count-up + start, reload 0; large prescaler must be ignored.
    timers.write16(4, 0);
    timers.write16(6, 0x80 | (1 << 2) | 0b11);
    let mask = timers.tick(2);
    assert_eq!(mask & 1, 1);
    assert_eq!(timers.counter(1), 1);
    assert!(timers.cascade(1));
}
