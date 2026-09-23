//! Unit tests for normal OBJ sprites.

use super::{obj_window_hit, sprite_count, sprite_pixel};

fn write_u16_le(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset] = value as u8;
    buf[offset + 1] = (value >> 8) as u8;
}

/// One 8×8 4bpp sprite at (0,0), tile 0, priority 0, bank 0; plus a disabled entry.
#[test]
fn one_sprite_pixel_and_count() {
    let mut oam = [0u8; 128 * 8];
    // ATTR0 bit 9 = disabled — hide every slot first.
    for i in 0..128 {
        write_u16_le(&mut oam, i * 8, 0x0200);
    }

    // Entry 0: y=0, normal mode, shape square; x=0, size 0; tile 0, pri 0, bank 0.
    write_u16_le(&mut oam, 0, 0x0000); // attr0
    write_u16_le(&mut oam, 2, 0x0000); // attr1
    write_u16_le(&mut oam, 4, 0x0000); // attr2

    // Entry 1 stays disabled (bit 9) and must not count or draw.

    let mut pal = [0u8; 0x400];
    // Color index 1 in bank 0 → pal[0x200 + 2] = 0x7C00.
    write_u16_le(&mut pal, 0x200 + 2, 0x7C00);

    let mut vram = vec![0u8; 0x18000];
    // First pixel of tile 0 at OBJ base 0x10000: low nibble = index 1.
    vram[0x10000] = 0x01;

    assert_eq!(sprite_count(&oam), 1);

    let px = sprite_pixel(0, 0, &oam, &pal, &vram, true, false, 0);
    let px = px.expect("sprite pixel at (0,0)");
    assert_eq!(px.color, 0x7C00);
    assert_eq!(px.priority, 0);
    assert!(!px.semi);

    assert!(sprite_pixel(1, 0, &oam, &pal, &vram, true, false, 0).is_none());
}

/// Mode bits 10–11 == 2 (OBJ window): opaque hit, transparent neighbor; not drawn.
#[test]
fn obj_window_hit_opaque_and_transparent() {
    let mut oam = [0u8; 128 * 8];
    for i in 0..128 {
        write_u16_le(&mut oam, i * 8, 0x0200);
    }

    // Entry 0: y=0, mode 2 (OBJ window = 0x0800), shape square; x=0, size 0; tile 0.
    write_u16_le(&mut oam, 0, 0x0800);
    write_u16_le(&mut oam, 2, 0x0000);
    write_u16_le(&mut oam, 4, 0x0000);

    let pal = [0u8; 0x400];
    let mut vram = vec![0u8; 0x18000];
    vram[0x10000] = 0x01; // first pixel index 1; neighbor nibble 0

    assert!(obj_window_hit(0, 0, &oam, &vram, true, false));
    assert!(!obj_window_hit(1, 0, &oam, &vram, true, false));
    assert!(sprite_pixel(0, 0, &oam, &pal, &vram, true, false, 0).is_none());
    // Disabled (0x0200) is not an OBJ window.
    assert_eq!(sprite_count(&oam), 0);
}

/// Semi-transparent mode is bits 10–11 == 1 (`0x0400`), not bits 8–9.
#[test]
fn semi_transparent_sets_semi_flag() {
    let mut oam = [0u8; 128 * 8];
    for i in 0..128 {
        write_u16_le(&mut oam, i * 8, 0x0200);
    }
    write_u16_le(&mut oam, 0, 0x0400); // semi-transparent
    write_u16_le(&mut oam, 2, 0x0000);
    write_u16_le(&mut oam, 4, 0x0000);

    let mut pal = [0u8; 0x400];
    write_u16_le(&mut pal, 0x200 + 2, 0x7C00);
    let mut vram = vec![0u8; 0x18000];
    vram[0x10000] = 0x01;

    let px = sprite_pixel(0, 0, &oam, &pal, &vram, true, false, 0).expect("semi sprite");
    assert!(px.semi);
    assert_eq!(sprite_count(&oam), 1);
}
