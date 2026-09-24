use super::{HEIGHT, Ppu, WIDTH};

fn pix(ppu: &Ppu, x: usize, y: usize) -> u16 {
    ppu.pixels[x + y * WIDTH]
}

fn empty_oam() -> [u8; 0x400] {
    [0u8; 0x400]
}

fn zero_io() -> [u8; 0x400] {
    [0u8; 0x400]
}

fn write_u16_le(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset] = value as u8;
    buf[offset + 1] = (value >> 8) as u8;
}

/// Hide all 128 OAM entries (attr0 bits 8–9 = 2).
fn hide_all_oam(oam: &mut [u8; 0x400]) {
    for i in 0..128 {
        write_u16_le(oam, i * 8, 0x0200);
    }
}

/// One 8×8 4bpp sprite at (0,0), tile 0, given priority, bank 0.
fn place_sprite(oam: &mut [u8; 0x400], priority: u8) {
    hide_all_oam(oam);
    write_u16_le(oam, 0, 0x0000); // attr0: y=0, normal
    write_u16_le(oam, 2, 0x0000); // attr1: x=0, size 0
    write_u16_le(oam, 4, u16::from(priority) << 10); // attr2: tile 0, pri
}

/// OBJ tile 0 at `obj_base`: first pixel index 1. OBJ palette color 1 = `color`.
fn sprite_tile_and_pal(vram: &mut [u8], pal: &mut [u8], obj_base: usize, color: u16) {
    vram[obj_base] = 0x01;
    write_u16_le(pal, 0x200 + 2, color);
}

/// Text BG0: map slot 0 → tile 1, first pixel opaque `color`, priority from bgcnt.
fn text_bg0_one_pixel(vram: &mut [u8], pal: &mut [u8], io: &mut [u8], bgcnt: u16, color: u16) {
    write_u16_le(io, 0x08, bgcnt); // BG0CNT
    // Map slot 0 → tile 1 (avoids overlap with tile 0 at char base 0).
    vram[0] = 0x01;
    vram[1] = 0x00;
    vram[32] = 0x01; // tile 1 first pixel index 1
    write_u16_le(pal, 2, color); // BG palette index 1
}

#[test]
fn mode3_copies_red_pixel() {
    let mut vram = vec![0u8; 0x18000];
    let pal = [0u8; 0x400];
    // Red BGR555 0x001F at (1, 2).
    let off = (1 + 2 * WIDTH) * 2;
    vram[off] = 0x1F;
    vram[off + 1] = 0x00;

    let mut ppu = Ppu::new();
    // Mode 3, BG2 on.
    ppu.render(0x0403, &pal, &vram, &empty_oam(), &zero_io());
    assert_eq!(pix(&ppu, 1, 2), 0x001F);
    assert_eq!(pix(&ppu, 0, 2), 0); // neighbor stays backdrop
    assert_eq!(pix(&ppu, 1, 1), 0);
}

#[test]
fn mode4_bg2_off_stays_backdrop() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    pal[0] = 0x1F; // backdrop = 0x001F
    vram[0] = 1; // would map to a different color if drawn

    let mut ppu = Ppu::new();
    // Mode 4, BG2 clear.
    ppu.render(0x0004, &pal, &vram, &empty_oam(), &zero_io());
    assert_eq!(pix(&ppu, 0, 0), 0x001F);
    assert_eq!(ppu.nonzero(), (WIDTH * HEIGHT) as u32);
}

#[test]
fn mode4_draws_palette_index_and_frame() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    // index 1 -> palette halfword at byte 2: 0x03E0
    pal[2] = 0xE0;
    pal[3] = 0x03;
    vram[0] = 1;

    let mut ppu = Ppu::new();
    // Mode 4, BG2 on, frame 0.
    ppu.render(0x0404, &pal, &vram, &empty_oam(), &zero_io());
    assert_eq!(pix(&ppu, 0, 0), 0x03E0);

    // Second frame at 0xA000.
    vram[0] = 0;
    vram[0xA000] = 1;
    ppu.render(0x0414, &pal, &vram, &empty_oam(), &zero_io()); // bit 4 = frame 1
    assert_eq!(pix(&ppu, 0, 0), 0x03E0);
}

#[test]
fn mode5_inside_drawn_outside_backdrop() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    pal[0] = 0xE0;
    pal[1] = 0x03; // backdrop 0x03E0
    // Pixel (0, 0) in the 160x128 bitmap: 0x001F
    vram[0] = 0x1F;
    vram[1] = 0x00;

    let mut ppu = Ppu::new();
    // Mode 5, BG2 on.
    ppu.render(0x0405, &pal, &vram, &empty_oam(), &zero_io());
    assert_eq!(pix(&ppu, 0, 0), 0x001F);
    assert_eq!(pix(&ppu, 200, 0), 0x03E0); // outside 160x128
}

