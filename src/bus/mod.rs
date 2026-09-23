//! Memory map for the pages that can run a ROM.
//!
//! Cited: GBATEK, GBA Memory Map. https://problemkaputt.de/gbatek.htm

use std::collections::HashSet;

use crate::apu::Apu;
use crate::cart::{detect_save, gpio_reject_line, Eeprom, Flash, FlashSize, SaveKind};
use crate::dma::{self, Dma, StartReason};
use crate::input::Keypad;
use crate::irq::Irq;
use crate::timer::Timers;

const EWRAM: usize = 256 * 1024;
const IWRAM: usize = 32 * 1024;
const PAL: usize = 1024;
const VRAM: usize = 96 * 1024;
const OAM: usize = 1024;
const IO: usize = 0x400;
const SRAM: usize = 64 * 1024;
const BIOS: usize = 16 * 1024;

/// Cartridge backup chip held on the bus.
#[derive(Debug)]
enum SaveChip {
    None,
    Sram(Vec<u8>),
    Flash(Flash),
    Eeprom(Eeprom),
}

#[derive(Debug)]
pub struct Bus {
    pub rom: Vec<u8>,
    ewram: Vec<u8>,
    iwram: Vec<u8>,
    pal: Vec<u8>,
    vram: Vec<u8>,
    oam: Vec<u8>,
    io: Vec<u8>,
    save: SaveChip,
    save_kind: SaveKind,
    /// When true, 0x080000C4/C6/C8 are cart GPIO (unsupported). Default false.
    has_gpio: bool,
    gpio_warned: bool,
    pub timers: Timers,
    pub irq: Irq,
    pub keypad: Keypad,
    pub dma: Dma,
    pub apu: Apu,
    /// Set by HALTCNT byte 0 or SWI 2; cleared when an enabled IRQ wakes the CPU.
    pub halted: bool,
    halt_forever_warned: bool,
    pub vblank: bool,
    pub hblank: bool,
    pub vcount: u16,
    last_data: u32,
    bios_prefetch: u32,
    pub warn_lines: Vec<String>,
    openbus_logged: HashSet<u32>,
    sio_warned: bool,
    /// Placeholder waitstate costs (still 1 everywhere until page 11).
    pub wait_rom_n: u32,
    pub wait_rom_s: u32,
    pub wait_rom_i: u32,
    pub wait_sram: u32,
}

impl Bus {
    pub fn new(rom: Vec<u8>) -> Self {
        let kind = detect_save(&rom);
        let save = match kind {
            SaveKind::None => SaveChip::None,
            SaveKind::Sram => SaveChip::Sram(vec![0xFF; SRAM]),
            SaveKind::Flash64 => SaveChip::Flash(Flash::new(FlashSize::K64)),
            SaveKind::Flash128 => SaveChip::Flash(Flash::new(FlashSize::K128)),
            SaveKind::Eeprom => SaveChip::Eeprom(Eeprom::new()),
        };
        Self {
            rom,
            ewram: vec![0; EWRAM],
            iwram: vec![0; IWRAM],
            pal: vec![0; PAL],
            vram: vec![0; VRAM],
            oam: vec![0; OAM],
            io: vec![0; IO],
            save,
            save_kind: kind,
            has_gpio: false,
            gpio_warned: false,
            timers: Timers::new(),
            irq: Irq::new(),
            keypad: Keypad::new(),
            dma: Dma::new(),
            apu: Apu::new(),
            halted: false,
            halt_forever_warned: false,
            vblank: false,
            hblank: false,
            vcount: 0,
            last_data: 0,
            // Post-boot latch: last BIOS opcode word (at 0xE4), not a fetch of 0.
            bios_prefetch: 0xE129_F000,
            warn_lines: Vec::new(),
            openbus_logged: HashSet::new(),
            sio_warned: false,
            wait_rom_n: 1,
            wait_rom_s: 1,
            wait_rom_i: 1,
            wait_sram: 1,
        }
    }

    /// Detected save kind (from ROM ID strings).
    pub fn save_kind(&self) -> SaveKind {
        self.save_kind
    }

    /// Enable unsupported cart GPIO at 0x080000C4/C6/C8 (tests only).
    pub fn set_has_gpio(&mut self, yes: bool) {
        self.has_gpio = yes;
    }

    /// Load sidecar bytes into the save chip (no-op for [`SaveKind::None`]).
    pub fn load_save(&mut self, bytes: &[u8]) {
        match &mut self.save {
            SaveChip::None => {}
            SaveChip::Sram(data) => {
                data.fill(0xFF);
                let n = bytes.len().min(data.len());
                data[..n].copy_from_slice(&bytes[..n]);
            }
            SaveChip::Flash(flash) => flash.load_bytes(bytes),
            SaveChip::Eeprom(eeprom) => eeprom.load_bytes(bytes),
        }
    }

