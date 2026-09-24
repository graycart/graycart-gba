//! Unit tests for normal and affine OBJ sprites.

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

/// Affine + identity matrix: opaque texel at sprite local (0,0) draws at screen origin.
#[test]
fn affine_identity_draws_at_screen_origin() {
    let mut oam = [0u8; 128 * 8];
    for i in 0..128 {
        write_u16_le(&mut oam, i * 8, 0x0200);
    }

    // ATTR0 bit 8 = affine; y=0; square 8×8. ATTR1 x=0, affine index 0.
    write_u16_le(&mut oam, 0, 0x0100);
    write_u16_le(&mut oam, 2, 0x0000);
    write_u16_le(&mut oam, 4, 0x0000);
    // Matrix group 0: PA at +6, PB at +0xE, PC at +0x16, PD at +0x1E.
    write_u16_le(&mut oam, 0x06, 0x0100); // pa
    write_u16_le(&mut oam, 0x0E, 0x0000); // pb
    write_u16_le(&mut oam, 0x16, 0x0000); // pc
    write_u16_le(&mut oam, 0x1E, 0x0100); // pd

    let mut pal = [0u8; 0x400];
    write_u16_le(&mut pal, 0x200 + 2, 0x7C00);
    let mut vram = vec![0u8; 0x18000];
    vram[0x10000] = 0x01; // local (0,0) index 1

    assert_eq!(sprite_count(&oam), 1);

    let px = sprite_pixel(0, 0, &oam, &pal, &vram, true, false, 0)
        .expect("affine identity at screen origin");
    assert_eq!(px.color, 0x7C00);
    assert!(
        sprite_pixel(1, 0, &oam, &pal, &vram, true, false, 0).is_none(),
        "pixel just outside opaque texel stays backdrop"
    );
}

/// Affine index-0 texel is transparent (unwritten), even with identity.
#[test]
fn affine_identity_index0_stays_transparent() {
    let mut oam = [0u8; 128 * 8];
    for i in 0..128 {
        write_u16_le(&mut oam, i * 8, 0x0200);
    }
    write_u16_le(&mut oam, 0, 0x0100);
    write_u16_le(&mut oam, 2, 0x0000);
    write_u16_le(&mut oam, 4, 0x0000);
    write_u16_le(&mut oam, 0x06, 0x0100);
    write_u16_le(&mut oam, 0x0E, 0x0000);
    write_u16_le(&mut oam, 0x16, 0x0000);
    write_u16_le(&mut oam, 0x1E, 0x0100);

    let mut pal = [0u8; 0x400];
    write_u16_le(&mut pal, 0x200 + 2, 0x7C00);
    // Tile all zeros → every texel index 0.
    let vram = vec![0u8; 0x18000];

    assert!(
        sprite_pixel(0, 0, &oam, &pal, &vram, true, false, 0).is_none(),
        "index 0 must not write"
    );
}

/// Double-size + identity: graphic centered in 16×16 box; opaque texel visible inside.
#[test]
fn affine_double_size_identity_centers_graphic() {
    let mut oam = [0u8; 128 * 8];
    for i in 0..128 {
        write_u16_le(&mut oam, i * 8, 0x0200);
    }
    // ATTR0 bits 8–9 = affine + double-size (0x0300).
    write_u16_le(&mut oam, 0, 0x0300);
    write_u16_le(&mut oam, 2, 0x0000);
    write_u16_le(&mut oam, 4, 0x0000);
    write_u16_le(&mut oam, 0x06, 0x0100);
    write_u16_le(&mut oam, 0x0E, 0x0000);
    write_u16_le(&mut oam, 0x16, 0x0000);
    write_u16_le(&mut oam, 0x1E, 0x0100);

    let mut pal = [0u8; 0x400];
    write_u16_le(&mut pal, 0x200 + 2, 0x7C00);
    let mut vram = vec![0u8; 0x18000];
    vram[0x10000] = 0x01; // sprite local (0,0)

    // Clip is 16×16; graphic stays centered — opaque at (4,4), not at old (0,0).
    assert!(
        sprite_pixel(0, 0, &oam, &pal, &vram, true, false, 0).is_none(),
        "double-size corner inside clip but outside centered graphic"
    );
    let px = sprite_pixel(4, 4, &oam, &pal, &vram, true, false, 0)
        .expect("opaque texel centered in double-size box");
    assert_eq!(px.color, 0x7C00);

    // Visible somewhere in the 16×16 window.
    let mut found = false;
    for y in 0..16 {
        for x in 0..16 {
            if sprite_pixel(x, y, &oam, &pal, &vram, true, false, 0).is_some() {
                found = true;
            }
        }
    }
    assert!(found, "opaque texel visible in double-size window");
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