#[test]
fn mode0_with_bg0_empty_vram_stays_backdrop() {
    let vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    pal[0] = 0x1F;
    pal[1] = 0x00; // backdrop 0x001F

    let mut ppu = Ppu::new();
    // Mode 0, BG0 on (and BG2 on) — empty VRAM tiles are transparent.
    ppu.render(0x0500, &pal, &vram, &empty_oam(), &zero_io());
    assert_eq!(pix(&ppu, 0, 0), 0x001F);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            assert_eq!(pix(&ppu, x, y), 0x001F);
        }
    }
}

#[test]
fn forced_blank_is_white() {
    let vram = [0u8; 0];
    let pal = [0u8; 0];
    let mut ppu = Ppu::new();
    ppu.render(0x0080, &pal, &vram, &empty_oam(), &zero_io());
    for p in ppu.pixels.iter() {
        assert_eq!(*p, 0x7FFF);
    }
}

#[test]
fn equal_priority_bg_beats_sprite() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    let mut io = zero_io();
    let mut oam = empty_oam();

    // BG0 priority 0, color 0x001F at (0,0).
    text_bg0_one_pixel(&mut vram, &mut pal, &mut io, 0, 0x001F);
    // Sprite priority 0, color 0x7C00 at (0,0). Tile mode OBJ base 0x10000.
    place_sprite(&mut oam, 0);
    sprite_tile_and_pal(&mut vram, &mut pal, 0x10000, 0x7C00);

    let mut ppu = Ppu::new();
    // Mode 0, BG0 on, OBJ on, 1D mapping.
    ppu.render(0x1100, &pal, &vram, &oam, &io);
    assert_eq!(pix(&ppu, 0, 0), 0x001F, "equal priority: BG wins");
}

#[test]
fn equal_priority_lower_bg_index_wins() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    let mut io = zero_io();

    // Both BGs priority 0. BG0 uses screen/char base 0; BG1 uses screen base 1
    // (0x800) and char base 1 (0x4000) so maps/tiles do not collide.
    write_u16_le(&mut io, 0x08, 0x0000); // BG0CNT: pri 0, screen 0, char 0
    write_u16_le(&mut io, 0x0A, 0x0104); // BG1CNT: pri 0, char 1, screen 1

    // BG0: map→tile 1, color index 1 → 0x001F
    vram[0] = 0x01;
    vram[32] = 0x01;
    write_u16_le(&mut pal, 2, 0x001F);

    // BG1: map at 0x800 → tile 1, tile at 0x4000+32, color index 1 → 0x03E0
    // (same bank index; distinct halfword would need bank/palette — use same
    // palette entry 1 but we need a different color. Use 8bpp? Simpler: put
    // BG1's opaque pixel in palette bank 1 via map entry high nibble.)
    // Map entry: tile 1 | pal bank 1 << 12.
    write_u16_le(&mut vram, 0x800, 0x1001);
    vram[0x4000 + 32] = 0x01;
    write_u16_le(&mut pal, (16 + 1) * 2, 0x03E0);

    let mut ppu = Ppu::new();
    // Mode 0, BG0|BG1 on.
    ppu.render(0x0300, &pal, &vram, &empty_oam(), &io);
    assert_eq!(
        pix(&ppu, 0, 0),
        0x001F,
        "lower BG index wins on equal priority"
    );
}

#[test]
fn bitmap_sprite_equal_priority_leaves_bitmap() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    let io = zero_io();
    let mut oam = empty_oam();

    // Mode 3 bitmap red at (0,0).
    vram[0] = 0x1F;
    vram[1] = 0x00;
    // BG2CNT priority 0 (default). Sprite priority 0 — equal, bitmap wins.
    place_sprite(&mut oam, 0);
    sprite_tile_and_pal(&mut vram, &mut pal, 0x14000, 0x7C00);

    let mut ppu = Ppu::new();
    // Mode 3, BG2 on, OBJ on, 1D.
    ppu.render(0x1443, &pal, &vram, &oam, &io);
    assert_eq!(pix(&ppu, 0, 0), 0x001F);
}

