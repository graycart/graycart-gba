use super::{blend, first_target, second_target};

fn bldcnt_effect(effect: u16) -> u16 {
    effect << 6
}

#[test]
fn first_and_second_target_bits() {
    let bldcnt = 0b0011_1111_0000_0001; // BG0 1st, all 2nd
    assert!(first_target(bldcnt, 0));
    assert!(!first_target(bldcnt, 1));
    assert!(second_target(bldcnt, 0));
    assert!(second_target(bldcnt, 5));
    assert!(!second_target(bldcnt, 6));
}

#[test]
fn alpha_eva16_evb0_returns_top() {
    let bldcnt = bldcnt_effect(1);
    let bldalpha = 16;
    assert_eq!(
        blend(0x001F, Some(0x03E0), bldcnt, bldalpha, 0, false),
        0x001F
    );
}

#[test]
fn alpha_eva0_evb16_returns_bottom() {
    let bldcnt = bldcnt_effect(1);
    let bldalpha = 16 << 8;
    assert_eq!(
        blend(0x001F, Some(0x03E0), bldcnt, bldalpha, 0, false),
        0x03E0
    );
}

#[test]
fn alpha_eva8_evb8_half_red() {
    let bldcnt = bldcnt_effect(1);
    let bldalpha = 8 | (8 << 8);
    // (31 * 8 + 0 * 8) / 16 = 15
    assert_eq!(
        blend(0x001F, Some(0x0000), bldcnt, bldalpha, 0, false),
        0x000F
    );
}

#[test]
fn brighten_evy16_black_to_white() {
    let bldcnt = bldcnt_effect(2);
    assert_eq!(blend(0x0000, None, bldcnt, 0, 16, false), 0x7FFF);
}

#[test]
fn darken_evy16_white_to_black() {
    let bldcnt = bldcnt_effect(3);
    assert_eq!(blend(0x7FFF, None, bldcnt, 0, 16, false), 0);
}

#[test]
fn effect_off_returns_top() {
    let bldcnt = bldcnt_effect(0);
    assert_eq!(
        blend(0x001F, Some(0x03E0), bldcnt, 8 | (8 << 8), 16, false),
        0x001F
    );
}

#[test]
fn eva_field_31_clamped_to_16() {
    let bldcnt = bldcnt_effect(1);
    let with_31 = blend(0x001F, Some(0x0000), bldcnt, 31, 0, false);
    let with_16 = blend(0x001F, Some(0x0000), bldcnt, 16, 0, false);
    assert_eq!(with_31, with_16);
    assert_eq!(with_31, 0x001F);
}

#[test]
fn force_alpha_with_effect_off_still_blends() {
    let bldcnt = bldcnt_effect(0);
    let bldalpha = 8 | (8 << 8);
    assert_eq!(
        blend(0x001F, Some(0x0000), bldcnt, bldalpha, 0, true),
        0x000F
    );
}

#[test]
fn force_alpha_with_no_bottom_returns_top() {
    let bldcnt = bldcnt_effect(0);
    assert_eq!(blend(0x001F, None, bldcnt, 8 | (8 << 8), 0, true), 0x001F);
}

#[test]
fn alpha_with_no_bottom_returns_top() {
    let bldcnt = bldcnt_effect(1);
    assert_eq!(blend(0x001F, None, bldcnt, 8 | (8 << 8), 0, false), 0x001F);
}
