//! P3–P6 MMIO dispatch — timers / IRQ / keypad / hw / LCD / DMA / sound ports.
//!
//! Cited: GBATEK — Memory Map / Interrupt Control / Timers / Keypad / LCD I/O / DMA / Sound
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/05-io-timers-irq-input.md` §2.3
//!   Project store `docs/graycart-gba/03-ppu.md` §13
//!   Project store `docs/graycart-gba/02-memory-bus-dma.md` §8
//!   Project store `docs/graycart-gba/04-apu.md` §2
//! Note: sound FIFO / SOUNDCNT_* owned by APU (P6).
//! IRQ delay / IO write latency TBD (IO-TBD-4).

use crate::apu::Apu;
use crate::bus::mirror::io_offset;
use crate::bus::{Bus, CpuMem};
use crate::cart::Cart;
use crate::dma::Dma;
use crate::hw::Hw;
use crate::input::Input;
use crate::irq::Irq;
use crate::ppu::Ppu;
use crate::timer::Timers;

/// CPU-facing memory view that routes known I/O ports to subsystems.
pub struct MachineMem<'a> {
    pub bus: &'a mut Bus,
    pub irq: &'a mut Irq,
    pub timer: &'a mut Timers,
    pub input: &'a mut Input,
    pub hw: &'a mut Hw,
    pub ppu: &'a mut Ppu,
    pub dma: &'a mut Dma,
    pub apu: &'a mut Apu,
    pub cart: &'a mut Cart,
}

impl MachineMem<'_> {
    fn read_io8(&self, off: usize) -> u8 {
        let half = self.read_io16(off & !1);
        if off & 1 == 0 {
            half as u8
        } else {
            (half >> 8) as u8
        }
    }

    fn read_io16(&self, off: usize) -> u16 {
        match off {
            // LCD I/O 0x000–0x056
            o if o <= 0x56 => self.ppu.read16(o),
            // Sound 0x060–0x0A6
            o if Apu::owns_offset(o) => self.apu.read16(o),
            // DMA0–3 CNT_H readable; other DMA regs open-bus → 0 via dma helper
            o if (0xB0..0xE0).contains(&o) => self.dma.read_mmio16((o - 0xB0) as u32),
            // Timers 0–3
            o if (0x100..0x110).contains(&o) => self.timer.read_mmio16(o - 0x100),
            // SIO data low / high as 16-bit views of sio_data32
            0x120 => self.hw.sio_data32 as u16,
            0x122 => (self.hw.sio_data32 >> 16) as u16,
            0x128 => self.hw.read_siocnt(),
            0x130 => self.input.read_keyinput(),
            0x132 => self.input.read_keycnt(),
            0x134 => self.hw.read_rcnt(),
            0x200 => self.irq.read_ie(),
            0x202 => self.irq.read_if(),
            0x204 => self.hw.read_waitcnt(),
            0x208 => self.irq.read_ime() as u16,
            0x300 => u16::from(self.hw.read_postflg()), // HALTCNT write-only
            _ => {
                let lo = u16::from(self.bus.io.get(off).copied().unwrap_or(0));
                let hi = u16::from(self.bus.io.get(off + 1).copied().unwrap_or(0));
                lo | (hi << 8)
            }
        }
    }

    fn write_io8(&mut self, off: usize, value: u8) {
        if off == 0x301 {
            self.hw.write_haltcnt(value);
            return;
        }
        if off == 0x300 {
            self.hw.write_postflg(value);
            self.poke_io8(0x300, value);
            return;
        }
        // FIFO byte writes: push as low byte of a word slot (partial write still advances).
        if off == 0xA0 || off == 0xA4 {
            let word = u32::from(value);
            self.apu.write32(off, word);
            self.mirror_u16(off, self.apu.read16(off));
            return;
        }
        let aligned = off & !1;
        let cur = self.read_io16(aligned);
        let next = if off & 1 == 0 {
            (cur & 0xFF00) | u16::from(value)
        } else {
            (cur & 0x00FF) | (u16::from(value) << 8)
        };
        self.write_io16(aligned, next);
    }

    fn write_io16(&mut self, off: usize, value: u16) {
        match off {
            o if o <= 0x56 => {
                self.ppu.write16(o, value);
                self.mirror_u16(o, self.ppu.read16(o));
            }
            o if Apu::owns_offset(o) => {
                self.apu.write16(o, value);
                self.mirror_u16(o, self.apu.read16(o));
            }
            o if (0xB0..0xE0).contains(&o) => {
                self.dma.write_mmio16((o - 0xB0) as u32, value);
                self.mirror_u16(o, self.dma.read_mmio16((o - 0xB0) as u32));
            }
            o if (0x100..0x110).contains(&o) => {
                self.timer.write_mmio16(o - 0x100, value);
                self.mirror_u16(o, self.timer.read_mmio16(o - 0x100));
            }
            0x120 => {
                let hi = self.hw.sio_data32 & 0xFFFF_0000;
                self.hw.write_sio_data32(hi | u32::from(value));
                self.mirror_u16(0x120, value);
            }
            0x122 => {
                let lo = self.hw.sio_data32 & 0x0000_FFFF;
                self.hw.write_sio_data32(lo | (u32::from(value) << 16));
                self.mirror_u16(0x122, value);
            }
            0x128 => {
                self.hw.write_siocnt(value);
                self.mirror_u16(0x128, self.hw.read_siocnt());
            }
            0x130 => {
                // KEYINPUT is read-only (active-low pad state).
            }
            0x132 => {
                self.input.write_keycnt(value);
                self.mirror_u16(0x132, self.input.read_keycnt());
            }
            0x134 => {
                self.hw.write_rcnt(value);
                self.mirror_u16(0x134, self.hw.read_rcnt());
            }
            0x200 => {
                self.irq.write_ie(value);
                self.mirror_u16(0x200, self.irq.read_ie());
            }
            0x202 => {
                self.irq.write_if_ack(value);
                self.mirror_u16(0x202, self.irq.read_if());
            }
            0x204 => {
                self.hw.write_waitcnt(value);
                self.mirror_u16(0x204, self.hw.read_waitcnt());
            }
            0x208 => {
                self.irq.write_ime(u32::from(value));
                self.mirror_u16(0x208, self.irq.read_ime() as u16);
            }
            0x300 => {
                self.hw.write_postflg(value as u8);
                self.hw.write_haltcnt((value >> 8) as u8);
                self.poke_io8(0x300, self.hw.read_postflg());
            }
            _ => {
                self.mirror_u16(off, value);
            }
        }
    }

    fn write_io32(&mut self, off: usize, value: u32) {
        match off {
            o if Apu::owns_offset(o) && (o & 3) == 0 => {
                self.apu.write32(o, value);
                self.mirror_u16(o, self.apu.read16(o));
                self.mirror_u16(o + 2, self.apu.read16(o + 2));
            }
            o if (0xB0..0xE0).contains(&o) && (o & 3) == 0 => {
                self.dma.write_mmio32((o - 0xB0) as u32, value);
                self.mirror_u16(o, self.dma.read_mmio16((o - 0xB0) as u32));
                self.mirror_u16(o + 2, self.dma.read_mmio16((o + 2 - 0xB0) as u32));
            }
            o if (0x100..0x110).contains(&o) && (o & 3) == 0 => {
                self.timer.write_mmio32(o - 0x100, value);
                self.mirror_u16(o, self.timer.read_mmio16(o - 0x100));
                self.mirror_u16(o + 2, self.timer.read_mmio16(o + 2 - 0x100));
            }
            0x120 => {
                self.hw.write_sio_data32(value);
                self.mirror_u16(0x120, value as u16);
                self.mirror_u16(0x122, (value >> 16) as u16);
            }
            0x200 => {
                // IE (lo) + IF W1C (hi) in one store.
                self.irq.write_ie(value as u16);
                self.irq.write_if_ack((value >> 16) as u16);
                self.mirror_u16(0x200, self.irq.read_ie());
                self.mirror_u16(0x202, self.irq.read_if());
            }
            0x208 => {
                self.irq.write_ime(value);
                self.mirror_u16(0x208, self.irq.read_ime() as u16);
                self.mirror_u16(0x20A, 0);
            }
            _ => {
                self.write_io16(off, value as u16);
                self.write_io16(off + 2, (value >> 16) as u16);
            }
        }
    }

    fn mirror_u16(&mut self, off: usize, value: u16) {
        self.poke_io8(off, value as u8);
        self.poke_io8(off + 1, (value >> 8) as u8);
    }

    fn poke_io8(&mut self, off: usize, value: u8) {
        if let Some(slot) = self.bus.io.get_mut(off) {
            *slot = value;
        }
    }
}

impl CpuMem for MachineMem<'_> {
    fn read8(&mut self, addr: u32) -> u8 {
        if let Some(off) = io_offset(addr) {
            return self.read_io8(off);
        }
        if crate::bus::region::decode(addr) == crate::bus::region::Region::GamePakSram {
            return self.cart.save.read8(addr);
        }
        self.bus.read8(addr)
    }

    fn write8(&mut self, addr: u32, value: u8) {
        if let Some(off) = io_offset(addr) {
            self.write_io8(off, value);
            return;
        }
        if crate::bus::region::decode(addr) == crate::bus::region::Region::GamePakSram {
            self.cart.save.write8(addr, value);
            return;
        }
        self.bus.write8(addr, value);
    }

    fn read16(&mut self, addr: u32) -> u16 {
        if let Some(off) = io_offset(addr) {
            return self.read_io16(off);
        }
        if crate::bus::region::decode(addr) == crate::bus::region::Region::GamePakSram {
            let b = u16::from(self.cart.save.read8(addr));
            return b | (b << 8);
        }
        self.bus.read16(addr)
    }

    fn write16(&mut self, addr: u32, value: u16) {
        if let Some(off) = io_offset(addr) {
            self.write_io16(off, value);
            return;
        }
        if crate::bus::region::decode(addr) == crate::bus::region::Region::GamePakSram {
            let rotated = value.rotate_right((addr & 1) * 8);
            self.cart.save.write8(addr, rotated as u8);
            return;
        }
        self.bus.write16(addr, value);
    }

    fn read32(&mut self, addr: u32) -> u32 {
        if let Some(off) = io_offset(addr) {
            let lo = u32::from(self.read_io16(off));
            let hi = u32::from(self.read_io16(off + 2));
            return lo | (hi << 16);
        }
        if crate::bus::region::decode(addr) == crate::bus::region::Region::GamePakSram {
            let b = u32::from(self.cart.save.read8(addr));
            return b * 0x0101_0101;
        }
        self.bus.read32(addr)
    }

    fn write32(&mut self, addr: u32, value: u32) {
        if let Some(off) = io_offset(addr) {
            self.write_io32(off, value);
            return;
        }
        if crate::bus::region::decode(addr) == crate::bus::region::Region::GamePakSram {
            let rotated = value.rotate_right((addr & 3) * 8);
            self.cart.save.write8(addr, rotated as u8);
            return;
        }
        self.bus.write32(addr, value);
    }
}

/// Bus + APU view for DMA drains so FIFO dest writes feed the APU.
pub struct BusApuMem<'a> {
    pub bus: &'a mut Bus,
    pub apu: &'a mut Apu,
}

impl CpuMem for BusApuMem<'_> {
    fn read8(&mut self, addr: u32) -> u8 {
        self.bus.read8(addr)
    }
    fn write8(&mut self, addr: u32, value: u8) {
        if let Some(off) = io_offset(addr) {
            if off == 0xA0 || off == 0xA4 {
                self.apu.write32(off, u32::from(value));
                return;
            }
        }
        self.bus.write8(addr, value);
    }
    fn read16(&mut self, addr: u32) -> u16 {
        self.bus.read16(addr)
    }
    fn write16(&mut self, addr: u32, value: u16) {
        if let Some(off) = io_offset(addr) {
            if Apu::owns_offset(off) {
                self.apu.write16(off, value);
                return;
            }
        }
        self.bus.write16(addr, value);
    }
    fn read32(&mut self, addr: u32) -> u32 {
        self.bus.read32(addr)
    }
    fn write32(&mut self, addr: u32, value: u32) {
        if let Some(off) = io_offset(addr) {
            if off == 0xA0 || off == 0xA4 {
                self.apu.write32(off, value);
                return;
            }
            if Apu::owns_offset(off) {
                self.apu.write32(off, value);
                return;
            }
        }
        self.bus.write32(addr, value);
    }
}