#[test]
fn bitmap_sprite_strictly_better_priority_replaces() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    let mut io = zero_io();
    let mut oam = empty_oam();

    vram[0] = 0x1F;
    vram[1] = 0x00;
    // BG2CNT priority 1; sprite priority 0 → sprite wins.
    write_u16_le(&mut io, 0x0C, 0x0001);
    place_sprite(&mut oam, 0);
    sprite_tile_and_pal(&mut vram, &mut pal, 0x14000, 0x7C00);

    let mut ppu = Ppu::new();
    ppu.render(0x1443, &pal, &vram, &oam, &io);
    assert_eq!(pix(&ppu, 0, 0), 0x7C00);
}

#[test]
fn bitmap_margin_sprite_draws_over_backdrop() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    let io = zero_io();
    let mut oam = empty_oam();

    // Backdrop green.
    write_u16_le(&mut pal, 0, 0x03E0);
    // Mode 5 bitmap red at (0,0) only; margin stays backdrop.
    vram[0] = 0x1F;
    vram[1] = 0x00;

    // Sprite at (200, 0) — outside 160×128.
    hide_all_oam(&mut oam);
    write_u16_le(&mut oam, 0, 0x0000); // y=0
    write_u16_le(&mut oam, 2, 200); // x=200
    write_u16_le(&mut oam, 4, 0); // tile 0, pri 0
    sprite_tile_and_pal(&mut vram, &mut pal, 0x14000, 0x7C00);

    let mut ppu = Ppu::new();
    // Mode 5, BG2 on, OBJ on, 1D. BG2CNT pri 0 — sprite still draws on margin.
    ppu.render(0x1445, &pal, &vram, &oam, &io);
    assert_eq!(pix(&ppu, 0, 0), 0x001F, "inside bitmap stays");
    assert_eq!(pix(&ppu, 200, 0), 0x7C00, "sprite over margin backdrop");
}

#[test]
fn bitmap_bg2_off_sprite_draws_over_backdrop() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    let io = zero_io();
    let mut oam = empty_oam();

    write_u16_le(&mut pal, 0, 0x03E0);
    vram[0] = 0x1F; // would be red if BG2 were on
    vram[1] = 0x00;
    place_sprite(&mut oam, 0);
    sprite_tile_and_pal(&mut vram, &mut pal, 0x14000, 0x7C00);

    let mut ppu = Ppu::new();
    // Mode 3, BG2 off, OBJ on, 1D.
    ppu.render(0x1043, &pal, &vram, &oam, &io);
    assert_eq!(pix(&ppu, 0, 0), 0x7C00);
}

#[test]
fn obj_enable_clear_sprite_does_not_draw() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    let mut io = zero_io();
    let mut oam = empty_oam();

    text_bg0_one_pixel(&mut vram, &mut pal, &mut io, 0, 0x001F);
    place_sprite(&mut oam, 0);
    // Priority 0 sprite that would beat nothing — use priority better than BG
    // would still not draw when bit 12 is clear. Put sprite-only pixel by
    // leaving BG transparent at (1,0) and placing sprite there.
    hide_all_oam(&mut oam);
    write_u16_le(&mut oam, 0, 0x0000);
    write_u16_le(&mut oam, 2, 1); // x=1
    write_u16_le(&mut oam, 4, 0);
    sprite_tile_and_pal(&mut vram, &mut pal, 0x10000, 0x7C00);
    write_u16_le(&mut pal, 0, 0x03E0); // backdrop

    let mut ppu = Ppu::new();
    // Mode 0, BG0 on, OBJ off (bit 12 clear), 1D.
    ppu.render(0x0100, &pal, &vram, &oam, &io);
    assert_eq!(pix(&ppu, 0, 0), 0x001F);
    assert_eq!(
        pix(&ppu, 1, 0),
        0x03E0,
        "sprite must not draw with bit 12 clear"
    );
}

#[test]
fn window_clips_background_outside() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    let mut io = zero_io();

    // Backdrop green; BG0 red at (0,0) and we also put opaque data that would
    // cover (5,0) if the window did not clip.
    write_u16_le(&mut pal, 0, 0x03E0);
    text_bg0_one_pixel(&mut vram, &mut pal, &mut io, 0, 0x001F);
    // Map slot for tile at screen (5,0): screen base 0, tile map entry at
    // map_x=5 → entry offset 5*2. Point at tile 1 (same opaque tile).
    write_u16_le(&mut vram, 5 * 2, 0x0001);

    // WIN0: x in [0,4), y in [0,1). Inside: BG0 on. Outside: BG0 off.
    write_u16_le(&mut io, 0x40, 0x0004); // WIN0H: X1=0, X2=4 (high=X1, low=X2)
    write_u16_le(&mut io, 0x44, 0x0001); // WIN0V: Y1=0, Y2=1
    write_u16_le(&mut io, 0x48, 0x0021); // WININ: BG0 + blend
    write_u16_le(&mut io, 0x4A, 0x0020); // WINOUT: blend only (no BG0)

    let mut ppu = Ppu::new();
    // Mode 0, BG0 on, WIN0 on (bit 13).
    ppu.render(0x2100, &pal, &vram, &empty_oam(), &io);
    assert_eq!(pix(&ppu, 0, 0), 0x001F, "inside window draws BG0");
    assert_eq!(pix(&ppu, 5, 0), 0x03E0, "outside window leaves backdrop");
}

