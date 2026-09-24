use super::*;
use graycart::{Bus, Cpu, capture};

fn dummy_state(heap: usize) -> RewindFrame {
    let mut state = capture(&Cpu::new(), &Bus::from_rom(vec![0u8; 0x8000]));
    state.cart.ram = vec![0; heap];
    RewindFrame::Sm83(state)
}

#[test]
fn rewind_ring_evicts_oldest_when_over_soft_cap() {
    let template = dummy_state(64 * 1024);
    let mut ring = RewindRing::new(template.clone());
    let per = template.heap_bytes();
    let n = (REWIND_SOFT_CAP_BYTES / per).max(2) + 2;
    for i in 0..n {
        let RewindFrame::Sm83(mut s) = template.clone() else {
            panic!("expected sm83");
        };
        s.cpu.pc = i as u16;
        ring.push(RewindFrame::Sm83(s));
    }
    assert!(ring.len() <= n);
    assert_ne!(ring.oldest_pc(), 0);
}

#[test]
fn scrub_back_returns_previous_states_in_order() {
    let template = dummy_state(1024);
    let mut ring = RewindRing::new(template.clone());
    for i in 1..=3u16 {
        let RewindFrame::Sm83(mut s) = template.clone() else {
            panic!("expected sm83");
        };
        s.cpu.pc = i;
        ring.push(RewindFrame::Sm83(s));
    }
    ring.begin_scrub();
    assert_eq!(
        match ring.scrub_back().unwrap() {
            RewindFrame::Sm83(s) => s.cpu.pc,
            _ => panic!(),
        },
        2
    );
    assert_eq!(
        match ring.scrub_back().unwrap() {
            RewindFrame::Sm83(s) => s.cpu.pc,
            _ => panic!(),
        },
        1
    );
}

#[test]
fn end_scrub_truncates_ring_to_scrubbed_position() {
    let template = dummy_state(1024);
    let mut ring = RewindRing::new(template.clone());
    for i in 1..=3u16 {
        let RewindFrame::Sm83(mut s) = template.clone() else {
            panic!("expected sm83");
        };
        s.cpu.pc = i;
        ring.push(RewindFrame::Sm83(s));
    }
    ring.begin_scrub();
    let _ = ring.scrub_back();
    let _ = ring.scrub_back();
    ring.end_scrub();
    assert_eq!(ring.len(), 1);
    assert_eq!(ring.newest_pc(), 1);
    ring.begin_scrub();
    assert_eq!(ring.newest_pc(), 1);
    assert!(ring.scrub_back().is_none());
}

#[test]
fn enabled_follows_settings_flag() {
    let mut settings = FrontendSettings::default();
    assert!(!enabled(&settings));
    settings.rewind_enabled = true;
    assert!(enabled(&settings));
}