    /// Current save-chip bytes for the sidecar, if any.
    pub fn save_bytes(&self) -> Option<&[u8]> {
        match &self.save {
            SaveChip::None => None,
            SaveChip::Sram(data) => Some(data.as_slice()),
            SaveChip::Flash(flash) => Some(flash.bytes()),
            SaveChip::Eeprom(eeprom) => {
                let b = eeprom.bytes();
                if b.is_empty() {
                    None
                } else {
                    Some(b)
                }
            }
        }
    }

    /// Record `gba-debug: warn halt forever` at most once per machine.
    pub fn warn_halt_forever(&mut self) {
        if self.halt_forever_warned {
            return;
        }
        self.halt_forever_warned = true;
        self.warn_lines
            .push("gba-debug: warn halt forever".to_string());
    }

    pub fn wait_line(&self, frame: u32) -> String {
        format!(
            "gba-debug: wait frame={frame} rom_n={} rom_s={} rom_i={} sram={}",
            self.wait_rom_n, self.wait_rom_s, self.wait_rom_i, self.wait_sram
        )
    }

    /// DISPCNT halfword at I/O offset 0.
    pub fn dispcnt(&self) -> u16 {
        slice_load(&self.io, 0, 2) as u16
    }

    /// Palette RAM (1 KiB).
    pub fn palette(&self) -> &[u8] {
        &self.pal
    }

    /// Video RAM (96 KiB).
    pub fn vram(&self) -> &[u8] {
        &self.vram
    }

    /// Object Attribute Memory (1 KiB).
    pub fn oam(&self) -> &[u8] {
        &self.oam
    }

    /// I/O registers (1 KiB, offsets from 0x04000000).
    pub fn io(&self) -> &[u8] {
        &self.io
    }

    /// Writable DISPSTAT bits (IRQ enables and LYC), ignoring read-only status.
    pub fn dispstat_written(&self) -> u16 {
        slice_load(&self.io, 4, 2) as u16
    }

    /// True when VCOUNT equals the DISPSTAT LYC byte.
    pub fn vcount_match(&self) -> bool {
        let lyc = (self.dispstat_written() >> 8) as u8;
        self.vcount as u8 == lyc
    }

    pub fn fetch16(&mut self, addr: u32) -> u16 {
        let addr = addr & !1;
        let bits = self.access(addr, 2, Access::Fetch);
        bits as u16
    }

    pub fn fetch32(&mut self, addr: u32) -> u32 {
        self.access(addr & !3, 4, Access::Fetch)
    }

    pub fn read8(&mut self, addr: u32) -> u8 {
        self.access(addr, 1, Access::Data).to_le_bytes()[0]
    }

    pub fn read16(&mut self, addr: u32) -> u16 {
        let addr = align_data(addr, 2);
        let bits = self.access(addr, 2, Access::Data);
        bits as u16
    }

    pub fn read32(&mut self, addr: u32) -> u32 {
        let addr = align_data(addr, 4);
        self.access(addr, 4, Access::Data)
    }

    pub fn write8(&mut self, addr: u32, value: u8) {
        self.store(addr, value as u32, 1);
    }

    pub fn write16(&mut self, addr: u32, value: u16) {
        let addr = align_data(addr, 2);
        self.store(addr, value as u32, 2);
    }

    pub fn write32(&mut self, addr: u32, value: u32) {
        let addr = align_data(addr, 4);
        self.store(addr, value, 4);
    }

