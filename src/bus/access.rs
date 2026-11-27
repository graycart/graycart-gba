//! CPU reads and writes across the memory map.
//!
//! Cited: GBATEK, GBA Memory Map. https://problemkaputt.de/gbatek.htm

use super::map::{
    Access, align_data, rom_past_end, slice_load, slice_store, vram_off, width_bytes,
};
use super::{BIOS, Bus, IO, SaveChip};
use crate::cart::SaveKind;
use crate::timing::{Width, internal_cycles, rom_cycles, sram_cycles};

impl Bus {
    pub fn fetch16(&mut self, addr: u32) -> u16 {
        let addr = addr & !1;
        self.charge(addr, Width::Half, Access::Fetch);
        let bits = self.access(addr, 2, Access::Fetch);
        bits as u16
    }

    pub fn fetch32(&mut self, addr: u32) -> u32 {
        let addr = addr & !3;
        self.charge(addr, Width::Word, Access::Fetch);
        let instr = self.access(addr, 4, Access::Fetch);
        // GBATEK (Unpredictable Things): ARM open bus is the pipelined prefetch
        // WORD = [$+8], not the opcode at PC. Instruction fetch must leave that
        // word in the CPU MDR so unused 16-bit data reads (which do not refresh
        // the bus) still expose [$+8] to deferred 32-bit unused-I/O DMA.
        if addr >> 24 != 0 {
            let pref = self.load_arm_prefetch(addr.wrapping_add(8));
            self.latch(pref);
        }
        instr
    }

    pub fn read8(&mut self, addr: u32) -> u8 {
        let run_pending = self.any_imm_pending();
        self.charge(addr, Width::Byte, Access::Data);
        let value = self.access(addr, 1, Access::Data).to_le_bytes()[0];
        if run_pending {
            self.fire_pending_imm();
        }
        value
    }

    pub fn read16(&mut self, addr: u32) -> u16 {
        let addr = align_data(addr, 2);
        let run_pending = self.any_imm_pending();
        self.charge(addr, Width::Half, Access::Data);
        let bits = self.access(addr, 2, Access::Data);
        if run_pending {
            self.fire_pending_imm();
        }
        bits as u16
    }

    pub fn read32(&mut self, addr: u32) -> u32 {
        let addr = align_data(addr, 4);
        let run_pending = self.any_imm_pending();
        self.charge(addr, Width::Word, Access::Data);
        let value = self.access(addr, 4, Access::Data);
        if run_pending {
            self.fire_pending_imm();
        }
        value
    }

    pub fn write8(&mut self, addr: u32, value: u8) {
        let run_pending = self.any_imm_pending();
        self.charge(addr, Width::Byte, Access::Data);
        self.store(addr, value as u32, 1);
        self.latch(value as u32);
        if run_pending {
            self.fire_pending_imm();
        }
    }

    pub fn write16(&mut self, addr: u32, value: u16) {
        let addr = align_data(addr, 2);
        let run_pending = self.any_imm_pending();
        self.charge(addr, Width::Half, Access::Data);
        self.store(addr, value as u32, 2);
        self.latch(value as u32);
        if run_pending {
            self.fire_pending_imm();
        }
    }

    /// Store a word without waitstates. Empty Thumb PUSH writes PC in the fetch cycle.
    pub fn poke32(&mut self, addr: u32, value: u32) {
        self.store(addr, value, 4);
    }

    pub fn write32(&mut self, addr: u32, value: u32) {
        let addr = align_data(addr, 4);
        let run_pending = self.any_imm_pending();
        self.charge(addr, Width::Word, Access::Data);
        self.store(addr, value, 4);
        self.latch(value);
        if run_pending {
            self.fire_pending_imm();
        }
    }

