//! P3 MMIO dispatch — side-effect handlers for timers / IRQ / keypad / hw regs.
//!
//! Cited: GBATEK — Memory Map / Interrupt Control / Timers / Keypad / System Control
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/05-io-timers-irq-input.md` §2.3
//! Note: LCD/sound/DMA ports stay dumb `bus.io` until their owners wire handlers.
//! IRQ delay / IO write latency TBD (IO-TBD-4).

use crate::bus::mirror::io_offset;
use crate::bus::{Bus, CpuMem};
use crate::hw::Hw;
use crate::input::Input;
use crate::irq::Irq;
use crate::timer::Timers;

/// CPU-facing memory view that routes known I/O ports to P3 subsystems.
pub struct MachineMem<'a> {
    pub bus: &'a mut Bus,
    pub irq: &'a mut Irq,
    pub timer: &'a mut Timers,
    pub input: &'a mut Input,
    pub hw: &'a mut Hw,
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
            o if (0x100..0x110).contains(&o) && o.is_multiple_of(4) => {
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
        self.bus.read8(addr)
    }

    fn write8(&mut self, addr: u32, value: u8) {
        if let Some(off) = io_offset(addr) {
            self.write_io8(off, value);
            return;
        }
        self.bus.write8(addr, value);
    }

    fn read16(&mut self, addr: u32) -> u16 {
        if let Some(off) = io_offset(addr) {
            return self.read_io16(off);
        }
        self.bus.read16(addr)
    }

    fn write16(&mut self, addr: u32, value: u16) {
        if let Some(off) = io_offset(addr) {
            self.write_io16(off, value);
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
        self.bus.read32(addr)
    }

    fn write32(&mut self, addr: u32, value: u32) {
        if let Some(off) = io_offset(addr) {
            self.write_io32(off, value);
            return;
        }
        self.bus.write32(addr, value);
    }
}