    fn access(&mut self, addr: u32, size: u32, kind: Access) -> u32 {
        let region = addr >> 24;
        match region {
            0x00 if (addr as usize) < BIOS => {
                if kind == Access::Fetch {
                    // No BIOS image: opcode is 0. Do not wipe the prefetch latch —
                    // data reads still need the last real BIOS word (post-boot, SWI, IRQ).
                    self.last_data = 0;
                    0
                } else {
                    self.open_slice(self.bios_prefetch, addr, size)
                }
            }
            0x02 => {
                let value = slice_load(&self.ewram, (addr & 0x3_FFFF) as usize, size);
                self.latch(value);
                value
            }
            0x03 => {
                let value = slice_load(&self.iwram, (addr & 0x7FFF) as usize, size);
                self.latch(value);
                value
            }
            0x04 => {
                let off = addr & 0x00FF_FFFF;
                if off >= IO as u32 {
                    return self.openbus(addr, size, "io");
                }
                let value = self.load_io(off, size);
                self.latch(value);
                value
            }
            0x05 => {
                let value = slice_load(&self.pal, (addr & 0x3FF) as usize, size);
                self.latch(value);
                value
            }
            0x06 => {
                let value = slice_load(&self.vram, vram_off(addr), size);
                self.latch(value);
                value
            }
            0x07 => {
                let value = slice_load(&self.oam, (addr & 0x3FF) as usize, size);
                self.latch(value);
                value
            }
            0x08..=0x0C => {
                if self.touch_gpio(addr) {
                    let value = 0;
                    self.latch(value);
                    return value;
                }
                let value = self.load_rom(addr, size);
                self.latch(value);
                value
            }
            0x0D => {
                if self.save_kind == SaveKind::Eeprom && size == 2 {
                    let bit = match &mut self.save {
                        SaveChip::Eeprom(eeprom) => u32::from(eeprom.read_bit()),
                        _ => 1,
                    };
                    self.latch(bit);
                    return bit;
                }
                let value = self.load_rom(addr, size);
                self.latch(value);
                value
            }
            0x0E | 0x0F => self.access_save(addr, size),
            _ => self.openbus(addr, size, "unused"),
        }
    }

    fn store(&mut self, addr: u32, value: u32, size: u32) {
        match addr >> 24 {
            0x02 => slice_store(&mut self.ewram, (addr & 0x3_FFFF) as usize, value, size),
            0x03 => slice_store(&mut self.iwram, (addr & 0x7FFF) as usize, value, size),
            0x04 => {
                let off = addr & 0x00FF_FFFF;
                if off >= IO as u32 {
                    return;
                }
                self.store_io(off, value, size);
            }
            0x05 => self.store_pal(addr, value, size),
            0x06 => self.store_vram(addr, value, size),
            0x07 => self.store_oam(addr, value, size),
            0x08..=0x0C => {
                let _ = self.touch_gpio(addr);
            }
            0x0D => {
                if self.save_kind == SaveKind::Eeprom && size == 2 {
                    if let SaveChip::Eeprom(eeprom) = &mut self.save {
                        eeprom.write_bit(value as u16);
                    }
                }
            }
            0x0E | 0x0F => self.store_save(addr, value, size),
            _ => {}
        }
    }

    /// SRAM / flash are an 8-bit bus: multi-byte reads duplicate the byte;
    /// multi-byte writes program only the lane that hits `addr`.
    fn access_save(&mut self, addr: u32, size: u32) -> u32 {
        let wide_ok = matches!(
            self.save_kind,
            SaveKind::Sram | SaveKind::Flash64 | SaveKind::Flash128
        );
        match size {
            1 => {
                let value = self.read_save8(addr);
                self.latch(value);
                value
            }
            2 | 4 if wide_ok => {
                let b = self.read_save8(addr) & 0xFF;
                let value = if size == 2 {
                    b | (b << 8)
                } else {
                    b | (b << 8) | (b << 16) | (b << 24)
                };
                self.latch(value);
                value
            }
            _ => self.openbus(addr, size, "sram"),
        }
    }

    fn store_save(&mut self, addr: u32, value: u32, size: u32) {
        let wide_ok = matches!(
            self.save_kind,
            SaveKind::Sram | SaveKind::Flash64 | SaveKind::Flash128
        );
        match size {
            1 => self.write_save8(addr, value as u8),
            2 | 4 if wide_ok => {
                let shift = (addr & (size - 1)) * 8;
                let byte = ((value >> shift) & 0xFF) as u8;
                self.write_save8(addr, byte);
            }
            _ => self.log_openbus(addr, "sram"),
        }
    }

    fn read_save8(&mut self, addr: u32) -> u32 {
        match &mut self.save {
            SaveChip::None | SaveChip::Eeprom(_) => 0xFF,
            SaveChip::Sram(data) => u32::from(data[(addr & 0xFFFF) as usize]),
            SaveChip::Flash(flash) => u32::from(flash.read(addr)),
        }
    }

    fn write_save8(&mut self, addr: u32, value: u8) {
        match &mut self.save {
            SaveChip::None | SaveChip::Eeprom(_) => {}
            SaveChip::Sram(data) => {
                data[(addr & 0xFFFF) as usize] = value;
            }
            SaveChip::Flash(flash) => flash.write(addr, value),
        }
    }

    /// Unsupported cart GPIO at the three fixed ROM addresses. Warns once.
    fn touch_gpio(&mut self, addr: u32) -> bool {
        if !self.has_gpio || !matches!(addr, 0x0800_00C4 | 0x0800_00C6 | 0x0800_00C8) {
            return false;
        }
        if !self.gpio_warned {
            self.gpio_warned = true;
            self.warn_lines.push(gpio_reject_line().to_string());
        }
        true
    }