    /// Charge waitstates for one CPU access. Skipped while DMA owns the bus.
    fn charge(&mut self, addr: u32, width: Width, kind: Access) {
        if self.dma.busy {
            return;
        }

        let waitcnt = self.waitcnt();
        let region = addr >> 24;

        if (0x08..=0x0D).contains(&region) {
            self.step_pak = true;
            if kind == Access::Data {
                self.prefetch.invalidate();
            }

            let sequential = !self.prefetch_ns
                && addr & 0x1_FFFF != 0
                && match self.last_rom {
                    Some((prev, prev_w)) => addr == prev.wrapping_add(prev_w),
                    None => false,
                };
            let width_bytes = width_bytes(width);

            let cycles = if kind == Access::Fetch && waitcnt & (1 << 14) != 0 {
                let hit = match width {
                    Width::Byte | Width::Half => self.prefetch.take(addr),
                    Width::Word => self.prefetch.take_n(addr, 2),
                };
                if hit {
                    self.last_rom = Some((addr, width_bytes));
                    self.step_cycles = self.step_cycles.saturating_add(1);
                    return;
                }
                rom_cycles(waitcnt, addr, width, sequential)
            } else {
                rom_cycles(waitcnt, addr, width, sequential)
            };

            self.step_cycles = self.step_cycles.saturating_add(cycles);
            self.last_rom = Some((addr, width_bytes));
            return;
        }

        if region == 0x0E || region == 0x0F {
            self.step_pak = true;
            self.step_cycles = self.step_cycles.saturating_add(sram_cycles(waitcnt));
            return;
        }

        let internal = internal_cycles(addr, width);
        self.cart_idle = self.cart_idle.saturating_add(internal);
        self.step_cycles = self.step_cycles.saturating_add(internal);
        // Game Pak sequential burst ends after any non-cart access (I/O, etc.).
        // IWRAM/EWRAM are excluded so address-based N/S tests keep alyosha's rule;
        // I/O and OAM still break the cart burst (GBATEK waitstate chapter).
        if region == 0x04 || region == 0x07 {
            self.last_rom = None;
        }
    }

