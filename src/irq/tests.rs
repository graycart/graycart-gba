use super::*;

#[test]
fn raise_sets_if() {
    let mut irq = Irq::new();
    assert_eq!(irq.iff(), 0);
    irq.raise(1 << 3);
    assert_eq!(irq.iff(), 1 << 3);
    irq.raise(1 << 0);
    assert_eq!(irq.iff(), (1 << 3) | (1 << 0));
}

#[test]
fn write_if_clears_only_one_bits() {
    let mut irq = Irq::new();
    irq.raise(0x00FF);
    irq.write16(2, 0x00F0);
    assert_eq!(irq.iff(), 0x000F);
    irq.write16(2, 0x0000);
    assert_eq!(irq.iff(), 0x000F);
}

#[test]
fn pending_requires_ime_and_ie_mask() {
    let mut irq = Irq::new();
    irq.raise(1 << 2);
    assert!(!irq.pending());

    irq.write16(0, 1 << 2);
    assert!(!irq.pending());

    irq.write16(8, 1);
    assert!(irq.pending());

    irq.write16(0, 0);
    assert!(!irq.pending());

    irq.write16(0, 1 << 2);
    irq.write16(8, 0);
    assert!(!irq.pending());
}

#[test]
fn can_wake_false_when_ime_off_or_ie_zero() {
    let mut irq = Irq::new();
    assert!(!irq.can_wake());

    irq.write16(8, 1);
    assert!(!irq.can_wake());

    irq.write16(0, 1 << 0);
    assert!(irq.can_wake());

    irq.write16(8, 0);
    assert!(!irq.can_wake());

    irq.write16(8, 1);
    irq.write16(0, 0);
    assert!(!irq.can_wake());
}