#[test]
fn alpha_blend_two_backgrounds() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    let mut io = zero_io();

    // BG0 priority 0, red 0x001F. BG1 priority 1, blue 0x7C00.
    write_u16_le(&mut io, 0x08, 0x0000); // BG0CNT pri 0
    write_u16_le(&mut io, 0x0A, 0x0105); // BG1CNT pri 1, char base 1, screen base 1
    vram[0] = 0x01;
    vram[32] = 0x01;
    write_u16_le(&mut pal, 2, 0x001F);
    write_u16_le(&mut vram, 0x800, 0x0001);
    vram[0x4000 + 32] = 0x01;
    write_u16_le(&mut pal, 2, 0x001F); // already set
    // BG1 uses palette index 1 as well — give it bank 0 same entry; use a
    // distinct color via palette bank 1 on the map entry.
    write_u16_le(&mut vram, 0x800, 0x1001);
    write_u16_le(&mut pal, (16 + 1) * 2, 0x7C00);

    // BLDCNT: alpha (mode 1), 1st=BG0, 2nd=BG1. BLDALPHA EVA=8 EVB=8.
    write_u16_le(&mut io, 0x50, 0x0201 | (1 << 6)); // bit0 BG0, bit9 BG1, effect=1
    write_u16_le(&mut io, 0x52, 0x0808);

    let mut ppu = Ppu::new();
    // Mode 0, BG0|BG1 on.
    ppu.render(0x0300, &pal, &vram, &empty_oam(), &io);
    // (31*8+0*8)/16 = 15 red, (0*8+31*8)/16 = 15 blue → 0x3C0F
    assert_eq!(pix(&ppu, 0, 0), 0x3C0F);
}

#[test]
fn bg_mosaic_repeats_even_into_odd() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    let mut io = zero_io();

    write_u16_le(&mut pal, 0, 0x03E0); // backdrop
    // BG0CNT: mosaic on (bit 6), pri 0.
    text_bg0_one_pixel(&mut vram, &mut pal, &mut io, 1 << 6, 0x001F);
    // Neighbor map entry (1,0) → tile 2 with a different color so without
    // mosaic (1,0) would not match (0,0).
    write_u16_le(&mut vram, 2, 0x0002);
    vram[64] = 0x01; // tile 2 first pixel index 1
    write_u16_le(&mut pal, 2, 0x001F); // index 1 red (shared)
    // Make tile 2 use palette bank 1 → green, so mosaic failure is obvious.
    write_u16_le(&mut vram, 2, 0x1002);
    write_u16_le(&mut pal, (16 + 1) * 2, 0x03E0);

    // MOSAIC: BG H field = 1 → block width 2 (even origin for odd x).
    write_u16_le(&mut io, 0x4C, 0x0001);

    let mut ppu = Ppu::new();
    ppu.render(0x0100, &pal, &vram, &empty_oam(), &io);
    assert_eq!(pix(&ppu, 0, 0), 0x001F);
    assert_eq!(
        pix(&ppu, 1, 0),
        0x001F,
        "mosaic field 1 must sample even pixel at odd x"
    );
}

#[test]
fn opaque_non_second_target_blocks_blend() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    let mut io = zero_io();

    // Backdrop blue is a 2nd target — must not be reached past BG1.
    write_u16_le(&mut pal, 0, 0x7C00);
    // BG0 pri 0 red (1st target). BG1 pri 1 green (opaque, not 2nd target).
    write_u16_le(&mut io, 0x08, 0x0000);
    write_u16_le(&mut io, 0x0A, 0x0105); // pri 1, char 1, screen 1
    vram[0] = 0x01;
    vram[32] = 0x01;
    write_u16_le(&mut pal, 2, 0x001F);
    write_u16_le(&mut vram, 0x800, 0x1001);
    vram[0x4000 + 32] = 0x01;
    write_u16_le(&mut pal, (16 + 1) * 2, 0x03E0);

    // Alpha: 1st=BG0, 2nd=backdrop only (not BG1). EVA=8 EVB=8.
    write_u16_le(&mut io, 0x50, (1 << 6) | 0x0001 | (1 << 13));
    write_u16_le(&mut io, 0x52, 0x0808);

    let mut ppu = Ppu::new();
    ppu.render(0x0300, &pal, &vram, &empty_oam(), &io);
    assert_eq!(
        pix(&ppu, 0, 0),
        0x001F,
        "opaque non-2nd-target BG1 must block blend with backdrop"
    );
}