    fn eeprom_region(addr: u32) -> bool {
        (0x0D00_0000..=0x0DFF_FFFF).contains(&addr)
    }

    fn store_pal(&mut self, addr: u32, value: u32, size: u32) {
        let off = (addr & 0x3FF) as usize;
        if size == 1 {
            let aligned = off & !1;
            if let Some(slot) = self.pal.get_mut(aligned) {
                *slot = value as u8;
            }
            if let Some(slot) = self.pal.get_mut(aligned | 1) {
                *slot = value as u8;
            }
            return;
        }
        slice_store(&mut self.pal, off, value, size);
    }

    fn store_vram(&mut self, addr: u32, value: u32, size: u32) {
        let off = vram_off(addr);
        if size == 1 {
            if self.vram_obj_window(off) {
                return;
            }
            let aligned = off & !1;
            if let Some(slot) = self.vram.get_mut(aligned) {
                *slot = value as u8;
            }
            if let Some(slot) = self.vram.get_mut(aligned | 1) {
                *slot = value as u8;
            }
            return;
        }
        slice_store(&mut self.vram, off, value, size);
    }

    fn store_oam(&mut self, addr: u32, value: u32, size: u32) {
        if size == 1 {
            return;
        }
        slice_store(&mut self.oam, (addr & 0x3FF) as usize, value, size);
    }

    fn vram_obj_window(&self, off: usize) -> bool {
        let mode = slice_load(&self.io, 0, 2) as u16 & 7;
        if mode >= 3 {
            (0x1_4000..0x1_8000).contains(&off)
        } else {
            (0x1_0000..0x1_8000).contains(&off)
        }
    }

    fn load_io(&mut self, off: u32, size: u32) -> u32 {
        if sio_touches(off, size) {
            self.warn_sio();
            return 0;
        }
        if let Some(value) = self.load_io_device(off, size) {
            return value;
        }
        let mut raw = slice_load(&self.io, off as usize, size);
        // DISPSTAT low byte is at offset 4; bits 0–2 are read-only status.
        if covers_byte(off, size, 4) {
            let shift = (4 - off) * 8;
            let status = u32::from(self.vblank)
                | (u32::from(self.hblank) << 1)
                | (u32::from(self.vcount_match()) << 2);
            raw = (raw & !(7u32 << shift)) | (status << shift);
        }
        // VCOUNT: offset 6 is the live scanline; offset 7 reads as 0.
        if covers_byte(off, size, 6) {
            let shift = (6 - off) * 8;
            raw = (raw & !(0xFFu32 << shift)) | (u32::from(self.vcount as u8) << shift);
        }
        if covers_byte(off, size, 7) {
            let shift = (7 - off) * 8;
            raw &= !(0xFFu32 << shift);
        }
        raw
    }

    fn store_io(&mut self, off: u32, value: u32, size: u32) {
        if self.store_io_device(off, value, size) {
            return;
        }
        let mut value = value;
        // DISPSTAT bits 0–2 are read-only; only mask when writing the low byte.
        if covers_byte(off, size, 4) {
            let shift = (4 - off) * 8;
            value &= !(7u32 << shift);
        }
        // VCOUNT (offsets 6–7) is read-only; writes must not stick in io[].
        if covers_byte(off, size, 6) {
            let shift = (6 - off) * 8;
            value &= !(0xFFu32 << shift);
        }
        if covers_byte(off, size, 7) {
            let shift = (7 - off) * 8;
            value &= !(0xFFu32 << shift);
        }
        slice_store(&mut self.io, off as usize, value, size);
    }

    /// Timers, keypad, IRQ, DMA, and sound registers. `None` means fall through to flat `io[]`.
    fn load_io_device(&self, off: u32, size: u32) -> Option<u32> {
        if sound_covers(off, size) {
            return Some(self.load_sound_io(off, size));
        }
        if (0x100..0x110).contains(&off) {
            return Some(self.load_timer_io(off, size));
        }
        if Dma::covers(off, size) {
            return Some(self.dma.load(off, size));
        }
        if covers_byte(off, size, 0x130) || covers_byte(off, size, 0x131) {
            return Some(self.load_key_io(off, size));
        }
        if covers_byte(off, size, 0x132) || covers_byte(off, size, 0x133) {
            return Some(self.load_key_io(off, size));
        }
        if matches!(off, 0x200..=0x203 | 0x208..=0x209) {
            return Some(self.load_irq_io(off, size));
        }
        None
    }

