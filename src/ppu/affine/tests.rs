use super::affine_bg_pixel;

/// Identity matrix, size 128×128, char/screen base 0, wrap off, priority 0.
fn identity_bgcnt() -> u16 {
    0
}

fn setup_fixture() -> (Vec<u8>, Vec<u8>) {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = vec![0u8; 0x400];

    // Map byte 0 = tile 1.
    vram[0] = 1;
    // Tile 1 at offset 64; pixel (0,0) = palette index 2.
    vram[64] = 2;
    // Palette color 2 = 0x03E0 (green).
    pal[4] = 0xE0;
    pal[5] = 0x03;

    (pal, vram)
}

#[test]
fn identity_samples_tile_pixel() {
    let (pal, vram) = setup_fixture();
    let p = affine_bg_pixel(
        0,
        0,
        identity_bgcnt(),
        0x0100,
        0,
        0,
        0x0100,
        0,
        0,
        &pal,
        &vram,
    )
    .expect("opaque pixel");
    assert_eq!(p.color, 0x03E0);
    assert_eq!(p.priority, 0);
}

#[test]
fn outside_map_is_none_when_wrap_off() {
    let (pal, vram) = setup_fixture();
    // Texel (128, 0) is outside a 128×128 map.
    let p = affine_bg_pixel(
        128,
        0,
        identity_bgcnt(),
        0x0100,
        0,
        0,
        0x0100,
        0,
        0,
        &pal,
        &vram,
    );
    assert!(p.is_none());
}

#[test]
fn outside_map_wraps_when_bit13_set() {
    let (pal, vram) = setup_fixture();
    let bgcnt = identity_bgcnt() | (1 << 13);
    // (128, 0) wraps to (0, 0) on a 128×128 map.
    let p = affine_bg_pixel(128, 0, bgcnt, 0x0100, 0, 0, 0x0100, 0, 0, &pal, &vram)
        .expect("wrapped pixel");
    assert_eq!(p.color, 0x03E0);
    assert_eq!(p.priority, 0);
}

#[test]
fn index_zero_is_transparent() {
    let mut vram = vec![0u8; 0x18000];
    let pal = vec![0u8; 0x400];
    // Map -> tile 1, but tile pixel is index 0.
    vram[0] = 1;
    vram[64] = 0;
    let p = affine_bg_pixel(0, 0, 0, 0x0100, 0, 0, 0x0100, 0, 0, &pal, &vram);
    assert!(p.is_none());
}
