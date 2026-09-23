//! LCD Video Controller — bitmap, tile BGs, and sprites.
//!
//! Cited: GBATEK LCD Video Controller.
//! <https://problemkaputt.de/gbatek.htm>

mod affine;
mod bg;
mod blend;
mod mosaic;
mod obj;
mod window;

#[cfg(test)]
mod tests;

pub use obj::sprite_count;

pub const WIDTH: usize = 240;
pub const HEIGHT: usize = 160;

/// Picture buffer. Pixels are hardware BGR555 in the low 15 bits.
pub struct Ppu {
    pub pixels: [u16; WIDTH * HEIGHT],
}

impl Ppu {
    /// Create a blank framebuffer (every pixel 0).
    pub fn new() -> Self {
        Self {
            pixels: [0; WIDTH * HEIGHT],
        }
    }

    /// Render one frame from `dispcnt`, palette RAM, VRAM, OAM, and I/O.
    ///
    /// `dispcnt` is the DISPCNT halfword. `pal` is the 1KB palette. `vram` is
    /// the 96KB VRAM. `oam` is the 1KB OAM. `io` is the 1KB I/O block (offsets
    /// from 0x04000000). Empty OAM and a zeroed I/O slice leave bitmap modes
    /// unchanged.
    #[allow(clippy::too_many_arguments)]
    pub fn render(&mut self, dispcnt: u16, pal: &[u8], vram: &[u8], oam: &[u8], io: &[u8]) {
        // Bit 7: Forced Blank — white screen.
        if dispcnt & (1 << 7) != 0 {
            self.pixels.fill(0x7FFF);
            return;
        }

        let backdrop = read_u16_le(pal, 0) & 0x7FFF;
        self.pixels.fill(backdrop);

        let mode = dispcnt & 0b111;
        match mode {
            0..=2 => self.render_tiled(dispcnt, pal, vram, oam, io, mode),
            3..=5 => self.render_bitmap(dispcnt, pal, vram, oam, io, mode),
            _ => {}
        }
    }

    /// Count pixels whose low 15 bits are not zero.
    pub fn nonzero(&self) -> u32 {
        self.pixels.iter().filter(|&&p| p & 0x7FFF != 0).count() as u32
    }

    fn render_tiled(
        &mut self,
        dispcnt: u16,
        pal: &[u8],
        vram: &[u8],
        oam: &[u8],
        io: &[u8],
        mode: u16,
    ) {
        let one_d = dispcnt & (1 << 6) != 0;
        let obj_on = dispcnt & (1 << 12) != 0;
        let objwin = dispcnt & (1 << 15) != 0;
        let bg_enable = [
            dispcnt & (1 << 8) != 0,
            dispcnt & (1 << 9) != 0,
            dispcnt & (1 << 10) != 0,
            dispcnt & (1 << 11) != 0,
        ];
        let mosaic_reg = io_u16(io, 0x4C);
        let bldcnt = io_u16(io, 0x50);
        let bldalpha = io_u16(io, 0x52);
        let bldy = io_u16(io, 0x54);
        let backdrop = self.pixels[0];

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let in_obj_window =
                    objwin && obj_on && obj::obj_window_hit(x, y, oam, vram, one_d, false);
                let mask = window::window_mask(x, y, dispcnt, io, in_obj_window);

                let mut layers = [Composite::empty(); 5];
                let mut n = 0usize;

                for (bg, enabled) in bg_enable.iter().enumerate() {
                    if !enabled || !mask.bg[bg] {
                        continue;
                    }
                    let bgcnt = io_u16(io, 0x08 + bg * 2);
                    let (sx, sy) = if bgcnt & (1 << 6) != 0 {
                        mosaic::bg_origin(x, y, mosaic_reg)
                    } else {
                        (x, y)
                    };
                    let sample = match mode {
                        0 => sample_text_bg(bg, sx, sy, pal, vram, io),
                        1 => match bg {
                            0 | 1 => sample_text_bg(bg, sx, sy, pal, vram, io),
                            2 => sample_affine_bg(2, sx, sy, pal, vram, io),
                            _ => None,
                        },
                        2 => match bg {
                            2 | 3 => sample_affine_bg(bg, sx, sy, pal, vram, io),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(px) = sample {
                        layers[n] = Composite {
                            color: px.color,
                            priority: px.priority,
                            is_sprite: false,
                            bg_index: bg as u8,
                            layer: bg as u8,
                            semi: false,
                        };
                        n += 1;
                    }
                }

                if obj_on && mask.obj {
                    if let Some(sp) =
                        obj::sprite_pixel(x, y, oam, pal, vram, one_d, false, mosaic_reg)
                    {
                        layers[n] = Composite {
                            color: sp.color,
                            priority: sp.priority,
                            is_sprite: true,
                            bg_index: 0xFF,
                            layer: 4,
                            semi: sp.semi,
                        };
                        n += 1;
                    }
                }

                self.pixels[x + y * WIDTH] =
                    finalize_pixel(&layers[..n], backdrop, mask.blend, bldcnt, bldalpha, bldy);
            }
        }
    }