    fn store_io_device(&mut self, off: u32, value: u32, size: u32) -> bool {
        if sound_covers(off, size) {
            self.store_sound_io(off, value, size);
            return true;
        }
        if (0x100..0x110).contains(&off) {
            self.store_timer_io(off, value, size);
            return true;
        }
        if Dma::covers(off, size) {
            if let Some(channel) = self.dma.store(off, value, size) {
                self.dma_on_enable(channel);
            }
            return true;
        }
        if covers_byte(off, size, 0x130) || covers_byte(off, size, 0x131) {
            // KEYINPUT is read-only; ignore writes to that halfword.
            if covers_byte(off, size, 0x132) || covers_byte(off, size, 0x133) {
                self.store_key_io(off, value, size);
            }
            return true;
        }
        if covers_byte(off, size, 0x132) || covers_byte(off, size, 0x133) {
            self.store_key_io(off, value, size);
            return true;
        }
        if matches!(off, 0x200..=0x203 | 0x208..=0x209) {
            self.store_irq_io(off, value, size);
            return true;
        }
        // HALTCNT: only a byte write of 0 at 0x04000301 requests halt.
        if off == 0x301 && size == 1 {
            if value as u8 == 0 {
                self.halted = true;
            }
            return true;
        }
        false
    }

    /// Tick the APU for one CPU cycle (same cadence as the timers).
    ///
    /// On a FIFO DMA request, runs channel 1 (A) or 2 (B) through [`Self::request_fifo`].
    pub fn tick_apu(&mut self, timer_overflow: u8) {
        let (req_a, req_b) = self.apu.tick(timer_overflow);
        if req_a {
            self.request_fifo(1);
        }
        if req_b {
            self.request_fifo(2);
        }
    }

    fn load_sound_io(&self, off: u32, size: u32) -> u32 {
        let mut raw = slice_load(&self.io, off as usize, size);
        if covers_byte(off, size, 0x84) || covers_byte(off, size, 0x85) {
            let master = u32::from(self.apu.psg.read_master());
            for byte in 0u32..2 {
                let addr = 0x84 + byte;
                if covers_byte(off, size, addr) {
                    let shift = (addr - off) * 8;
                    let piece = (master >> (8 * byte)) & 0xFF;
                    raw = (raw & !(0xFFu32 << shift)) | (piece << shift);
                }
            }
        }
        raw
    }

    fn store_sound_io(&mut self, off: u32, value: u32, size: u32) {
        match size {
            1 => {
                let aligned = off & !1;
                let cur = slice_load(&self.io, aligned as usize, 2) as u16;
                let next = if off & 1 != 0 {
                    (cur & 0x00FF) | (((value as u16) & 0xFF) << 8)
                } else {
                    (cur & 0xFF00) | ((value as u16) & 0xFF)
                };
                slice_store(&mut self.io, aligned as usize, u32::from(next), 2);
                if !(0xA0..=0xA7).contains(&off) {
                    self.apply_sound16(aligned, next);
                }
            }
            2 => {
                slice_store(&mut self.io, off as usize, value, 2);
                if !(0xA0..=0xA7).contains(&off) {
                    self.apply_sound16(off, value as u16);
                }
            }
            4 => {
                slice_store(&mut self.io, off as usize, value, 4);
                if off != 0xA0 && off != 0xA4 {
                    self.apply_sound16(off, value as u16);
                    self.apply_sound16(off.wrapping_add(2), (value >> 16) as u16);
                }
            }
            _ => {}
        }

        // FIFO A: 0xA0..=0xA3, FIFO B: 0xA4..=0xA7. 8/16/32-bit stores push bytes
        // (low byte first). A full word at A0/A4 uses the atomic 4-byte push.
        if size == 4 && off == 0xA0 {
            self.apu.fifo_a.push_word(value);
        } else if size == 4 && off == 0xA4 {
            self.apu.fifo_b.push_word(value);
        } else {
            for i in 0..size {
                let addr = off.wrapping_add(i);
                let byte = (value >> (8 * i)) as u8;
                if (0xA0..=0xA3).contains(&addr) {
                    self.apu.fifo_a.push_byte(byte);
                } else if (0xA4..=0xA7).contains(&addr) {
                    self.apu.fifo_b.push_byte(byte);
                }
            }
        }
    }

