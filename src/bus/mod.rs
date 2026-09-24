//! Memory map for the pages that can run a ROM.
//!
//! Cited: GBATEK, GBA Memory Map. https://problemkaputt.de/gbatek.htm

use std::collections::HashSet;

use crate::apu::Apu;
use crate::cart::{Eeprom, Flash, FlashSize, SaveKind, detect_save, gpio_reject_line};
use crate::dma::{self, Dma, StartReason};
use crate::input::Keypad;
use crate::irq::Irq;
use crate::timer::Timers;
use crate::timing::{Prefetch, Width, internal_cycles, rom_cycles, sram_cycles, ws0_ns};

const EWRAM: usize = 256 * 1024;
const IWRAM: usize = 32 * 1024;
const PAL: usize = 1024;
const VRAM: usize = 96 * 1024;
const OAM: usize = 1024;
const IO: usize = 0x400;
const SRAM: usize = 64 * 1024;
const BIOS: usize = 16 * 1024;

/// Cartridge backup chip held on the bus.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
enum SaveChip {
    None,
    Sram(Vec<u8>),
    Flash(Flash),
    Eeprom(Eeprom),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    /// Last DMA data-bus word. Unused/invalid DMA reads keep this latch
    /// (alyosha Bus/ReadMe; GBATEK open bus). 16-bit units duplicate the halfword.
    #[serde(default = "dma_open_reset")]
    dma_open: u32,
    /// After a DMA unit, the next CPU open-bus data read returns [`Self::dma_open`]
    /// instead of the instruction prefetch latch (mGBA `bus` / dmaPC window).
    #[serde(default)]
    dma_open_cpu: bool,
    /// Immediate DMA armed but not yet fired (GBATEK/ares: startup delay so the
    /// next real data access can update the CPU MDR first; open-bus reads force fire).
    #[serde(default)]
    dma_imm_pending: [bool; 4],
    /// GBATEK/ares: two-cycle wait after Immediate enable before the transfer starts.
    /// Only used when [`Self::dma_timing`] is set (Machine runs).
    #[serde(skip)]
    dma_imm_wait: [u8; 4],
    /// Immediate channels whose startup wait just hit zero; Machine drains these.
    #[serde(skip)]
    dma_imm_ready: [bool; 4],
    /// IWRAM 32-bit data bus latch (halfword accesses merge into one half).
    #[serde(default)]
    iwram_bus: u32,
    bios_prefetch: u32,
    pub warn_lines: Vec<String>,
    openbus_logged: HashSet<u32>,
    sio_warned: bool,
    /// Bitmask of SWI numbers that already emitted a once-per-run warn line.
    #[serde(default)]
    swi_warned: u64,
    /// WS0 N/S and related costs mirrored from WAITCNT for the debug wait line.
    pub wait_rom_n: u32,
    pub wait_rom_s: u32,
    pub wait_rom_i: u32,
    pub wait_sram: u32,
    /// Game Pak opcode prefetch buffer.
    pub prefetch: Prefetch,
    /// Cycles charged for the current CPU instruction (not DMA).
    step_cycles: u32,
    /// True when this step touched ROM or SRAM (not internal-only).
    step_pak: bool,
    /// Previous Game Pak ROM access, for N/S sequential detection.
    last_rom: Option<(u32, u32)>,
    /// WAITCNT bit 15: cartridge-shape sense. A Game Boy shell reads as 1.
    gb_cart_shape: bool,
    /// Set when HALTCNT stop applies a prepared switch. ARM does not execute after this.
    gb_mode: bool,
    /// `0x04000800` bit 3. Disables the CGB boot ROM.
    cgb_romdis: bool,
    /// Set when save-chip contents change; cleared after a successful sidecar flush.
    /// Not part of the on-disk snapshot (always false after decode).
    #[serde(skip)]
    save_dirty: bool,
    /// Machine cycle clock mirrored for mid-DMA / probe (set by Machine each tick).
    #[serde(skip)]
    pub(crate) emu_cycles: u64,
    /// When true, each DMA read/write phase advances timers and scanline so a
    /// higher-priority channel can preempt between phases (GBATEK / alyosha pause).
    #[serde(skip)]
    pub(crate) dma_timing: bool,
    /// [`Machine`] cycle count at [`Self::begin_step_at`]; DMA phases add on top.
    #[serde(skip)]
    pub(crate) cycle_base: u64,
    /// Scanline / timer cycles already applied inside the current DMA (not in `stall`).
    #[serde(skip)]
    pub(crate) dma_cycles_paid: u32,
    /// Channel currently inside `dma_fire` (for priority nesting).
    #[serde(skip)]
    dma_active: Option<u8>,
    /// ares `writeCycle`: a unit read finished and its write is still owed. Higher
    /// priority DMA waits until the write completes (GBATEK channel priority).
    #[serde(skip)]
    dma_write_cycle: bool,
    /// 1 = source beat, 2 = destination beat.
    #[serde(skip)]
    dma_beat: u8,
    /// Unit currently running is the channel's last one.
    #[serde(skip)]
    dma_last_unit: bool,
    /// Next ROM source beat stays sequential after an internal preempt.
    #[serde(skip)]
    dma_rom_seq_resume: bool,
    /// HBlank DMA armed while `dma_write_cycle` — run after the pending write.
    #[serde(skip)]
    dma_hblank_deferred: bool,
    /// Deferred HBlank was armed during writeCycle (vs mid-read).
    #[serde(skip)]
    dma_hblank_from_write: bool,
    /// Absolute cycle that armed the deferred HBlank.
    #[serde(skip)]
    dma_hblank_defer_abs: u64,
    /// The active DMA has finished at least one unit write.
    #[serde(skip)]
    dma_unit_written: bool,
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
            // Floating DMA data bus until the first real DMA memory read.
            dma_open: 0xFFFF_FFFF,
            dma_open_cpu: false,
            dma_imm_pending: [false; 4],
            dma_imm_wait: [0; 4],
            dma_imm_ready: [false; 4],
            iwram_bus: 0,
            // Post-boot latch: last BIOS opcode word (at 0xE4), not a fetch of 0.
            bios_prefetch: 0xE129_F000,
            warn_lines: Vec::new(),
            openbus_logged: HashSet::new(),
            sio_warned: false,
            swi_warned: 0,
            // WAITCNT reset is 0 → WS0 N=4, S=2, I=1, SRAM=4.
            wait_rom_n: 4,
            wait_rom_s: 2,
            wait_rom_i: 1,
            wait_sram: 4,
            prefetch: Prefetch::new(),
            step_cycles: 0,
            step_pak: false,
            last_rom: None,
            gb_cart_shape: false,
            gb_mode: false,
            cgb_romdis: false,
            save_dirty: false,
            emu_cycles: 0,
            dma_timing: false,
            cycle_base: 0,
            dma_cycles_paid: 0,
            dma_active: None,
            dma_write_cycle: false,
            dma_beat: 0,
            dma_last_unit: false,
            dma_rom_seq_resume: false,
            dma_hblank_deferred: false,
            dma_hblank_from_write: false,
            dma_hblank_defer_abs: 0,
            dma_unit_written: false,
        }
    }

    /// Cart-shape sense for WAITCNT bit 15. Software writes cannot clear it.
    pub fn set_gb_cart_shape(&mut self, shape: bool) {
        self.gb_cart_shape = shape;
    }

    /// True after DISPCNT bit 3 was prepared and HALTCNT stop applied it.
    pub fn gb_mode(&self) -> bool {
        self.gb_mode
    }

    /// True when `0x04000800` bit 3 has disabled the CGB boot ROM.
    pub fn cgb_romdis(&self) -> bool {
        self.cgb_romdis
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
        self.save_dirty = false;
    }

    /// True when the save chip differs from the last successful sidecar flush.
    pub fn save_dirty(&self) -> bool {
        self.save_dirty
    }

    /// Clear the dirty flag after a successful flush.
    pub fn clear_save_dirty(&mut self) {
        self.save_dirty = false;
    }

    /// Mark save contents dirty (e.g. after restoring a slot snapshot).
    pub fn mark_save_dirty(&mut self) {
        if self.save_kind != SaveKind::None {
            self.save_dirty = true;
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
                if b.is_empty() { None } else { Some(b) }
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

    /// Once per SWI number per machine: music / unsupported HLE stubs.
    pub fn warn_swi_once(&mut self, number: u32, kind: &str) {
        let bit = 1u64 << (number & 63);
        if self.swi_warned & bit != 0 {
            return;
        }
        self.swi_warned |= bit;
        self.warn_lines
            .push(format!("gba-debug: warn swi {kind} n={number:x}"));
    }

    pub fn wait_line(&self, frame: u32, stall: u64) -> String {
        format!(
            "gba-debug: wait frame={frame} rom_n={} rom_s={} rom_i={} sram={} stall={stall} prefetch={}",
            self.wait_rom_n,
            self.wait_rom_s,
            self.wait_rom_i,
            self.wait_sram,
            self.prefetch.hits()
        )
    }

    /// ARM B/BL/BX pipeline refill: opcode fetch already paid 1 bus cycle; pay the
    /// remaining 1N (ARM 2S+1N total with the next step's sequential fetch) and leave
    /// `last_rom` so the next fetch at `target` is sequential (GBATEK).
    pub fn arm_branch_refill(&mut self, target: u32) {
        if self.dma.busy {
            return;
        }
        let t = target & !3;
        let region = t >> 24;
        if (0x08..=0x0D).contains(&region) {
            self.step_pak = true;
            let waitcnt = self.waitcnt();
            // Remaining non-sequential word of the refill; next CPU fetch pays S.
            let n = rom_cycles(waitcnt, t, Width::Word, false);
            self.step_cycles = self.step_cycles.saturating_add(n);
            self.last_rom = Some((t.wrapping_sub(4), 4));
        } else {
            // Internal 32-bit bus: remaining 1N after the opcode fetch.
            self.step_cycles = self.step_cycles.saturating_add(1);
            self.last_rom = None;
        }
    }

    /// Thumb taken B/Bcond pipeline refill: opcode fetch already paid 1 bus cycle;
    /// pay the remaining 1N+1S (Thumb 2S+1N total) and leave `last_rom` so the next
    /// fetch at `target` is sequential. The following opcode fetch is still charged
    /// as S; on Game Pak with S=1 that matches one filled pipeline slot (GBATEK).
    pub fn thumb_branch_refill(&mut self, target: u32) {
        if self.dma.busy {
            return;
        }
        let t = target & !1;
        let region = t >> 24;
        if (0x08..=0x0D).contains(&region) {
            self.step_pak = true;
            let waitcnt = self.waitcnt();
            let n = rom_cycles(waitcnt, t, Width::Half, false);
            let s = rom_cycles(waitcnt, t.wrapping_add(2), Width::Half, true);
            self.step_cycles = self.step_cycles.saturating_add(n.saturating_add(s));
            self.last_rom = Some((t.wrapping_sub(2), 2));
        } else {
            self.step_cycles = self.step_cycles.saturating_add(2);
            self.last_rom = None;
        }
    }

    /// Clear per-instruction wait accounting before [`Cpu::step`](crate::cpu::Cpu::step).
    pub fn begin_step(&mut self) {
        self.begin_step_at(0);
    }

    /// Like [`Self::begin_step`], and record the machine cycle clock for mid-DMA ticks.
    pub fn begin_step_at(&mut self, cycles: u64) {
        self.step_cycles = 0;
        self.step_pak = false;
        self.cycle_base = cycles;
        self.dma_cycles_paid = 0;
    }

    /// Cycles already applied inside DMA during the last step (timers/scanline updated).
    pub fn take_dma_cycles_paid(&mut self) -> u32 {
        let n = self.dma_cycles_paid;
        self.dma_cycles_paid = 0;
        n
    }

    /// After an internal-only instruction, fill the prefetch buffer during those cycles.
    /// Only seeds Game Pak PCs so IWRAM/BIOS execution does not pollute the buffer.
    pub fn finish_step(&mut self, next_opcode: u32) {
        if self.step_pak || self.step_cycles == 0 {
            return;
        }
        let region = next_opcode >> 24;
        if !(0x08..=0x0D).contains(&region) {
            return;
        }
        let waitcnt = self.waitcnt();
        let s = rom_cycles(waitcnt, next_opcode, Width::Half, true);
        self.prefetch.idle(self.step_cycles, next_opcode, s);
    }

    /// Cycles charged for the instruction just stepped (at least 1 for the machine).
    pub fn take_step_cycles(&mut self) -> u32 {
        let n = self.step_cycles.max(1);
        self.step_cycles = 0;
        n
    }

    /// WAITCNT halfword at I/O 0x04000204.
    pub fn waitcnt(&self) -> u16 {
        let stored = slice_load(&self.io, 0x204, 2) as u16;
        if self.gb_cart_shape {
            stored | 0x8000
        } else {
            stored & !0x8000
        }
    }

    fn sync_waitcnt(&mut self) {
        let waitcnt = self.waitcnt();
        let (n, s) = ws0_ns(waitcnt);
        self.wait_rom_n = n;
        self.wait_rom_s = s;
        self.wait_rom_i = 1;
        self.wait_sram = sram_cycles(waitcnt);
        self.prefetch.set_enabled(waitcnt & (1 << 14) != 0);
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

            let sequential = match self.last_rom {
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

        self.step_cycles = self
            .step_cycles
            .saturating_add(internal_cycles(addr, width));
        // Game Pak sequential burst ends after any non-cart access (I/O, etc.).
        // IWRAM/EWRAM are excluded so address-based N/S tests keep alyosha's rule;
        // I/O and OAM still break the cart burst (GBATEK waitstate chapter).
        if region == 0x04 || region == 0x07 {
            self.last_rom = None;
        }
    }

    /// Add bare internal cycles (ARM LDR trailing I, etc.).
    pub fn add_internal_cycles(&mut self, n: u32) {
        if self.dma.busy {
            return;
        }
        self.step_cycles = self.step_cycles.saturating_add(n);
    }

    /// Advance timers / Immediate DMA wait mid-instruction (SWI CpuSet, STRH trailing N).
    ///
    /// Unlike [`Self::add_internal_cycles`], this updates timer state immediately so a
    /// later op that enables DMA sees the correct TIM* counters before the instruction
    /// ends. Not added to `step_cycles` (Machine must not double-tick timers).
    pub fn elapse(&mut self, n: u32) {
        if self.dma.busy || n == 0 {
            return;
        }
        for _ in 0..n {
            self.emu_cycles = self.emu_cycles.wrapping_add(1);
            self.tick_imm_dma_wait();
            let mask = self.timers.tick(1);
            self.tick_apu(mask);
            if self.any_imm_ready() {
                self.cycle_base = self.emu_cycles;
                self.dma_cycles_paid = 0;
                self.fire_ready_imm();
                let paid = self.take_dma_cycles_paid();
                self.emu_cycles = self.emu_cycles.wrapping_add(u64::from(paid));
            }
        }
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
                if self.touch_gpio(addr) {
                    let value = 0;
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

    fn store(&mut self, addr: u32, value: u32, size: u32) {
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

    /// BIOS IntrWait check flags live at `0x03007FF8` and in [`Irq::check_flags`].
    fn load_intr_check(&self, off: usize, size: u32) -> Option<u32> {
        let flags = u32::from(self.irq.check_flags());
        match (off, size) {
            (0x7FF8, 1) => Some(flags & 0xFF),
            (0x7FF9, 1) => Some((flags >> 8) & 0xFF),
            (0x7FF8, 2) => Some(flags),
            (0x7FF8, 4) => {
                let hi = slice_load(&self.iwram, 0x7FFA, 2);
                Some(flags | (hi << 16))
            }
            (0x7FF6, 4) => {
                let lo = slice_load(&self.iwram, 0x7FF6, 2);
                Some(lo | (flags << 16))
            }
            _ => None,
        }
    }

    fn store_intr_check(&mut self, off: usize, value: u32, size: u32) -> bool {
        match (off, size) {
            (0x7FF8, 1) => {
                let next = (self.irq.check_flags() & 0xFF00) | (value as u16 & 0xFF);
                self.irq.set_check_flags(next);
                true
            }
            (0x7FF9, 1) => {
                let next = (self.irq.check_flags() & 0x00FF) | ((value as u16 & 0xFF) << 8);
                self.irq.set_check_flags(next);
                true
            }
            (0x7FF8, 2) => {
                self.irq.set_check_flags(value as u16);
                true
            }
            (0x7FF8, 4) => {
                self.irq.set_check_flags(value as u16);
                slice_store(&mut self.iwram, 0x7FFA, value >> 16, 2);
                true
            }
            (0x7FF6, 4) => {
                slice_store(&mut self.iwram, 0x7FF6, value & 0xFFFF, 2);
                self.irq.set_check_flags((value >> 16) as u16);
                true
            }
            _ => false,
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
                self.save_dirty = true;
            }
            SaveChip::Flash(flash) => {
                flash.write(addr, value);
                self.save_dirty = true;
            }
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
        // WAITCNT bit 15 is the cart-shape sense, not a bit software stored.
        if self.gb_cart_shape && covers_byte(off, size, 0x205) {
            let shift = (0x205 - off) * 8;
            raw |= 0x80 << shift;
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
        // WAITCNT bit 15 is read-only (cart shape).
        if covers_byte(off, size, 0x205) {
            let shift = (0x205 - off) * 8;
            value &= !(0x80 << shift);
        }
        slice_store(&mut self.io, off as usize, value, size);
        if covers_byte(off, size, 0x204) || covers_byte(off, size, 0x205) {
            self.sync_waitcnt();
        }
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
            match self.dma.store(off, value, size) {
                Some(dma::CntHWrite::EnableRise(channel)) => {
                    self.dma_on_enable(channel, true);
                }
                Some(dma::CntHWrite::CtrlKeep(channel)) => {
                    self.dma_on_enable(channel, false);
                }
                Some(dma::CntHWrite::EnableFall(channel)) => {
                    self.dma_imm_pending[channel] = false;
                    self.dma_imm_wait[channel] = 0;
                    self.dma_imm_ready[channel] = false;
                }
                None => {}
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
        // HALTCNT: byte 0 halts. Byte bit 7 (stop) applies a prepared Game Boy switch.
        if off == 0x301 && size == 1 {
            let byte = value as u8;
            let prepared = self.dispcnt() & 8 != 0 && self.gb_cart_shape;
            if byte & 0x80 != 0 && prepared {
                self.gb_mode = true;
            } else if byte == 0 {
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
        // ares: finish the active unit before a higher-priority read.
        if self.dma_write_cycle {
            // Destination beat of a unit that still has another after it: the
            // higher channel reads the timer on this edge, before the beat's tick.
            // The last unit keeps the deferred write so its startup still runs.
            if self.dma_beat == 2 && self.dma_unit_written && !self.dma_last_unit {
                self.dma_hblank_from_write = false;
                for channel in 0..4 {
                    if self.dma.reason(channel) == Some(StartReason::HBlank) {
                        self.dma_fire(channel, StartReason::HBlank);
                    }
                }
                return;
            }
            self.dma_hblank_deferred = true;
            self.dma_hblank_from_write = true;
            self.dma_hblank_defer_abs = self.dma_abs();
            return;
        }
        if self.dma_active.is_some() && !self.dma_unit_written {
            self.dma_hblank_deferred = true;
            self.dma_hblank_from_write = false;
            self.dma_hblank_defer_abs = self.dma_abs();
            return;
        }
        if self.dma_active.is_some() {
            // Later unit boundary: the higher channel reads before this cycle's timer tick.
            self.dma_hblank_from_write = false;
            for channel in 0..4 {
                if self.dma.reason(channel) == Some(StartReason::HBlank) {
                    self.dma_fire(channel, StartReason::HBlank);
                }
            }
            return;
        }
        for channel in 0..4 {
            if self.dma.reason(channel) == Some(StartReason::HBlank) {
                self.dma_fire(channel, StartReason::HBlank);
            }
        }
    }

    /// Fire DMA3 when armed for video capture (timing 3) on an in-window HBlank edge.
    ///
    /// Caller must only invoke this for VCOUNT 2..=161 (GBATEK video-capture window).
    pub fn dma_on_video_capture(&mut self) {
        if self.dma.reason(3) == Some(StartReason::VideoCapture) {
            self.dma_fire(3, StartReason::VideoCapture);
        }
    }

    fn dma_on_enable(&mut self, channel: usize, rising: bool) {
        if self.dma.busy {
            return;
        }
        let Some(reason) = self.dma.reason(channel) else {
            // Enable set with unsupported timing (DMA0 special).
            if self.dma.cnt_h(channel) & (1 << 15) != 0 {
                self.dma.clear_enable(channel);
            }
            return;
        };
        if rising {
            self.dma.latch(channel);
            if let Err(line) =
                dma::region_access(channel, self.dma.sad(channel), self.dma.dad(channel))
            {
                self.warn_lines.push(line.to_string());
                self.dma.clear_enable(channel);
                return;
            }
        }
        match reason {
            // Normal immediate DMA runs on the enable write (before the CPU
            // continues). 32-bit unused-I/O sources defer until the next real
            // data access so a following LDRH can update the CPU MDR first
            // (alyosha Bus/DMA_OAM_Bus).
            // Changing timing to Immediate while already enabled also starts
            // (alyosha DMA/DMA_Mode_Change) but does not re-latch addresses.
            StartReason::Immediate => {
                if !rising {
                    if let Err(line) = dma::region_access(
                        channel,
                        self.dma.job(channel).src,
                        self.dma.job(channel).dst,
                    ) {
                        self.warn_lines.push(line.to_string());
                        return;
                    }
                }
                let src = if rising {
                    self.dma.sad(channel)
                } else {
                    self.dma.job(channel).src
                };
                let width32 = self.dma.cnt_h(channel) & (1 << 10) != 0;
                if width32 && Self::dma_source_unused_io(src) {
                    self.dma_imm_pending[channel] = true;
                } else if self.dma_timing && rising {
                    // ares: active + waiting=2 after Immediate enable. Unit tests leave
                    // `dma_timing` off so they still see the copy on the enable write.
                    self.dma_imm_wait[channel] = 2;
                } else {
                    self.dma_fire(channel, StartReason::Immediate);
                    if !rising {
                        // Mode-change Immediate: keep Enable so CNT_H matches the
                        // written value (alyosha DMA/DMA_Mode_Change).
                        // Pad is 2S+2I plus the post-enable and bus-handoff waits.
                        self.dma.set_enable(channel);
                        // Pad uses a sequential 32-bit Game Pak word. A non-sequential
                        // word makes DMA_Mode_Change's timer six counts high.
                        let n32 = rom_cycles(self.waitcnt(), 0x0800_0000, Width::Word, true);
                        self.dma.stall = 2 * n32 + 2 + 2 + 2;
                    }
                }
            }
            StartReason::VBlank
            | StartReason::HBlank
            | StartReason::Fifo
            | StartReason::VideoCapture => {}
        }
    }

    fn dma_fire(&mut self, channel: usize, reason: StartReason) {
        if let Some(active) = self.dma_active {
            // Lower-or-equal priority cannot preempt (GBATEK channel order).
            if (channel as u8) >= active {
                return;
            }
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
        let prev = self.dma_active;
        let parent_written = self.dma_unit_written;
        let nested = prev.is_some();
        let phased =
            self.dma_timing && (nested || reason == StartReason::Immediate);
        self.dma_active = Some(channel as u8);
        self.dma.busy = true;
        self.dma_unit_written = false;
        self.last_rom = None;
        let units = if phased {
            self.dma_copy_phased(&job, nested)
        } else {
            self.dma_copy_atomic(&job)
        };
        self.dma_active = prev;
        self.dma.busy = prev.is_some();
        self.dma_unit_written = parent_written;
        if let Some(mask) = self.dma.finish(channel, reason, units, width32) {
            self.irq.raise(mask);
        }
        // Internal preempt leaves the parent's Game Pak burst sequential.
        if nested && !(0x08..=0x0D).contains(&(job.src >> 24)) {
            self.dma_rom_seq_resume = true;
        }
        if phased {
            self.dma.stall = 0;
        }
    }

    fn dma_copy_atomic(&mut self, job: &dma::Copy) -> u32 {
        let this = self as *mut Bus;
        let width32 = job.width32;
        let mut read = |addr: u32| -> u32 {
            // Safety: `dma_active` blocks equal/lower nested `dma_fire`; unit tests
            // keep `dma_timing` off so these closures do not re-enter mid-copy.
            unsafe { (*this).dma_read_unit(addr, width32) }
        };
        let mut write = |addr: u32, value: u32| unsafe {
            (*this).dma_write_unit(addr, value, width32);
        };
        let units = dma::run_copy(job, &mut read, &mut write);
        self.dma_finish_eeprom(job);
        units
    }

    /// ares-style unit loop: read then write; higher-priority DMA runs only when
    /// `writeCycle` is clear (after a write / before the next read).
    fn dma_copy_phased(&mut self, job: &dma::Copy, nested: bool) -> u32 {
        let width32 = job.width32;
        let unit_size = if width32 { 4u32 } else { 2 };
        let width = if width32 { Width::Word } else { Width::Half };
        let mut src = job.src;
        let mut dst = job.dst;
        // ares: CPU->DMA bus take. A writeCycle preempt that lands on the
        // arming cycle still owes GBATEK's 2-cycle startup; that cycle was
        // the in-progress write, not the turnaround.
        if nested && self.dma_hblank_from_write {
            if self.dma_abs() == self.dma_hblank_defer_abs {
                self.dma_phase_tick();
                self.dma_phase_tick();
            }
        } else if !nested {
            self.dma_phase_tick();
        }
        if nested {
            self.dma_hblank_from_write = false;
        }
        for unit in 0..job.units {
            self.dma_last_unit = unit + 1 == job.units;
            self.dma_drain_hblank();
            let value = self.dma_read_unit(src, width32);
            self.dma_write_cycle = true;
            self.dma_beat = 1;
            let resume_seq = self.dma_rom_seq_resume;
            self.dma_rom_seq_resume = false;
            for _ in 0..self.dma_access_ticks(src, width, resume_seq) {
                self.dma_phase_tick();
            }
            self.dma_beat = 2;
            let stored = if width32 { value } else { value & 0xffff };
            // ares setDMA: waitstates then write.
            for _ in 0..self.dma_access_ticks(dst, width, false) {
                self.dma_phase_tick();
            }
            self.dma_write_unit(dst, stored, width32);
            self.dma_unit_written = true;
            self.dma_write_cycle = false;
            src = dma_step_addr(src, job.src_ctrl, unit_size);
            dst = dma_step_addr(dst, job.dst_ctrl, unit_size);
            // Idle between units before a preempted channel reads.
            // EWRAM's access beats already include that turnaround.
            if unit + 1 < job.units && (dst >> 24) != 0x02 {
                // Next unit is not the last, and its source beat is the HBlank
                // edge: read the higher channel before this idle increments the timer.
                let next_src = self.dma_abs().wrapping_add(2);
                if unit + 2 < job.units && next_src % 1232 == 960 {
                    self.dma_hblank_from_write = false;
                    for channel in 0..4 {
                        if self.dma.reason(channel) == Some(StartReason::HBlank) {
                            self.dma_fire(channel, StartReason::HBlank);
                        }
                    }
                    self.hblank = true;
                }
                self.dma_phase_tick();
            }
            self.dma_drain_hblank();
        }
        self.dma_finish_eeprom(job);
        job.units
    }

    /// Memory beats for one DMA access (GBATEK waitstate tables).
    ///
    /// I/O matches ares `prefetchStep(1)` for 16- and 32-bit DMA.
    fn dma_access_ticks(&self, addr: u32, width: Width, force_seq: bool) -> u32 {
        let region = addr >> 24;
        if (0x08..=0x0D).contains(&region) {
            let sequential = force_seq
                || match self.last_rom {
                    Some((prev, prev_w)) => addr == prev.wrapping_add(prev_w),
                    None => false,
                };
            rom_cycles(self.waitcnt(), addr, width, sequential).max(1)
        } else if region == 0x04 {
            1
        } else {
            internal_cycles(addr, width).max(1)
        }
    }

    fn dma_drain_hblank(&mut self) {
        if self.dma_hblank_deferred && !self.dma_write_cycle {
            self.dma_hblank_deferred = false;
            for channel in 0..4 {
                if self.dma.reason(channel) == Some(StartReason::HBlank) {
                    self.dma_fire(channel, StartReason::HBlank);
                }
            }
        }
    }

    fn dma_read_unit(&mut self, addr: u32, width32: bool) -> u32 {
        let inaccessible = Self::dma_source_inaccessible(addr);
        let unused_io = Self::dma_source_unused_io(addr);
        let raw = if width32 {
            self.access(addr, 4, Access::Data)
        } else {
            self.access(addr, 2, Access::Data)
        };
        let value = if inaccessible || (!width32 && unused_io) {
            if width32 {
                self.dma_open
            } else {
                self.dma_open & 0xFFFF
            }
        } else if unused_io {
            self.last_data
        } else {
            raw
        };
        if width32 {
            if !inaccessible {
                self.dma_open = value;
            }
            self.last_data = self.dma_open;
            self.dma_open_cpu = true;
            self.dma_open
        } else {
            let half = value & 0xFFFF;
            self.dma_open = half | (half << 16);
            self.last_data = self.dma_open;
            self.dma_open_cpu = true;
            half
        }
    }

    fn dma_write_unit(&mut self, addr: u32, value: u32, width32: bool) {
        if width32 {
            self.store(addr, value, 4);
        } else {
            self.store(addr, value, 2);
        }
    }

    fn dma_finish_eeprom(&mut self, job: &dma::Copy) {
        if self.save_kind == SaveKind::Eeprom
            && (Self::eeprom_region(job.src) || Self::eeprom_region(job.dst))
            && let SaveChip::Eeprom(eeprom) = &mut self.save
        {
            eeprom.end_transfer();
            self.save_dirty = true;
        }
    }

    fn dma_abs(&self) -> u64 {
        self.cycle_base
            .wrapping_add(u64::from(self.step_cycles))
            .wrapping_add(u64::from(self.dma_cycles_paid))
    }

    /// One DMA bus phase: scanline first so HBlank DMA samples the timer before
    /// this cycle's increment (matches CPU `advance_cycles` edge-before-use for
    /// the same absolute time), then tick timers/APU.
    fn dma_phase_tick(&mut self) {
        if !self.dma_timing {
            return;
        }
        self.dma_cycles_paid = self.dma_cycles_paid.saturating_add(1);
        let abs = self
            .cycle_base
            .wrapping_add(u64::from(self.step_cycles))
            .wrapping_add(u64::from(self.dma_cycles_paid));

        const CYCLES_PER_LINE: u64 = 1232;
        const HBLANK_START: u64 = 960;
        const LINES: u64 = 228;
        let line = (abs / CYCLES_PER_LINE) % LINES;
        self.vcount = line as u16;
        let vblank = self.vcount >= 160;
        let hblank = (abs % CYCLES_PER_LINE) >= HBLANK_START;
        let vblank_edge = vblank && !self.vblank;
        let hblank_edge = hblank && !self.hblank;
        self.vblank = vblank;
        self.hblank = hblank;
        if vblank_edge {
            self.dma_on_vblank();
        }
        if hblank_edge {
            if self.vcount < 160 {
                self.dma_on_hblank();
            }
            if (2..=161).contains(&self.vcount) {
                self.dma_on_video_capture();
            }
        }

        let mask = self.timers.tick(1);
        self.tick_apu(mask);
        for index in 0..4u32 {
            if mask & (1 << index) != 0 {
                let control = self.timers.read16(index * 4 + 2);
                if control & (1 << 6) != 0 {
                    self.irq.raise(1 << (3 + index));
                }
            }
        }
    }

    /// DMA sources below EWRAM (BIOS / unused hole): keep `dma_open` (ares).
    fn dma_source_inaccessible(addr: u32) -> bool {
        addr < 0x0200_0000
    }

    /// I/O past the 1 KiB window (except the 0x800 mirror) is unused open bus.
    fn dma_source_unused_io(addr: u32) -> bool {
        if addr >> 24 != 0x04 {
            return false;
        }
        let off = addr & 0x00FF_FFFF;
        off >= IO as u32 && off != 0x800
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

    fn latch(&mut self, value: u32) {
        self.last_data = value;
    }

    fn openbus(&mut self, addr: u32, size: u32, region: &str) -> u32 {
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

    fn any_imm_pending(&self) -> bool {
        self.dma_imm_pending.iter().any(|&p| p)
    }

    /// Count down Immediate startup waits (GBATEK 2-cycle post-enable). Call once per cycle.
    pub fn tick_imm_dma_wait(&mut self) {
        for channel in 0..4 {
            if self.dma_imm_wait[channel] == 0 {
                continue;
            }
            self.dma_imm_wait[channel] -= 1;
            if self.dma_imm_wait[channel] == 0 {
                self.dma_imm_ready[channel] = true;
            }
        }
    }

    /// True when an Immediate channel finished its 2-cycle startup wait.
    pub fn any_imm_ready(&self) -> bool {
        self.dma_imm_ready.iter().any(|&r| r)
    }

    /// Fire every Immediate channel whose startup wait has elapsed.
    pub fn fire_ready_imm(&mut self) {
        for channel in 0..4 {
            if self.dma_imm_ready[channel] {
                self.dma_imm_ready[channel] = false;
                self.dma_fire(channel, StartReason::Immediate);
            }
        }
    }

    fn fire_pending_imm(&mut self) {
        for channel in 0..4 {
            if self.dma_imm_pending[channel] {
                self.dma_imm_pending[channel] = false;
                self.dma_imm_wait[channel] = 0;
                self.dma_imm_ready[channel] = false;
                self.dma_fire(channel, StartReason::Immediate);
            }
        }
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

fn dma_step_addr(addr: u32, ctrl: u8, unit_size: u32) -> u32 {
    match ctrl {
        0 | 3 => addr.wrapping_add(unit_size),
        1 => addr.wrapping_sub(unit_size),
        _ => addr,
    }
}

fn width_bytes(width: Width) -> u32 {
    match width {
        Width::Byte => 1,
        Width::Half => 2,
        Width::Word => 4,
    }
}

fn dma_open_reset() -> u32 {
    0xFFFF_FFFF
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
    if off < VRAM { off } else { off - 0x8000 }
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
