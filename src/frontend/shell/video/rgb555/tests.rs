use super::{rgb555_framebuffer_to_rgba, rgb555_le_bytes_to_u16, rgb555_to_rgba};
use crate::frontend::shell::video::palette::{PalettePreset, framebuffer_to_rgba};
use crate::frontend::shell::video::{DisplayMode, present_framebuffer_rgba};
use crate::frontend::video::cgb_channel;
use graycart::{Framebuffer, SCREEN_HEIGHT, SCREEN_WIDTH, Shade};

fn expand5(c: u8) -> u8 {
    (c << 3) | (c >> 2)
}

#[test]
fn rgb555_black_is_zero() {
    assert_eq!(rgb555_to_rgba(0), [0, 0, 0, 255]);
}

#[test]
fn rgb555_max_is_white() {
    assert_eq!(rgb555_to_rgba(0x7FFF), [255, 255, 255, 255]);
}

#[test]
fn rgb555_mid_channel_values() {
    // R=10, G=20, B=5 → packed RGB555, then GBA CGB brightness curve before expand.
    let packed = 10 | (20 << 5) | (5 << 10);
    assert_eq!(
        rgb555_to_rgba(packed),
        [
            expand5(cgb_channel(10)),
            expand5(cgb_channel(20)),
            expand5(cgb_channel(5)),
            255
        ]
    );
    assert_eq!(rgb555_to_rgba(0x001F), [255, 0, 0, 255]);
    assert_eq!(rgb555_to_rgba(0x03E0), [0, 255, 0, 255]);
    assert_eq!(rgb555_to_rgba(0x7C00), [0, 0, 255, 255]);
    // Channel 16 → curve 10 → expand 82, not linear 132.
    assert_eq!(rgb555_to_rgba(0x0010), [82, 0, 0, 255]);
}

#[test]
fn rgb555_bit_15_is_ignored() {
    assert_eq!(rgb555_to_rgba(0xFFFF), rgb555_to_rgba(0x7FFF));
    assert_eq!(rgb555_to_rgba(0x8000), rgb555_to_rgba(0x0000));
}

#[test]
fn rgb555_le_bytes_low_then_high() {
    // 0x168A: R=10, G=20, B=5 stored little-endian
    assert_eq!(rgb555_le_bytes_to_u16(0x8A, 0x16), 0x168A);
    assert_eq!(
        rgb555_to_rgba(rgb555_le_bytes_to_u16(0x8A, 0x16)),
        rgb555_to_rgba(0x168A)
    );
}

#[test]
fn rgb555_framebuffer_fills_rgba() {
    let mut pixels = vec![0u16; SCREEN_WIDTH * SCREEN_HEIGHT];
    pixels[0] = 0x7FFF;
    pixels[1] = 0x001F;
    let mut out = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
    rgb555_framebuffer_to_rgba(&pixels, &mut out);
    assert_eq!(&out[0..4], &[255, 255, 255, 255]);
    assert_eq!(&out[4..8], &[255, 0, 0, 255]);
    assert_eq!(&out[8..12], &[0, 0, 0, 255]);
}

#[test]
fn dmg_framebuffer_to_rgba_is_unchanged() {
    let pal = PalettePreset::ClassicDmg.palette();
    let fb = Framebuffer::new();
    let mut buf = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
    framebuffer_to_rgba(&fb, pal, &mut buf);
    assert_eq!(&buf[0..4], &pal.shade_to_rgba(Shade::Lightest));
}

#[test]
fn present_native_cgb_ignores_host_dmg_palette() {
    let pal = PalettePreset::ClassicDmg.palette();
    let mut fb = Framebuffer::new();
    fb.set_presents_cgb_color(true);
    fb.set_cgb_pixel(0, 0, Shade::Lightest, 0x7FFF);
    fb.set_cgb_pixel(1, 0, Shade::Darkest, 0x001F);
    let mut buf = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
    let mut previous = None;
    present_framebuffer_rgba(&fb, pal, DisplayMode::Sharp, &mut buf, &mut previous);
    assert_eq!(&buf[0..4], &[255, 255, 255, 255]);
    assert_eq!(&buf[4..8], &[255, 0, 0, 255]);
    assert_ne!(&buf[0..4], &pal.shade_to_rgba(Shade::Lightest));
}

#[test]
fn present_dmg_still_uses_host_palette() {
    let pal = PalettePreset::ClassicDmg.palette();
    let fb = Framebuffer::new();
    let mut buf = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
    let mut previous = None;
    present_framebuffer_rgba(&fb, pal, DisplayMode::Sharp, &mut buf, &mut previous);
    assert_eq!(&buf[0..4], &pal.shade_to_rgba(Shade::Lightest));
}

#[test]
fn present_applies_effects_after_rgb555() {
    let pal = PalettePreset::ClassicDmg.palette();
    let mut fb = Framebuffer::new();
    fb.set_presents_cgb_color(true);
    fb.set_cgb_pixel(1, 0, Shade::Lightest, 0x7FFF);
    let mut sharp = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
    let mut grid = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
    let mut previous = None;
    present_framebuffer_rgba(&fb, pal, DisplayMode::Sharp, &mut sharp, &mut previous);
    previous = None;
    present_framebuffer_rgba(&fb, pal, DisplayMode::PixelGrid, &mut grid, &mut previous);
    assert_eq!(&sharp[4..8], &[255, 255, 255, 255]);
    assert_eq!(&grid[4..8], &[191, 191, 191, 255]);
}

/// Same clone + `present_framebuffer_rgba` path as the winit thread (no GPU).
#[test]
fn native_cgb_cart_clone_presents_rgb555_not_host_palette() {
    use graycart::{
        Cartridge, ExecSession, HostHardwarePref, RunOutcome, apply_fast, bus_from_cartridge,
    };
    use std::path::Path;

    let path = [
        "carts/wario-land-3.gbc",
        "carts/legend-of-zelda_links-awakening_dx.gbc",
        "carts/pokemon_crystal-version.gbc",
    ]
    .iter()
    .map(Path::new)
    .find(|p| p.exists());
    let Some(path) = path else {
        eprintln!("SKIP native CGB present e2e (no carts/*.gbc)");
        return;
    };

    let cart = Cartridge::load(path).expect("load CGB cart");
    let mut cpu = graycart::Cpu::new();
    let mut bus = bus_from_cartridge(cart, HostHardwarePref::GameBoyColor).expect("force GBC");
    apply_fast(&mut cpu, &mut bus);
    match ExecSession::new().run_frames(&mut cpu, &mut bus, 60) {
        RunOutcome::FrameLimit { .. } => {}
        RunOutcome::Fault(report) => panic!("{report}"),
    }

    let fb = bus.ppu.framebuffer.clone();
    assert!(
        fb.presents_cgb_color(),
        "{} NativeCgb must present RGB555",
        path.display()
    );

    let pal = PalettePreset::ClassicDmg.palette();
    let mut presented = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
    let mut previous = None;
    present_framebuffer_rgba(&fb, pal, DisplayMode::Sharp, &mut presented, &mut previous);

    let mut from555 = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
    rgb555_framebuffer_to_rgba(fb.rgb555_pixels(), &mut from555);
    assert_eq!(
        presented, from555,
        "window present path must be RGB555→RGBA, not Shade"
    );

    let mut shade = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
    framebuffer_to_rgba(&fb, pal, &mut shade);
    assert_ne!(
        presented, shade,
        "Classic DMG palette must not be applied on NativeCgb"
    );
}