    fn apply_sound16(&mut self, off: u32, value: u16) {
        match off {
            0x60..=0x7E => self.apu.psg.write(off - 0x60, value),
            0x80 => self.apu.set_cnt_l(value),
            0x82 => self.apu.set_cnt_h(value),
            0x84 => self.apu.psg.write(0x20, value),
            0x88 => self.apu.set_bias(value),
            0x90..=0x9E => self.apu.psg.write(off - 0x60, value),
            _ => {}
        }
    }

    /// Live DMA summary line for `--debug`.
    pub fn dma_debug_line(&self, frame: u32) -> String {
        self.dma.debug_line(frame)
    }

    /// Run channel 1 or 2 if it is armed as FIFO special.
    pub fn request_fifo(&mut self, channel: usize) {
        if channel != 1 && channel != 2 {
            return;
        }
        if self.dma.reason(channel) != Some(StartReason::Fifo) {
            return;
        }
        self.dma_fire(channel, StartReason::Fifo);
    }

    /// Fire every channel armed for VBlank (one shot per rising edge).
    pub fn dma_on_vblank(&mut self) {
        for channel in 0..4 {
            if self.dma.reason(channel) == Some(StartReason::VBlank) {
                self.dma_fire(channel, StartReason::VBlank);
            }
        }
    }

    /// Fire every channel armed for HBlank (one shot per rising edge).
    pub fn dma_on_hblank(&mut self) {
        for channel in 0..4 {
            if self.dma.reason(channel) == Some(StartReason::HBlank) {
                self.dma_fire(channel, StartReason::HBlank);
            }
        }
    }

    fn dma_on_enable(&mut self, channel: usize) {
        if self.dma.busy {
            return;
        }
        let Some(reason) = self.dma.reason(channel) else {
            // Enable set with unsupported timing (DMA0/DMA3 special / video capture).
            if self.dma.cnt_h(channel) & (1 << 15) != 0 {
                self.dma.clear_enable(channel);
            }
            return;
        };
        self.dma.latch(channel);
        if let Err(line) = dma::region_access(channel, self.dma.sad(channel), self.dma.dad(channel))
        {
            self.warn_lines.push(line.to_string());
            self.dma.clear_enable(channel);
            return;
        }
        match reason {
            StartReason::Immediate => self.dma_fire(channel, StartReason::Immediate),
            StartReason::VBlank | StartReason::HBlank | StartReason::Fifo => {}
        }
    }

    fn dma_fire(&mut self, channel: usize, reason: StartReason) {
        if self.dma.busy {
            return;
        }
        let job = match reason {
            StartReason::Fifo => self.dma.fifo_job(channel),
            _ => self.dma.job(channel),
        };
        if let Err(line) = dma::region_access(channel, job.src, job.dst) {
            self.warn_lines.push(line.to_string());
            self.dma.clear_enable(channel);
            return;
        }
        let width32 = job.width32;
        self.dma.busy = true;
        let units = self.dma_copy(&job);
        self.dma.busy = false;
        if let Some(mask) = self.dma.finish(channel, reason, units, width32) {
            self.irq.raise(mask);
        }
    }

    fn dma_copy(&mut self, job: &dma::Copy) -> u32 {
        let this = self as *mut Bus;
        let width32 = job.width32;
        let mut read = |addr: u32| -> u32 {
            // Safety: `dma.busy` blocks nested `dma_fire` / `dma_on_enable` from
            // starting another copy, so these closures never re-enter `dma_copy`.
            unsafe {
                if width32 {
                    (*this).read32(addr)
                } else {
                    u32::from((*this).read16(addr))
                }
            }
        };
        let mut write = |addr: u32, value: u32| unsafe {
            if width32 {
                (*this).write32(addr, value);
            } else {
                (*this).write16(addr, value as u16);
            }
        };
        let units = dma::run_copy(job, &mut read, &mut write);
        if self.save_kind == SaveKind::Eeprom
            && (Self::eeprom_region(job.src) || Self::eeprom_region(job.dst))
        {
            if let SaveChip::Eeprom(eeprom) = &mut self.save {
                eeprom.end_transfer();
            }
        }
        units
    }

    fn load_timer_io(&self, off: u32, size: u32) -> u32 {
        let base = off.wrapping_sub(0x100);
        match size {
            1 => {
                let half = self.timers.read16(base & !1);
                u32::from(if base & 1 != 0 {
                    half >> 8
                } else {
                    half & 0xFF
                })
            }
            2 => u32::from(self.timers.read16(base)),
            4 => {
                let lo = u32::from(self.timers.read16(base));
                let hi = u32::from(self.timers.read16(base.wrapping_add(2)));
                lo | (hi << 16)
            }
            _ => 0,
        }
    }

