use super::{bg_origin, obj_origin};

#[test]
fn mosaic_zero_is_identity() {
    assert_eq!(bg_origin(5, 7, 0), (5, 7));
    assert_eq!(obj_origin(5, 7, 0), (5, 7));
}

#[test]
fn bg_horizontal_two_pixel_blocks() {
    // BG horizontal field 1 (2-pixel blocks), vertical 0.
    let mosaic = 0x0001u16;
    assert_eq!(bg_origin(0, 3, mosaic), (0, 3));
    assert_eq!(bg_origin(1, 3, mosaic), (0, 3));
    assert_eq!(bg_origin(2, 3, mosaic), (2, 3));
}

#[test]
fn bg_and_obj_fields_are_independent() {
    // OBJ fields in the high byte do not move bg_origin.
    let obj_only = 0xF3_00u16; // OBJ H=3, V=15
    assert_eq!(bg_origin(5, 7, obj_only), (5, 7));

    // BG fields in the low byte do not move obj_origin.
    let bg_only = 0x00_F3u16; // BG H=3, V=15
    assert_eq!(obj_origin(5, 7, bg_only), (5, 7));
}

#[test]
fn obj_horizontal_four_pixel_blocks() {
    // OBJ horizontal field 3 (4-pixel blocks).
    let mosaic = 0x0300u16;
    assert_eq!(obj_origin(5, 0, mosaic), (4, 0));
}
