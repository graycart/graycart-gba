use super::text_bg_pixel;

/// One 8×8 4bpp tile, screen/char block 0, no scroll.
/// Map slot 0 selects the tile; first pixel index 1 → 0x001F; (1,0) transparent.
///
/// Screen base 0 and char base 0 overlap in VRAM. Map slot 0 and tile 0 share
/// the first halfword, so the fixture stores tile number 1 in that slot and
/// places the 4bpp row at tile 1 (offset 32), matching the sampled tile.
fn fixture_4bpp_one_opaque_pixel() -> (u16, Vec<u8>, [u8; 0x400]) {
    let bgcnt = 0u16; // priority 0, char 0, 4bpp, screen 0, 256×256
    let mut vram = vec![0u8; 0x10000];
    let mut pal = [0u8; 0x400];

    // Map slot 0 → tile 1 (low byte is also the start of tile 0's data).
    vram[0] = 0x01;
    vram[1] = 0x00;
    // Tile 1: first pixel (low nibble) = index 1, rest of row transparent.
    vram[32] = 0x01;

    // Palette bank 0, color 1 = 0x001F.
    pal[2] = 0x1F;
    pal[3] = 0x00;

    (bgcnt, vram, pal)
}

#[test]
fn text_bg_pixel_4bpp_top_left_opaque_neighbor_transparent() {
    let (bgcnt, vram, pal) = fixture_4bpp_one_opaque_pixel();

    let p = text_bg_pixel(0, 0, bgcnt, 0, 0, &pal, &vram).expect("opaque");
    assert_eq!(p.color, 0x001F);
    assert_eq!(p.priority, 0);

    assert!(text_bg_pixel(1, 0, bgcnt, 0, 0, &pal, &vram).is_none());
}

#[test]
fn text_bg_pixel_hofs_1_samples_transparent_neighbor() {
    let (bgcnt, vram, pal) = fixture_4bpp_one_opaque_pixel();

    // Screen x=0 with hofs=1 samples tile x=1 → transparent.
    assert!(text_bg_pixel(0, 0, bgcnt, 1, 0, &pal, &vram).is_none());
}