    /// Modes 3–5 with windows, BG2 mosaic, sprites, and blend.
    fn render_bitmap(
        &mut self,
        dispcnt: u16,
        pal: &[u8],
        vram: &[u8],
        oam: &[u8],
        io: &[u8],
        mode: u16,
    ) {
        let one_d = dispcnt & (1 << 6) != 0;
        let obj_on = dispcnt & (1 << 12) != 0;
        let objwin = dispcnt & (1 << 15) != 0;
        let bg2_on = dispcnt & (1 << 10) != 0;
        let frame1 = dispcnt & (1 << 4) != 0;
        let mosaic_reg = io_u16(io, 0x4C);
        let bldcnt = io_u16(io, 0x50);
        let bldalpha = io_u16(io, 0x52);
        let bldy = io_u16(io, 0x54);
        let bg2cnt = io_u16(io, 0x0C);
        let bg2_pri = (bg2cnt & 0b11) as u8;
        let bg2_mosaic = bg2cnt & (1 << 6) != 0;
        let backdrop = self.pixels[0];

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let in_obj_window =
                    objwin && obj_on && obj::obj_window_hit(x, y, oam, vram, one_d, true);
                let mask = window::window_mask(x, y, dispcnt, io, in_obj_window);

                let mut layers = [Composite::empty(); 5];
                let mut n = 0usize;

                if bg2_on && mask.bg[2] {
                    // Mode 5: screen pixels outside 160×128 stay backdrop.
                    let on_screen = match mode {
                        5 => x < 160 && y < 128,
                        _ => true,
                    };
                    if on_screen {
                        let (sx, sy) = if bg2_mosaic {
                            mosaic::bg_origin(x, y, mosaic_reg)
                        } else {
                            (x, y)
                        };
                        if let Some(color) = sample_bitmap(mode, sx, sy, pal, vram, frame1) {
                            layers[n] = Composite {
                                color,
                                priority: bg2_pri,
                                is_sprite: false,
                                bg_index: 2,
                                layer: 2,
                                semi: false,
                            };
                            n += 1;
                        }
                    }
                }

                if obj_on && mask.obj {
                    if let Some(sp) =
                        obj::sprite_pixel(x, y, oam, pal, vram, one_d, true, mosaic_reg)
                    {
                        layers[n] = Composite {
                            color: sp.color,
                            priority: sp.priority,
                            is_sprite: true,
                            bg_index: 0xFF,
                            layer: 4,
                            semi: sp.semi,
                        };
                        n += 1;
                    }
                }

                self.pixels[x + y * WIDTH] =
                    finalize_pixel(&layers[..n], backdrop, mask.blend, bldcnt, bldalpha, bldy);
            }
        }
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

struct TextOrAffine {
    color: u16,
    priority: u8,
}

fn sample_text_bg(
    bg: usize,
    x: usize,
    y: usize,
    pal: &[u8],
    vram: &[u8],
    io: &[u8],
) -> Option<TextOrAffine> {
    let bgcnt = io_u16(io, 0x08 + bg * 2);
    let hofs = io_u16(io, 0x10 + bg * 4);
    let vofs = io_u16(io, 0x12 + bg * 4);
    bg::text_bg_pixel(x, y, bgcnt, hofs, vofs, pal, vram).map(|p| TextOrAffine {
        color: p.color,
        priority: p.priority,
    })
}

fn sample_affine_bg(
    bg: usize,
    x: usize,
    y: usize,
    pal: &[u8],
    vram: &[u8],
    io: &[u8],
) -> Option<TextOrAffine> {
    let bgcnt = io_u16(io, 0x08 + bg * 2);
    let (pa, pb, pc, pd, x0, y0) = if bg == 2 {
        (
            io_u16(io, 0x20) as i16,
            io_u16(io, 0x22) as i16,
            io_u16(io, 0x24) as i16,
            io_u16(io, 0x26) as i16,
            sign_extend_28(io_u32(io, 0x28)),
            sign_extend_28(io_u32(io, 0x2C)),
        )
    } else {
        (
            io_u16(io, 0x30) as i16,
            io_u16(io, 0x32) as i16,
            io_u16(io, 0x34) as i16,
            io_u16(io, 0x36) as i16,
            sign_extend_28(io_u32(io, 0x38)),
            sign_extend_28(io_u32(io, 0x3C)),
        )
    };
    affine::affine_bg_pixel(x as i32, y as i32, bgcnt, pa, pb, pc, pd, x0, y0, pal, vram).map(|p| {
        TextOrAffine {
            color: p.color,
            priority: p.priority,
        }
    })
}