#[test]
fn obj_window_hides_bg0_inside_opaque_texels() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    let mut io = zero_io();
    let mut oam = empty_oam();

    write_u16_le(&mut pal, 0, 0x03E0); // backdrop green
    text_bg0_one_pixel(&mut vram, &mut pal, &mut io, 0, 0x001F);
    // Same 8×8 tile covers x=0 and x=1; make fine_x=1 opaque too (high nibble).
    vram[32] = 0x11;

    // OBJ window sprite at (0,0), mode bits 10–11 = 2.
    hide_all_oam(&mut oam);
    write_u16_le(&mut oam, 0, 0x0800);
    write_u16_le(&mut oam, 2, 0x0000);
    write_u16_le(&mut oam, 4, 0x0000);
    vram[0x10000] = 0x01; // opaque at (0,0); (1,0) nibble stays 0

    // Outside OBJ window: BG0 on. Inside OBJ window (WINOUT high): BG0 off.
    write_u16_le(&mut io, 0x4A, 0x2001); // low=BG0, high=no BG0

    let mut ppu = Ppu::new();
    // Mode 0, BG0, OBJ, OBJ window (bits 8, 12, 15).
    ppu.render(0x9100, &pal, &vram, &oam, &io);
    assert_eq!(
        pix(&ppu, 0, 0),
        0x03E0,
        "inside OBJ window opaque texel must hide BG0"
    );
    assert_eq!(
        pix(&ppu, 1, 0),
        0x001F,
        "outside OBJ window must still show BG0"
    );
}

#[test]
fn semi_transparent_sprite_blends_with_effect_off() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    let mut io = zero_io();
    let mut oam = empty_oam();

    // BG0 red as 2nd target. Semi sprite blue on top. BLDCNT effect = 0.
    text_bg0_one_pixel(&mut vram, &mut pal, &mut io, 0, 0x001F);
    hide_all_oam(&mut oam);
    write_u16_le(&mut oam, 0, 0x0400); // semi-transparent
    write_u16_le(&mut oam, 2, 0x0000);
    write_u16_le(&mut oam, 4, 0); // pri 0 — beats BG0 only if strictly better;
    // equal pri: BG wins. Use sprite pri 0, BG pri 1.
    write_u16_le(&mut io, 0x08, 0x0001); // BG0CNT pri 1
    sprite_tile_and_pal(&mut vram, &mut pal, 0x10000, 0x7C00);

    // Effect off, 2nd target BG0. EVA=8 EVB=8. Semi forces alpha.
    write_u16_le(&mut io, 0x50, 1 << 8); // bit 8 = BG0 2nd target, effect 0
    write_u16_le(&mut io, 0x52, 0x0808);

    let mut ppu = Ppu::new();
    // Mode 0, BG0, OBJ, 1D.
    ppu.render(0x1100, &pal, &vram, &oam, &io);
    // Sprite top blue + BG red → 0x3C0F
    assert_eq!(pix(&ppu, 0, 0), 0x3C0F);
}

#[test]
fn window_blend_bit_clear_skips_blend() {
    let mut vram = vec![0u8; 0x18000];
    let mut pal = [0u8; 0x400];
    let mut io = zero_io();

    write_u16_le(&mut pal, 0, 0x7C00); // backdrop blue (2nd target)
    text_bg0_one_pixel(&mut vram, &mut pal, &mut io, 0, 0x001F);

    // WIN0 covers (0,0): BG0 on, blend bit clear.
    write_u16_le(&mut io, 0x40, 0x0004); // X1=0 X2=4
    write_u16_le(&mut io, 0x44, 0x0001); // Y1=0 Y2=1
    write_u16_le(&mut io, 0x48, 0x0001); // WININ: BG0, no blend
    write_u16_le(&mut io, 0x4A, 0x0000);

    // Alpha would blend BG0 with backdrop if mask.blend were true.
    write_u16_le(&mut io, 0x50, (1 << 6) | 0x0001 | (1 << 13));
    write_u16_le(&mut io, 0x52, 0x0808);

    let mut ppu = Ppu::new();
    ppu.render(0x2100, &pal, &vram, &empty_oam(), &io);
    assert_eq!(
        pix(&ppu, 0, 0),
        0x001F,
        "WININ blend bit clear must leave 1st-target unblended"
    );
}
