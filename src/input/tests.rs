use super::Keypad;

#[test]
fn keyinput_reset_is_all_released() {
    let pad = Keypad::new();
    assert_eq!(pad.read_input(), 0x03FF);
}

#[test]
fn set_pressed_clears_selected_low_bits() {
    let mut pad = Keypad::new();
    pad.set_pressed(1);
    assert_eq!(pad.read_input() & 1, 0);
    assert_eq!(pad.read_input() & 0x03FE, 0x03FE);
}

#[test]
fn irq_never_asserts_when_bit14_off() {
    let mut pad = Keypad::new();
    pad.write_cnt(0x0001); // select A, IRQ off
    pad.set_pressed(1);
    assert!(!pad.irq_asserted());
}

#[test]
fn irq_or_asserts_when_selected_key_pressed() {
    let mut pad = Keypad::new();
    pad.write_cnt((1 << 14) | 0x0001); // IRQ enable + select A, OR
    pad.set_pressed(1);
    assert!(pad.irq_asserted());
    pad.set_pressed(0);
    assert!(!pad.irq_asserted());
}

#[test]
fn irq_and_requires_all_selected_keys() {
    let mut pad = Keypad::new();
    pad.write_cnt((1 << 15) | (1 << 14) | 0x0003); // AND + IRQ + A|B
    pad.set_pressed(1); // only A
    assert!(!pad.irq_asserted());
    pad.set_pressed(0x0003); // A and B
    assert!(pad.irq_asserted());
}