    fn store_timer_io(&mut self, off: u32, value: u32, size: u32) {
        let base = off.wrapping_sub(0x100);
        match size {
            1 => {
                let aligned = base & !1;
                let cur = if aligned & 2 == 0 {
                    // CNT_L: merge into the reload latch, not the live counter.
                    self.timers.reload((aligned / 4) as usize)
                } else {
                    self.timers.read16(aligned)
                };
                let next = if base & 1 != 0 {
                    (cur & 0x00FF) | (((value as u16) & 0xFF) << 8)
                } else {
                    (cur & 0xFF00) | ((value as u16) & 0xFF)
                };
                self.timers.write16(aligned, next);
            }
            2 => self.timers.write16(base, value as u16),
            4 => {
                self.timers.write16(base, value as u16);
                self.timers
                    .write16(base.wrapping_add(2), (value >> 16) as u16);
            }
            _ => {}
        }
    }

    fn load_key_io(&self, off: u32, size: u32) -> u32 {
        let input = self.keypad.read_input();
        let cnt = self.keypad.read_cnt();
        match size {
            1 => match off {
                0x130 => u32::from(input & 0xFF),
                0x131 => u32::from(input >> 8),
                0x132 => u32::from(cnt & 0xFF),
                0x133 => u32::from(cnt >> 8),
                _ => 0,
            },
            2 => match off {
                0x130 => u32::from(input),
                0x132 => u32::from(cnt),
                _ => 0,
            },
            4 if off == 0x130 => u32::from(input) | (u32::from(cnt) << 16),
            _ => 0,
        }
    }

    fn store_key_io(&mut self, off: u32, value: u32, size: u32) {
        // Only KEYCNT (0x132) is writable.
        match size {
            1 => {
                if off == 0x132 {
                    let cur = self.keypad.read_cnt();
                    self.keypad
                        .write_cnt((cur & 0xFF00) | ((value as u16) & 0xFF));
                } else if off == 0x133 {
                    let cur = self.keypad.read_cnt();
                    self.keypad
                        .write_cnt((cur & 0x00FF) | (((value as u16) & 0xFF) << 8));
                }
            }
            2 if off == 0x132 => self.keypad.write_cnt(value as u16),
            4 if off == 0x130 => self.keypad.write_cnt((value >> 16) as u16),
            _ => {}
        }
    }

    fn load_irq_io(&self, off: u32, size: u32) -> u32 {
        let ie = self.irq.read16(0);
        let iff = self.irq.read16(2);
        let ime = self.irq.read16(8);
        match size {
            1 => match off {
                0x200 => u32::from(ie & 0xFF),
                0x201 => u32::from(ie >> 8),
                0x202 => u32::from(iff & 0xFF),
                0x203 => u32::from(iff >> 8),
                0x208 => u32::from(ime & 0xFF),
                0x209 => u32::from(ime >> 8),
                _ => 0,
            },
            2 => match off {
                0x200 => u32::from(ie),
                0x202 => u32::from(iff),
                0x208 => u32::from(ime),
                _ => 0,
            },
            4 if off == 0x200 => u32::from(ie) | (u32::from(iff) << 16),
            4 if off == 0x208 => u32::from(ime),
            _ => 0,
        }
    }

    fn store_irq_io(&mut self, off: u32, value: u32, size: u32) {
        match size {
            1 => match off {
                0x200 => {
                    let cur = self.irq.read16(0);
                    self.irq
                        .write16(0, (cur & 0xFF00) | ((value as u16) & 0xFF));
                }
                0x201 => {
                    let cur = self.irq.read16(0);
                    self.irq
                        .write16(0, (cur & 0x00FF) | (((value as u16) & 0xFF) << 8));
                }
                0x202 => {
                    // IF acknowledge: only the written low bits clear.
                    self.irq.write16(2, (value as u16) & 0xFF);
                }
                0x203 => {
                    self.irq.write16(2, ((value as u16) & 0xFF) << 8);
                }
                0x208 => self.irq.write16(8, (value as u16) & 0xFF),
                0x209 => {}
                _ => {}
            },
            2 => match off {
                0x200 => self.irq.write16(0, value as u16),
                0x202 => self.irq.write16(2, value as u16),
                0x208 => self.irq.write16(8, value as u16),
                _ => {}
            },
            4 if off == 0x200 => {
                self.irq.write16(0, value as u16);
                self.irq.write16(2, (value >> 16) as u16);
            }
            4 if off == 0x208 => self.irq.write16(8, value as u16),
            _ => {}
        }
    }

    fn load_rom(&self, addr: u32, size: u32) -> u32 {
        let off = (addr & 0x01FF_FFFF) as usize;
        if self.rom.is_empty() {
            return 0;
        }
        if off >= self.rom.len() {
            return rom_past_end(addr, size);
        }
        slice_load(&self.rom, off, size)
    }

