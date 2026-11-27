//! Memory map for the pages that can run a ROM.
//!
//! Reads and writes live in `access`, the I/O window in `io`, save and GPIO
//! in `cart`, and the copy engine in `dma`.
//!
//! Cited: GBATEK, GBA Memory Map. https://problemkaputt.de/gbatek.htm

use std::collections::HashSet;

use crate::apu::Apu;
use crate::cart::{Eeprom, Flash, FlashSize, Gpio, SaveKind, detect_rtc, detect_save};
use crate::dma::Dma;
use crate::input::Keypad;
use crate::irq::Irq;
use crate::timer::Timers;
use crate::timing::{Prefetch, Width, rom_cycles, sram_cycles, ws0_ns};

mod access;
mod cart;
mod dma;
mod io;
mod map;

#[cfg(test)]
mod tests;

/// IE/IF/IME and timer-stop accesses happen part-way through an instruction.
/// Clocks past the end of the access's first bus beat at which the interrupt
/// controller sees a read or a write, and the timer block sees a stop
/// (alyosha irq/IE, IF, IF_Timer).
const IRQ_READ_DELTA: u32 = 1;
const IRQ_WRITE_DELTA: u32 = 2;
const TIMER_STOP_DELTA: u32 = 2;
/// A timer stop lands at most this many clocks into its instruction, so an
/// ARM `str` (two-clock opcode fetch) does not park the counter on 0xFFFF
/// (alyosha irq/BL_IRQ_3).
const TIMER_STOP_MAX: u32 = 4;

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
    /// Cart GPIO at 0x080000C4/C6/C8. `Some` for SIIRTC carts or test-forced GPIO.
    gpio: Option<Gpio>,
    gpio_warned: bool,
    pub timers: Timers,
    pub irq: Irq,
    pub keypad: Keypad,
    pub dma: Dma,
    pub apu: Apu,
    /// Set by HALTCNT byte 0 or SWI 2; cleared when an enabled IRQ wakes the CPU.
    pub halted: bool,
    /// CPU cycles spent in Halt since entry (alyosha halt_pc stacked LR phase).
    #[serde(default)]
    pub halt_age: u32,
    /// Halt was entered through SWI 2 (BIOS body ahead of HALTCNT), not a raw write.
    #[serde(default)]
    pub halt_via_swi: bool,
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
    /// Captured MGBA debug-printf lines (`0x04FFF600` / `0x04FFF700` / `0x04FFF780`).
    #[serde(skip)]
    pub mgba_log: Vec<String>,
    #[serde(skip)]
    mgba_enabled: bool,
    #[serde(skip)]
    mgba_buf: Vec<u8>,
    openbus_logged: HashSet<u32>,
    sio_warned: bool,
    /// Cycles left until an internal serial start bit (SIOCNT bit 7) clears.
    /// 316 leaves serial_time_start_bit set and serial_time_start_bit_2 clear.
    #[serde(default)]
    sio_start: u16,
    /// Cycles left until the serial IRQ after a transfer starts. For 256KHz this
    /// outlasts [`Self::sio_start`] so the start bit clears before the IRQ (alyosha
    /// `serial_time*` vs `serial_time_start_bit*`).
    #[serde(default)]
    sio_irq: u16,
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
    /// Timer cycles already ticked ahead of the machine clock (IE/IF/IME access
    /// timing). The machine skips that many timer ticks as it pays the step.
    #[serde(default)]
    timer_ahead: u32,
    /// The CPU IRQ line was asserted just before an IE/IF/IME write dropped it.
    /// The core already sampled it, so the next instruction boundary still takes it.
    #[serde(default)]
    irq_latch: bool,
    /// CPU IRQ line one timer clock ago (the core samples it a cycle late).
    #[serde(default)]
    irq_prev: bool,
    /// True when this step touched ROM or SRAM (not internal-only).
    step_pak: bool,
    /// Cycles this step spent off the Game Pak bus (I/O, internal). Prefetch fills then.
    #[serde(default)]
    cart_idle: u32,
    /// Empty Thumb POP aborts prefetch: later opcode fetches stay non-sequential
    /// until a later instruction actually refills the buffer.
    #[serde(default)]
    prefetch_ns: bool,
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
    /// Cycles [`Self::elapse`] already applied to timers. Machine advances the
    /// frame clock by this many without ticking the timers again.
    #[serde(skip)]
    elapse_frame: u32,
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
    /// Deferred HBlank was armed on the source beat of the last unit.
    #[serde(skip)]
    dma_hblank_src_edge: bool,
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
        let gpio = if detect_rtc(&rom) {
            Some(Gpio::with_rtc())
        } else {
            None
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
            gpio,
            gpio_warned: false,
            timers: Timers::new(),
            irq: Irq::new(),
            keypad: Keypad::new(),
            dma: Dma::new(),
            apu: Apu::new(),
            halted: false,
            halt_age: 0,
            halt_via_swi: false,
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
            mgba_log: Vec::new(),
            mgba_enabled: false,
            mgba_buf: vec![0; 256],
            openbus_logged: HashSet::new(),
            sio_warned: false,
            sio_start: 0,
            sio_irq: 0,
            swi_warned: 0,
            // WAITCNT reset is 0 → WS0 N=4, S=2, I=1, SRAM=4.
            wait_rom_n: 4,
            wait_rom_s: 2,
            wait_rom_i: 1,
            wait_sram: 4,
            prefetch: Prefetch::new(),
            step_cycles: 0,
            timer_ahead: 0,
            irq_latch: false,
            irq_prev: false,
            step_pak: false,
            cart_idle: 0,
            prefetch_ns: false,
            last_rom: None,
            gb_cart_shape: false,
            gb_mode: false,
            cgb_romdis: false,
            save_dirty: false,
            emu_cycles: 0,
            elapse_frame: 0,
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
            dma_hblank_src_edge: false,
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

    /// Enter Halt (SWI 2 / HALTCNT byte 0). Resets the Halt-age counter.
    pub fn enter_halt(&mut self) {
        self.halted = true;
        self.halt_age = 0;
        self.halt_via_swi = false;
    }

    /// Enter Halt through SWI 2: the BIOS body runs before HALTCNT (see `cpu::halt`).
    pub fn enter_halt_swi(&mut self) {
        self.enter_halt();
        self.halt_via_swi = true;
    }

    /// Cycles spent halted since [`Self::enter_halt`], then clear.
    pub fn take_halt_age(&mut self) -> u32 {
        let age = self.halt_age;
        self.halt_age = 0;
        age
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
    /// remaining 1N (ARM 2S+1N total). Forces the next fetch non-sequential; the
    /// prefetch buffer is kept so a hit at `target` still costs 1 (GBAHawk).
    /// Branch-to-next while empty: CPU pays N while the unit keeps its S cursor.
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
        self.cart_idle = 0;
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
    /// Opcode fetches stay non-sequential until the next prefetch refill.
    pub fn mark_prefetch_ns(&mut self) {
        self.prefetch_ns = true;
    }

    pub fn finish_step(&mut self, next_opcode: u32, thumb: bool) {
        let region = next_opcode >> 24;
        if !(0x08..=0x0D).contains(&region) {
            return;
        }
        // Non-cart cycles after a Game Pak fetch fill the prefetch buffer.
        let _ = thumb;
        let idle = if self.step_pak {
            self.cart_idle
        } else {
            self.step_cycles
        };
        if idle == 0 {
            return;
        }
        let waitcnt = self.waitcnt();
        let s = rom_cycles(waitcnt, next_opcode, Width::Half, true);
        self.prefetch.idle(idle, next_opcode, s);
        self.prefetch_ns = false;
    }

    /// Cycles charged for the instruction just stepped (at least 1 for the machine).
    pub fn take_step_cycles(&mut self) -> u32 {
        let n = self.step_cycles.max(1);
        self.step_cycles = 0;
        n
    }

    /// WAITCNT halfword at I/O 0x04000204.
    pub fn waitcnt(&self) -> u16 {
        let stored = map::slice_load(&self.io, 0x204, 2) as u16;
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
        map::slice_load(&self.io, 0, 2) as u16
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

    /// Add bare internal cycles (ARM LDR trailing I, etc.).
    pub fn add_internal_cycles(&mut self, n: u32) {
        if self.dma.busy {
            return;
        }
        self.cart_idle = self.cart_idle.saturating_add(n);
        self.step_cycles = self.step_cycles.saturating_add(n);
    }

    pub fn elapse(&mut self, n: u32) {
        if self.dma.busy || n == 0 {
            return;
        }
        // These clocks already moved the timers. The frame clock has to follow
        // or a VBlank-timed DMA restart lands after the FIFO has read past the
        // sample (Advance Wars 2's left-channel click).
        self.elapse_frame = self.elapse_frame.saturating_add(n);
        for _ in 0..n {
            self.emu_cycles = self.emu_cycles.wrapping_add(1);
            self.tick_imm_dma_wait();
            let mask = self.timers.tick(1);
            self.tick_sio_start();
            self.tick_apu(mask);
            if self.any_imm_ready() {
                self.cycle_base = self.emu_cycles;
                self.dma_cycles_paid = 0;
                self.fire_ready_imm();
                let paid = self.take_dma_cycles_paid();
                self.emu_cycles = self.emu_cycles.wrapping_add(u64::from(paid));
                self.elapse_frame = self.elapse_frame.saturating_add(paid);
            }
        }
    }

    /// Frame-clock cycles already applied to the timers by [`Self::elapse`].
    pub(crate) fn take_elapse_frame(&mut self) -> u32 {
        let n = self.elapse_frame;
        self.elapse_frame = 0;
        n
    }
}

fn dma_open_reset() -> u32 {
    0xFFFF_FFFF
}