    pub(super) fn access(&mut self, addr: u32, size: u32, kind: Access) -> u32 {
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
                let off = (addr & 0x7FFF) as usize;
                if let Some(v) = self.load_intr_check(off, size) {
                    self.latch(v);
                    v
                } else {
                    let word = self.load_iwram_bus(addr, size);
                    self.latch(word);
                    self.open_slice(word, addr, size)
                }
            }
            0x04 => {
                let off = addr & 0x00FF_FFFF;
                if off == 0x800 {
                    let value = u32::from(self.cgb_romdis) << 3;
                    self.latch(value);
                    return value;
                }
                if (0x00FF_F600..=0x00FF_F801).contains(&off) {
                    let value = self.load_mgba_debug(off, size);
                    self.latch(value);
                    return value;
                }
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
                // OAM is a 32-bit bus: any access latches the aligned word (ares/GBATEK).
                let word = slice_load(&self.oam, (addr & 0x3FC) as usize, 4);
                self.latch(word);
                self.open_slice(word, addr, size)
            }
            0x08..=0x0C => {
                if let Some(value) = self.read_gpio(addr, size) {
                    self.latch(value);
                    return value;
                }
                if size == 1 {
                    // Game Pak data bus is 16-bit. A byte read still latches the
                    // aligned halfword in both halves (alyosha LDRSH_misaligned).
                    let half = self.load_rom(addr & !1, 2) & 0xFFFF;
                    let word = half | (half << 16);
                    self.latch(word);
                    return (word >> ((addr & 1) * 8)) & 0xFF;
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

    pub(super) fn store(&mut self, addr: u32, value: u32, size: u32) {
        match addr >> 24 {
            0x02 => slice_store(&mut self.ewram, (addr & 0x3_FFFF) as usize, value, size),
            0x03 => {
                let off = (addr & 0x7FFF) as usize;
                if self.store_intr_check(off, value, size) {
                    return;
                }
                self.store_iwram_bus(addr, value, size);
            }
            0x04 => {
                let off = addr & 0x00FF_FFFF;
                if off == 0x800 {
                    self.cgb_romdis = value & 8 != 0;
                    return;
                }
                // mGBA debug port (libgba / tonc printf). Outside the 1 KiB I/O window.
                if (0x00FF_F600..=0x00FF_F801).contains(&off) {
                    self.store_mgba_debug(off, value, size);
                    return;
                }
                if off >= IO as u32 {
                    return;
                }
                self.store_io(off, value, size);
            }
            0x05 => self.store_pal(addr, value, size),
            0x06 => self.store_vram(addr, value, size),
            0x07 => self.store_oam(addr, value, size),
            0x08..=0x0C => {
                self.write_gpio(addr, value);
            }
            0x0D => {
                if self.save_kind == SaveKind::Eeprom
                    && size == 2
                    && let SaveChip::Eeprom(eeprom) = &mut self.save
                {
                    eeprom.write_bit(value as u16);
                    self.save_dirty = true;
                }
            }
            0x0E | 0x0F => self.store_save(addr, value, size),
            _ => {}
        }
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

    /// Quiet code read for the ARM7 prefetch queue. No waitstates and no bus latch.
    pub(crate) fn peek_code(&self, addr: u32, size: u32) -> Option<u32> {
        let addr = addr & !(size.max(1) - 1);
        let region = addr >> 24;
        let value = match region {
            0x02 => slice_load(&self.ewram, (addr & 0x3_FFFF) as usize, size),
            0x03 => slice_load(&self.iwram, (addr & 0x7FFF) as usize, size),
            0x06 => slice_load(&self.vram, vram_off(addr), size),
            0x08..=0x0D => self.load_rom(addr, size),
            _ => return None,
        };
        Some(value)
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

    /// Quiet 32-bit load for ARM pipeline prefetch (no waitstates, no MDR/DMA side effects).
    fn load_arm_prefetch(&self, addr: u32) -> u32 {
        let addr = addr & !3;
        match addr >> 24 {
            0x02 => slice_load(&self.ewram, (addr & 0x3_FFFF) as usize, 4),
            0x03 => slice_load(&self.iwram, (addr & 0x7FFF) as usize, 4),
            0x05 => slice_load(&self.pal, (addr & 0x3FF) as usize, 4),
            0x06 => slice_load(&self.vram, vram_off(addr), 4),
            0x07 => slice_load(&self.oam, (addr & 0x3FC) as usize, 4),
            0x08..=0x0D => self.load_rom(addr, 4),
            _ => self.last_data,
        }
    }

    /// Last BIOS opcode word returned for BIOS *data* reads (GBATEK prefetch latch).
    pub fn bios_prefetch(&self) -> u32 {
        self.bios_prefetch
    }

    /// Update the BIOS prefetch latch (HLE SWI / IRQ paths).
    pub fn set_bios_prefetch(&mut self, word: u32) {
        self.bios_prefetch = word;
    }

    pub(super) fn latch(&mut self, value: u32) {
        self.last_data = value;
    }

    pub(super) fn openbus(&mut self, addr: u32, size: u32, region: &str) -> u32 {
        // Open-bus data reads force any delayed immediate DMA first so the CPU
        // sees the DMA latch (alyosha DMA_CPU_Bus_Interaction).
        if !self.dma.busy {
            self.fire_pending_imm();
        }
        self.log_openbus(addr, region);
        // During DMA, and for the first CPU open-bus data read after DMA, the
        // DMA latch is what sits on the bus (alyosha Bus/ReadMe; mGBA `bus`).
        let word = if self.dma.busy || self.dma_open_cpu {
            self.dma_open
        } else {
            self.last_data
        };
        let value = self.open_slice(word, addr, size);
        if self.dma_open_cpu && !self.dma.busy {
            self.dma_open_cpu = false;
        }
        value
    }

    /// IWRAM bus: word accesses replace the latch; halfword merges one half (ares).
    fn load_iwram_bus(&mut self, addr: u32, size: u32) -> u32 {
        let off = (addr & 0x7FFF) as usize;
        match size {
            4 => {
                let value = slice_load(&self.iwram, off & !3, 4);
                self.iwram_bus = value;
                value
            }
            2 => {
                let half = slice_load(&self.iwram, off & !1, 2) as u16;
                if addr & 2 != 0 {
                    self.iwram_bus = (self.iwram_bus & 0x0000_FFFF) | (u32::from(half) << 16);
                } else {
                    self.iwram_bus = (self.iwram_bus & 0xFFFF_0000) | u32::from(half);
                }
                self.iwram_bus
            }
            1 => {
                let byte = slice_load(&self.iwram, off, 1) as u8;
                let shift = (addr & 3) * 8;
                self.iwram_bus = (self.iwram_bus & !(0xFF << shift)) | (u32::from(byte) << shift);
                self.iwram_bus
            }
            _ => 0,
        }
    }

    fn store_iwram_bus(&mut self, addr: u32, value: u32, size: u32) {
        let off = (addr & 0x7FFF) as usize;
        match size {
            4 => {
                self.iwram_bus = value;
                slice_store(&mut self.iwram, off & !3, value, 4);
            }
            2 => {
                let half = value as u16;
                if addr & 2 != 0 {
                    self.iwram_bus = (self.iwram_bus & 0x0000_FFFF) | (u32::from(half) << 16);
                } else {
                    self.iwram_bus = (self.iwram_bus & 0xFFFF_0000) | u32::from(half);
                }
                slice_store(&mut self.iwram, off & !1, u32::from(half), 2);
            }
            1 => {
                let byte = value as u8;
                let shift = (addr & 3) * 8;
                self.iwram_bus = (self.iwram_bus & !(0xFF << shift)) | (u32::from(byte) << shift);
                slice_store(&mut self.iwram, off, u32::from(byte), 1);
            }
            _ => {}
        }
    }

    fn open_slice(&self, word: u32, addr: u32, size: u32) -> u32 {
        match size {
            1 => (word >> (8 * (addr & 3))) & 0xFF,
            2 => (word >> (8 * (addr & 2))) & 0xFFFF,
            _ => word,
        }
    }

    pub(super) fn log_openbus(&mut self, addr: u32, region: &str) {
        if !self.openbus_logged.insert(addr) {
            return;
        }
        self.warn_lines.push(format!(
            "gba-debug: warn openbus addr={addr:#010X} region={region}"
        ));
    }
}