fn sample_bitmap(
    mode: u16,
    x: usize,
    y: usize,
    pal: &[u8],
    vram: &[u8],
    frame1: bool,
) -> Option<u16> {
    match mode {
        3 => {
            let off = (x + y * WIDTH) * 2;
            Some(read_u16_le(vram, off) & 0x7FFF)
        }
        4 => {
            let base = if frame1 { 0xA000 } else { 0 };
            let index = vram.get(base + x + y * WIDTH).copied().unwrap_or(0) as usize;
            Some(read_u16_le(pal, index * 2) & 0x7FFF)
        }
        5 => {
            const MW: usize = 160;
            const MH: usize = 128;
            if x >= MW || y >= MH {
                return None;
            }
            let base = if frame1 { 0xA000 } else { 0 };
            let off = base + (x + y * MW) * 2;
            Some(read_u16_le(vram, off) & 0x7FFF)
        }
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct Composite {
    color: u16,
    priority: u8,
    is_sprite: bool,
    bg_index: u8,
    layer: u8,
    semi: bool,
}

impl Composite {
    const fn empty() -> Self {
        Self {
            color: 0,
            priority: 0,
            is_sprite: false,
            bg_index: 0,
            layer: 0,
            semi: false,
        }
    }
}

/// True when `a` is strictly in front of `b` (same rules as the old `consider`).
fn front_of(a: &Composite, b: &Composite) -> bool {
    if a.priority != b.priority {
        return a.priority < b.priority;
    }
    if !a.is_sprite && b.is_sprite {
        return true;
    }
    if a.is_sprite && !b.is_sprite {
        return false;
    }
    if !a.is_sprite && !b.is_sprite {
        return a.bg_index < b.bg_index;
    }
    false
}

fn finalize_pixel(
    layers: &[Composite],
    backdrop: u16,
    mask_blend: bool,
    bldcnt: u16,
    bldalpha: u16,
    bldy: u16,
) -> u16 {
    if layers.is_empty() {
        if mask_blend && blend::first_target(bldcnt, 5) {
            let effect = (bldcnt >> 6) & 0b11;
            if effect == 2 || effect == 3 {
                return blend::blend(backdrop, None, bldcnt, bldalpha, bldy, false);
            }
        }
        return backdrop;
    }

    let mut top_i = 0;
    for i in 1..layers.len() {
        if front_of(&layers[i], &layers[top_i]) {
            top_i = i;
        }
    }
    let top = &layers[top_i];
    let mut color = top.color;

    if mask_blend {
        let force_alpha = top.semi;
        if force_alpha || blend::first_target(bldcnt, top.layer) {
            let bottom = find_blend_bottom(layers, top_i, bldcnt, backdrop);
            color = blend::blend(top.color, bottom, bldcnt, bldalpha, bldy, force_alpha);
        }
    }

    color
}

/// First opaque layer behind `top` is the only candidate. Blend with it only
/// when that candidate is a 2nd target. If nothing opaque is behind, the
/// backdrop is the candidate (layer 5). Do not skip a non-2nd-target layer.
fn find_blend_bottom(
    layers: &[Composite],
    top_i: usize,
    bldcnt: u16,
    backdrop: u16,
) -> Option<u16> {
    let top = &layers[top_i];
    let mut next: Option<&Composite> = None;
    for (i, layer) in layers.iter().enumerate() {
        if i == top_i {
            continue;
        }
        if !front_of(top, layer) {
            continue;
        }
        match next {
            None => next = Some(layer),
            Some(cur) if front_of(layer, cur) => next = Some(layer),
            _ => {}
        }
    }
    if let Some(layer) = next {
        if blend::second_target(bldcnt, layer.layer) {
            Some(layer.color)
        } else {
            None
        }
    } else if blend::second_target(bldcnt, 5) {
        Some(backdrop)
    } else {
        None
    }
}

fn io_u16(io: &[u8], offset: usize) -> u16 {
    read_u16_le(io, offset)
}

fn io_u32(io: &[u8], offset: usize) -> u32 {
    let lo = u32::from(io_u16(io, offset));
    let hi = u32::from(io_u16(io, offset + 2));
    lo | (hi << 16)
}

/// Sign-extend a 28-bit BG reference point (bit 27 is the sign).
fn sign_extend_28(raw: u32) -> i32 {
    ((raw << 4) as i32) >> 4
}

fn read_u16_le(bytes: &[u8], offset: usize) -> u16 {
    let lo = u16::from(bytes.get(offset).copied().unwrap_or(0));
    let hi = u16::from(bytes.get(offset + 1).copied().unwrap_or(0));
    lo | (hi << 8)
}
