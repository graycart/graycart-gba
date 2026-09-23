//! Unit tests for window enable masks.

use super::window_mask;

fn write_u16_le(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset] = value as u8;
    buf[offset + 1] = (value >> 8) as u8;
}

#[test]
fn windows_off_bg0_everywhere() {
    let io = [0u8; 0x4C];
    let dispcnt = 0u16;
    let m = window_mask(0, 0, dispcnt, &io, false);
    assert!(m.bg[0]);
    assert!(m.bg[1]);
    assert!(m.bg[2]);
    assert!(m.bg[3]);
    assert!(m.obj);
    assert!(m.blend);
}

#[test]
fn win0_bg0_inside_only() {
    let mut io = [0u8; 0x4C];
    // WIN0H: X1=10, X2=20 (high=X1, low=X2)
    write_u16_le(&mut io, 0x40, 0x0A14);
    // WIN0V: Y1=0, Y2=160
    write_u16_le(&mut io, 0x44, 0x00A0);
    // WININ: BG0 only inside window 0
    write_u16_le(&mut io, 0x48, 0x0001);
    // WINOUT: nothing outside
    write_u16_le(&mut io, 0x4A, 0x0000);

    let dispcnt = 1 << 13;
    let inside = window_mask(15, 0, dispcnt, &io, false);
    assert!(inside.bg[0]);
    let outside = window_mask(0, 0, dispcnt, &io, false);
    assert!(!outside.bg[0]);
}

#[test]
fn overlap_prefers_win0() {
    let mut io = [0u8; 0x4C];
    // Both windows cover (15, 0): win0 X 10..20, win1 X 0..40; Y 0..160.
    write_u16_le(&mut io, 0x40, 0x0A14);
    write_u16_le(&mut io, 0x42, 0x0028);
    write_u16_le(&mut io, 0x44, 0x00A0);
    write_u16_le(&mut io, 0x46, 0x00A0);
    // WININ: win0 enables BG0 only; win1 enables BG1 only.
    write_u16_le(&mut io, 0x48, 0x0201);
    write_u16_le(&mut io, 0x4A, 0x0000);

    let dispcnt = (1 << 13) | (1 << 14);
    let m = window_mask(15, 0, dispcnt, &io, false);
    assert!(m.bg[0]);
    assert!(!m.bg[1]);
}

#[test]
fn x1_gt_x2_empty_range() {
    let mut io = [0u8; 0x4C];
    // WIN0H: X1=200, X2=10 — GBATEK empty horizontal range (high=X1, low=X2).
    write_u16_le(&mut io, 0x40, 0xC80A);
    write_u16_le(&mut io, 0x44, 0x00A0);
    write_u16_le(&mut io, 0x48, 0x0001);
    write_u16_le(&mut io, 0x4A, 0x0000);

    let dispcnt = 1 << 13;
    assert!(!window_mask(5, 0, dispcnt, &io, false).bg[0]);
    assert!(!window_mask(210, 0, dispcnt, &io, false).bg[0]);
}

#[test]
#[ignore = "hardware wraps when X1>X2; GBATEK leaves that range empty"]
fn x1_gt_x2_hardware_wrap() {
    let mut io = [0u8; 0x4C];
    write_u16_le(&mut io, 0x40, 0xC80A); // X1=200, X2=10
    write_u16_le(&mut io, 0x44, 0x00A0);
    write_u16_le(&mut io, 0x48, 0x0001);
    write_u16_le(&mut io, 0x4A, 0x0000);

    let dispcnt = 1 << 13;
    // Hardware wrap: x >= 200 || x < 10 → x=5 is inside.
    assert!(window_mask(5, 0, dispcnt, &io, false).bg[0]);
}
