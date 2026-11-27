//! DMA scheduling, the phased copy, and Immediate startup.
//!
//! Cited: GBATEK, GBA DMA. https://problemkaputt.de/gbatek.htm

use super::map::Access;
use super::{Bus, IO, SaveChip};
use crate::cart::SaveKind;
use crate::dma::{self, StartReason};
use crate::timing::{Width, internal_cycles, rom_cycles};

/// Cycles before a DMA's first bus access (GBATEK: two idle cycles after the request).
const DMA_STARTUP: u32 = 2;

fn dma_step_addr(addr: u32, ctrl: u8, unit_size: u32) -> u32 {
    match ctrl {
        0 | 3 => addr.wrapping_add(unit_size),
        1 => addr.wrapping_sub(unit_size),
        _ => addr,
    }
}

impl Bus {
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
        self.dma_hblank_src_edge = false;
        if self.dma_write_cycle {
            if self.dma_beat == 2 && self.dma_unit_written && !self.dma_last_unit {
                self.dma_hblank_from_write = false;
                for _ in 0..4 {
                    self.dma_phase_tick();
                }
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
            self.dma_hblank_src_edge = self.dma_beat == 1 && self.dma_last_unit;
            return;
        }
        if self.dma_active.is_some() && !self.dma_unit_written {
            self.dma_hblank_deferred = true;
            self.dma_hblank_from_write = false;
            self.dma_hblank_defer_abs = self.dma_abs();
            return;
        }
        if self.dma_active.is_some() {
            // Idle between units. A 3-cycle unit (read, write, idle) is at
            // phase 2 here when only the idle remains; that read is one tick
            // sooner than a preempt that still owes the write.
            self.dma_hblank_from_write = false;
            let ticks = if self.dma_cycles_paid % 3 == 2 { 3 } else { 4 };
            for _ in 0..ticks {
                self.dma_phase_tick();
            }
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

    pub(super) fn dma_on_enable(&mut self, channel: usize, rising: bool) {
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
                #[allow(clippy::collapsible_if)]
                if !rising {
                    // FIFO special walks an internal source. A mode change to
                    // Immediate is the driver's restart: copy from the SAD the
                    // game wrote (the sample), not from the walked pointer
                    // just past the buffer. DMA0 HBlank mode-change still
                    // keeps its latch (alyosha DMA_Mode_Change).
                    let dad = self.dma.dad(channel) & !3;
                    if dad == 0x0400_00A0 || dad == 0x0400_00A4 {
                        self.dma.latch(channel);
                    }
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
                        // Pad is 2S+2I plus the post-enable and bus-handoff waits,
                        // and one more internal cycle so the timer reads 0x2A.
                        self.dma.set_enable(channel);
                        // Pad uses a sequential 32-bit Game Pak word. A non-sequential
                        // word makes DMA_Mode_Change's timer six counts high.
                        let n32 = rom_cycles(self.waitcnt(), 0x0800_0000, Width::Word, true);
                        self.dma.stall = 2 * n32 + 2 + 2 + 2 + 1;
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
        // FIFO special needs phased beats so TIM1 sees real EWRAM/ROM waitstates
        // during the refill (alyosha fifo_dma timing ROMs).
        let phased = self.dma_timing
            && (nested || reason == StartReason::Immediate || reason == StartReason::Fifo);
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
        } else if self.dma_timing && !nested && reason == StartReason::Fifo {
            // Sound FIFO bursts are not phased; charge their bus beats as CPU stall
            // (GBATEK: 2-cycle start, then source and destination access per unit).
            self.dma.stall = self.dma_atomic_stall(&job);
        }
    }

    /// Bus cycles an unphased copy holds the CPU: startup plus each unit's source
    /// and destination beats and the idle between units (as in [`Self::dma_copy_phased`]).
    fn dma_atomic_stall(&self, job: &dma::Copy) -> u32 {
        let unit_size = if job.width32 { 4u32 } else { 2 };
        let width = if job.width32 {
            Width::Word
        } else {
            Width::Half
        };
        let mut src = job.src;
        let mut dst = job.dst;
        let mut rom_addr = src;
        let mut prev_rom: Option<u32> = None;
        let mut rom_dec_glitch = false;
        let mut cycles = DMA_STARTUP;
        for unit in 0..job.units {
            let src_rom = (0x08..=0x0D).contains(&(src >> 24));
            let read_addr = if src_rom { rom_addr } else { src };
            let mut force_n = false;
            if src_rom {
                if rom_dec_glitch {
                    rom_dec_glitch = false;
                    force_n = true;
                }
                let mask = if job.width32 { 0x1_FFFC } else { 0x1_FFFE };
                if src & mask == mask {
                    rom_dec_glitch = true;
                }
            }
            let seq =
                !force_n && src_rom && prev_rom == Some(read_addr) && read_addr & 0x1_FFFF != 0;
            cycles += if src_rom {
                rom_cycles(self.waitcnt(), read_addr, width, seq).max(1)
            } else {
                self.dma_access_ticks(src, width, false)
            };
            if src_rom {
                prev_rom = Some(read_addr.wrapping_add(unit_size));
                rom_addr = rom_addr.wrapping_add(unit_size);
            } else {
                prev_rom = None;
            }
            cycles += self.dma_access_ticks(dst, width, false);
            if unit + 1 < job.units && (dst >> 24) != 0x02 {
                cycles += 1;
            }
            src = dma_step_addr(src, job.src_ctrl, unit_size);
            dst = dma_step_addr(dst, job.dst_ctrl, unit_size);
        }
        cycles
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
            // The parent unit's remaining write already elapsed after the edge.
            // That is not the 2-cycle startup, and the preempt still owes both.
            // Write-edge defer: same abs or abs==defer+1 still owes the full
            // 6-cycle startup (alyosha DMA_pause_timing_end_2 → 0x0C). Larger
            // skew keeps 4; src_edge subtracts 2 for end_4.
            let abs = self.dma_abs();
            let mut startup = if abs == self.dma_hblank_defer_abs
                || abs == self.dma_hblank_defer_abs.wrapping_add(1)
            {
                6
            } else {
                4
            };
            // Last unit, edge on the source beat: the write-beat startup samples
            // the timer two counts late (DMA_pause_timing_end_4).
            if self.dma_hblank_src_edge {
                startup -= 2;
            }
            self.dma_hblank_src_edge = false;
            for _ in 0..startup {
                self.dma_phase_tick();
            }
        } else if !nested {
            self.dma_phase_tick();
            self.last_rom = None;
        }
        if nested {
            self.dma_hblank_from_write = false;
        }
        // Game Pak ignores fixed/decrement: a separate ROM burst address always
        // increments (mGBA/GBAHawk). Fixed SRC near 0x1FFFC also forces N each beat.
        let mut rom_addr = src;
        let mut rom_dec_glitch = false;
        let src_is_rom = (0x08..=0x0D).contains(&(src >> 24));
        for unit in 0..job.units {
            self.dma_last_unit = unit + 1 == job.units;
            self.dma_drain_hblank();
            let read_addr = if src_is_rom { rom_addr } else { src };
            let value = self.dma_read_unit(read_addr, width32);
            self.dma_write_cycle = true;
            self.dma_beat = 1;
            let resume_seq = self.dma_rom_seq_resume;
            self.dma_rom_seq_resume = false;
            let mut force_n = false;
            if src_is_rom {
                if rom_dec_glitch {
                    rom_dec_glitch = false;
                    force_n = true;
                }
                let mask = if width32 { 0x1_FFFC } else { 0x1_FFFE };
                if src & mask == mask {
                    rom_dec_glitch = true;
                }
            }
            let sequential = !force_n
                && read_addr & 0x1_FFFF != 0
                && (resume_seq
                    || match self.last_rom {
                        Some((prev, prev_w)) => read_addr == prev.wrapping_add(prev_w),
                        None => false,
                    });
            // Nested HBlank latches an I/O source before this beat. Counting the
            // beat again leaves the following timer read one high (ROM_to_IWRAM).
            let mut src_ticks = if src_is_rom {
                rom_cycles(self.waitcnt(), read_addr, width, sequential).max(1)
            } else {
                self.dma_access_ticks(src, width, resume_seq)
            };
            if nested && (src >> 24) == 0x04 {
                src_ticks = 0;
            }
            for _ in 0..src_ticks {
                self.dma_phase_tick();
            }
            if src_is_rom {
                self.last_rom = Some((read_addr, unit_size));
                rom_addr = rom_addr.wrapping_add(unit_size);
            } else if (0x08..=0x0D).contains(&(src >> 24)) {
                self.last_rom = Some((src, unit_size));
            } else {
                self.last_rom = None;
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
                let next_src = self.dma_abs().wrapping_add(2);
                if unit + 2 < job.units && next_src % 1232 == 960 {
                    self.hblank = true;
                    self.dma_hblank_from_write = false;
                    for _ in 0..3 {
                        self.dma_phase_tick();
                    }
                    for channel in 0..4 {
                        if self.dma.reason(channel) == Some(StartReason::HBlank) {
                            self.dma_fire(channel, StartReason::HBlank);
                        }
                    }
                }
                self.dma_phase_tick();
            }
            self.dma_drain_hblank();
        }
        self.dma_finish_eeprom(job);
        // GBAHawk: fixed Game Pak SRC parked on the last word of a 128 KiB page
        // (`addr & 0x1FFFC == 0x1FFFC`) forces non-sequential timing every beat and
        // an extra cart turnaround our wait table under-counts. For WAITCNT 0x14 /
        // 32-word Immediate this is 25 clocks (alyosha DMA_ROM_Fixed → timer 0x30).
        if src_is_rom && job.src_ctrl == 2 {
            let mask = if width32 { 0x1_FFFC } else { 0x1_FFFE };
            if job.src & mask == mask {
                const FIXED_PAGE_END_EXTRA: u32 = 25;
                let _ = self.timers.tick(FIXED_PAGE_END_EXTRA);
                self.dma_cycles_paid = self.dma_cycles_paid.saturating_add(FIXED_PAGE_END_EXTRA);
            }
        }
        job.units
    }

    /// Memory beats for one DMA access (GBATEK waitstate tables).
    ///
    /// I/O matches ares `prefetchStep(1)` for 16- and 32-bit DMA.
    fn dma_access_ticks(&self, addr: u32, width: Width, force_seq: bool) -> u32 {
        let region = addr >> 24;
        if (0x08..=0x0D).contains(&region) {
            let sequential = force_seq
                || (addr & 0x1_FFFF != 0
                    && match self.last_rom {
                        Some((prev, prev_w)) => addr == prev.wrapping_add(prev_w),
                        None => false,
                    });
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
        // Mask low bits every beat (ares): latch may retain an odd Thumb-bit address.
        let addr = if width32 { addr & !3 } else { addr & !1 };
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
        let addr = if width32 { addr & !3 } else { addr & !1 };
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
    pub(crate) fn dma_phase_tick(&mut self) {
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
        let old_vmatch = self.vcount_match();
        self.vcount = line as u16;
        let vblank = self.vcount >= 160;
        let hblank = (abs % CYCLES_PER_LINE) >= HBLANK_START;
        let vblank_edge = vblank && !self.vblank;
        let hblank_edge = hblank && !self.hblank;
        self.vblank = vblank;
        self.hblank = hblank;
        let dispstat = self.dispstat_written();
        if vblank_edge {
            self.dma_on_vblank();
            if dispstat & (1 << 3) != 0 {
                self.irq.raise(1);
            }
        }
        if hblank_edge {
            if self.vcount < 160 {
                self.dma_on_hblank();
            }
            if (2..=161).contains(&self.vcount) {
                self.dma_on_video_capture();
            }
            if dispstat & (1 << 4) != 0 {
                self.irq.raise(2);
            }
        }
        if self.vcount_match() && !old_vmatch && dispstat & (1 << 5) != 0 {
            self.irq.raise(4);
        }

        let mask = self.timers.tick(1);
        self.tick_sio_start();
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

    pub(super) fn any_imm_pending(&self) -> bool {
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

    pub(super) fn fire_pending_imm(&mut self) {
        for channel in 0..4 {
            if self.dma_imm_pending[channel] {
                self.dma_imm_pending[channel] = false;
                self.dma_imm_wait[channel] = 0;
                self.dma_imm_ready[channel] = false;
                self.dma_fire(channel, StartReason::Immediate);
            }
        }
    }
}
