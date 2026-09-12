//! LCD I/O register file (DISPCNT … BLDY) — P4.
//!
//! Cited: GBATEK — LCD I/O Display Control / BG Control / Special Effects
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/03-ppu.md` §13
//! Note: write-only scroll/affine/window/mosaic/BLDY read-back is open-bus TBD;
//!   this module returns latched values for functional tests.

/// DISPCNT (`04000000`).
pub const DISPCNT_ADDR: u32 = 0x0400_0000;
/// GREENSWAP undocumented (`04000002`).
pub const GREENSWAP_ADDR: u32 = 0x0400_0002;
/// DISPSTAT (`04000004`).
pub const DISPSTAT_ADDR: u32 = 0x0400_0004;
/// VCOUNT (`04000006`).
pub const VCOUNT_ADDR: u32 = 0x0400_0006;

/// LCD I/O register block owned by the PPU.
#[derive(Debug, Clone)]
pub struct LcdRegs {
    pub dispcnt: u16,
    pub greenswap: u16,
    /// Software-writable bits of DISPSTAT (IRQ enables + LYC). Flags are timing-owned.
    pub dispstat_w: u16,
    pub bgcnt: [u16; 4],
    pub bg_hofs: [u16; 4],
    pub bg_vofs: [u16; 4],
    /// BG2 PA–PD (signed 8.8).
    pub bg2_pa: i16,
    pub bg2_pb: i16,
    pub bg2_pc: i16,
    pub bg2_pd: i16,
    /// BG2 reference (28-bit; we keep full u32 write latch).
    pub bg2_x: u32,
    pub bg2_y: u32,
    pub bg3_pa: i16,
    pub bg3_pb: i16,
    pub bg3_pc: i16,
    pub bg3_pd: i16,
    pub bg3_x: u32,
    pub bg3_y: u32,
    pub win0_h: u16,
    pub win1_h: u16,
    pub win0_v: u16,
    pub win1_v: u16,
    pub winin: u16,
    pub winout: u16,
    pub mosaic: u16,
    pub bldcnt: u16,
    pub bldalpha: u16,
    pub bldy: u16,
}

impl Default for LcdRegs {
    fn default() -> Self {
        Self {
            dispcnt: 0,
            greenswap: 0,
            dispstat_w: 0,
            bgcnt: [0; 4],
            bg_hofs: [0; 4],
            bg_vofs: [0; 4],
            bg2_pa: 0x0100, // identity 1.0
            bg2_pb: 0,
            bg2_pc: 0,
            bg2_pd: 0x0100,
            bg2_x: 0,
            bg2_y: 0,
            bg3_pa: 0x0100,
            bg3_pb: 0,
            bg3_pc: 0,
            bg3_pd: 0x0100,
            bg3_x: 0,
            bg3_y: 0,
            win0_h: 0,
            win1_h: 0,
            win0_v: 0,
            win1_v: 0,
            winin: 0,
            winout: 0,
            mosaic: 0,
            bldcnt: 0,
            bldalpha: 0,
            bldy: 0,
        }
    }
}

impl LcdRegs {
    #[must_use]
    pub fn bg_mode(&self) -> u16 {
        self.dispcnt & 0x7
    }

    #[must_use]
    pub fn forced_blank(&self) -> bool {
        self.dispcnt & (1 << 7) != 0
    }

    #[must_use]
    pub fn frame_select(&self) -> bool {
        self.dispcnt & (1 << 4) != 0
    }

    #[must_use]
    pub fn obj_1d_mapping(&self) -> bool {
        self.dispcnt & (1 << 6) != 0
    }

    #[must_use]
    pub fn hblank_interval_free(&self) -> bool {
        self.dispcnt & (1 << 5) != 0
    }

    #[must_use]
    pub fn layer_enable(&self, bit: u16) -> bool {
        self.dispcnt & (1 << bit) != 0
    }

    /// MMIO halfword read for LCD ports (`off` = offset within `0x04000000`).
    #[must_use]
    pub fn read16(&self, off: usize, vcount: u16, dispstat_flags: u16) -> u16 {
        match off {
            0x00 => self.dispcnt,
            0x02 => self.greenswap,
            0x04 => (self.dispstat_w & !0x7) | (dispstat_flags & 0x7),
            0x06 => vcount & 0xFF,
            0x08 => self.bgcnt[0],
            0x0A => self.bgcnt[1],
            0x0C => self.bgcnt[2],
            0x0E => self.bgcnt[3],
            // Write-only scroll: return latched (functional; open-bus TBD).
            0x10 => self.bg_hofs[0],
            0x12 => self.bg_vofs[0],
            0x14 => self.bg_hofs[1],
            0x16 => self.bg_vofs[1],
            0x18 => self.bg_hofs[2],
            0x1A => self.bg_vofs[2],
            0x1C => self.bg_hofs[3],
            0x1E => self.bg_vofs[3],
            0x20 => self.bg2_pa as u16,
            0x22 => self.bg2_pb as u16,
            0x24 => self.bg2_pc as u16,
            0x26 => self.bg2_pd as u16,
            0x28 => self.bg2_x as u16,
            0x2A => (self.bg2_x >> 16) as u16,
            0x2C => self.bg2_y as u16,
            0x2E => (self.bg2_y >> 16) as u16,
            0x30 => self.bg3_pa as u16,
            0x32 => self.bg3_pb as u16,
            0x34 => self.bg3_pc as u16,
            0x36 => self.bg3_pd as u16,
            0x38 => self.bg3_x as u16,
            0x3A => (self.bg3_x >> 16) as u16,
            0x3C => self.bg3_y as u16,
            0x3E => (self.bg3_y >> 16) as u16,
            0x40 => self.win0_h,
            0x42 => self.win1_h,
            0x44 => self.win0_v,
            0x46 => self.win1_v,
            0x48 => self.winin,
            0x4A => self.winout,
            0x4C => self.mosaic,
            0x50 => self.bldcnt,
            0x52 => self.bldalpha,
            0x54 => self.bldy,
            _ => 0,
        }
    }

    /// MMIO halfword write for LCD ports.
    pub fn write16(&mut self, off: usize, value: u16) {
        match off {
            0x00 => self.dispcnt = value,
            0x02 => self.greenswap = value & 1,
            0x04 => {
                // Bits 0–2 are flags (timing); software writes IRQ enables + LYC.
                self.dispstat_w = value & !0x7;
            }
            0x06 => { /* VCOUNT read-only */ }
            0x08 => self.bgcnt[0] = value,
            0x0A => self.bgcnt[1] = value,
            0x0C => self.bgcnt[2] = value,
            0x0E => self.bgcnt[3] = value,
            0x10 => self.bg_hofs[0] = value & 0x1FF,
            0x12 => self.bg_vofs[0] = value & 0x1FF,
            0x14 => self.bg_hofs[1] = value & 0x1FF,
            0x16 => self.bg_vofs[1] = value & 0x1FF,
            0x18 => self.bg_hofs[2] = value & 0x1FF,
            0x1A => self.bg_vofs[2] = value & 0x1FF,
            0x1C => self.bg_hofs[3] = value & 0x1FF,
            0x1E => self.bg_vofs[3] = value & 0x1FF,
            0x20 => self.bg2_pa = value as i16,
            0x22 => self.bg2_pb = value as i16,
            0x24 => self.bg2_pc = value as i16,
            0x26 => self.bg2_pd = value as i16,
            0x28 => self.bg2_x = (self.bg2_x & 0xFFFF_0000) | u32::from(value),
            0x2A => {
                self.bg2_x = (self.bg2_x & 0x0000_FFFF) | (u32::from(value & 0x0FFF) << 16);
            }
            0x2C => self.bg2_y = (self.bg2_y & 0xFFFF_0000) | u32::from(value),
            0x2E => {
                self.bg2_y = (self.bg2_y & 0x0000_FFFF) | (u32::from(value & 0x0FFF) << 16);
            }
            0x30 => self.bg3_pa = value as i16,
            0x32 => self.bg3_pb = value as i16,
            0x34 => self.bg3_pc = value as i16,
            0x36 => self.bg3_pd = value as i16,
            0x38 => self.bg3_x = (self.bg3_x & 0xFFFF_0000) | u32::from(value),
            0x3A => {
                self.bg3_x = (self.bg3_x & 0x0000_FFFF) | (u32::from(value & 0x0FFF) << 16);
            }
            0x3C => self.bg3_y = (self.bg3_y & 0xFFFF_0000) | u32::from(value),
            0x3E => {
                self.bg3_y = (self.bg3_y & 0x0000_FFFF) | (u32::from(value & 0x0FFF) << 16);
            }
            0x40 => self.win0_h = value,
            0x42 => self.win1_h = value,
            0x44 => self.win0_v = value,
            0x46 => self.win1_v = value,
            0x48 => self.winin = value,
            0x4A => self.winout = value,
            0x4C => self.mosaic = value,
            0x50 => self.bldcnt = value,
            0x52 => self.bldalpha = value,
            0x54 => self.bldy = value,
            _ => {}
        }
    }

    /// DISPSTAT IRQ enable bits (software).
    #[must_use]
    pub fn irq_vblank_en(&self) -> bool {
        self.dispstat_w & (1 << 3) != 0
    }
    #[must_use]
    pub fn irq_hblank_en(&self) -> bool {
        self.dispstat_w & (1 << 4) != 0
    }
    #[must_use]
    pub fn irq_vcount_en(&self) -> bool {
        self.dispstat_w & (1 << 5) != 0
    }
    #[must_use]
    pub fn lyc(&self) -> u16 {
        (self.dispstat_w >> 8) & 0xFF
    }
}