    /// Last BIOS opcode word returned for BIOS *data* reads (GBATEK prefetch latch).
    pub fn bios_prefetch(&self) -> u32 {
        self.bios_prefetch
    }

    /// Update the BIOS prefetch latch (HLE SWI / IRQ paths).
    pub fn set_bios_prefetch(&mut self, word: u32) {
        self.bios_prefetch = word;
    }

    fn latch(&mut self, value: u32) {
        self.last_data = value;
    }

    fn openbus(&mut self, addr: u32, size: u32, region: &str) -> u32 {
        self.log_openbus(addr, region);
        self.open_slice(self.last_data, addr, size)
    }

    fn open_slice(&self, word: u32, addr: u32, size: u32) -> u32 {
        match size {
            1 => (word >> (8 * (addr & 3))) & 0xFF,
            2 => (word >> (8 * (addr & 2))) & 0xFFFF,
            _ => word,
        }
    }

    fn log_openbus(&mut self, addr: u32, region: &str) {
        if !self.openbus_logged.insert(addr) {
            return;
        }
        self.warn_lines.push(format!(
            "gba-debug: warn openbus addr={addr:#010X} region={region}"
        ));
    }

    fn warn_sio(&mut self) {
        if self.sio_warned {
            return;
        }
        self.sio_warned = true;
        self.warn_lines
            .push("gba-debug: warn sio unlinked".to_string());
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    Fetch,
    Data,
}

/// Align halfword/word data accesses, except the 8-bit SRAM/flash bus where the
/// unaligned address selects which lane is written (jsmolka save tests).
fn align_data(addr: u32, size: u32) -> u32 {
    if matches!(addr >> 24, 0x0E | 0x0F) {
        return addr;
    }
    match size {
        2 => addr & !1,
        4 => addr & !3,
        _ => addr,
    }
}

fn covers_byte(off: u32, size: u32, byte: u32) -> bool {
    off <= byte && off.saturating_add(size) > byte
}

fn sound_covers(off: u32, size: u32) -> bool {
    for i in 0..size {
        let addr = off.wrapping_add(i);
        if (0x60..=0x7E).contains(&addr)
            || addr == 0x80
            || addr == 0x81
            || addr == 0x82
            || addr == 0x83
            || addr == 0x84
            || addr == 0x85
            || addr == 0x88
            || addr == 0x89
            || (0x90..=0x9F).contains(&addr)
            || (0xA0..=0xA7).contains(&addr)
        {
            return true;
        }
    }
    false
}

fn sio_touches(off: u32, size: u32) -> bool {
    let start = off;
    let end = off.saturating_add(size.saturating_sub(1));
    // SIODATA32 / SIOMULTI0–3 / SIOCNT / SIOMLT_SEND / SIODATA8: 0x120–0x12B
    // RCNT: 0x134–0x135
    ranges_overlap(start, end, 0x120, 0x12B) || ranges_overlap(start, end, 0x134, 0x135)
}

fn ranges_overlap(a0: u32, a1: u32, b0: u32, b1: u32) -> bool {
    a0 <= b1 && b0 <= a1
}

fn vram_off(addr: u32) -> usize {
    let off = (addr & 0x1_FFFF) as usize;
    if off < VRAM {
        off
    } else {
        off - 0x8000
    }
}

/// Past-end Game Pak reads: each halfword is `((addr >> 1) & 0xFFFF)`.
fn rom_past_end(addr: u32, size: u32) -> u32 {
    let half = |a: u32| (a >> 1) & 0xFFFF;
    match size {
        1 => {
            let hw = half(addr & !1);
            (hw >> (8 * (addr & 1))) & 0xFF
        }
        2 => half(addr),
        4 => {
            let base = addr & !3;
            let low = half(base);
            let high = half(base.wrapping_add(2));
            low | (high << 16)
        }
        _ => 0,
    }
}

fn slice_load(mem: &[u8], off: usize, size: u32) -> u32 {
    let mut out = 0u32;
    for i in 0..size as usize {
        let byte = mem.get(off.wrapping_add(i)).copied().unwrap_or(0);
        out |= (byte as u32) << (8 * i);
    }
    out
}

fn slice_store(mem: &mut [u8], off: usize, value: u32, size: u32) {
    for i in 0..size as usize {
        if let Some(slot) = mem.get_mut(off.wrapping_add(i)) {
            *slot = (value >> (8 * i)) as u8;
        }
    }
}

#[cfg(test)]
mod tests;
