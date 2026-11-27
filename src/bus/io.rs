//! I/O window: display status, timers, keypad, IRQ, serial, and sound.
//!
//! Cited: GBATEK, GBA I/O Map. https://problemkaputt.de/gbatek.htm

use super::map::{covers_byte, sio_touches, slice_load, slice_store, sound_covers};
use super::{Bus, IRQ_READ_DELTA, IRQ_WRITE_DELTA, TIMER_STOP_DELTA, TIMER_STOP_MAX};
use crate::dma::{self, Dma};
use crate::timer::Timers;
use crate::timing::{Width, internal_cycles};

impl Bus {
    /// Writable DISPSTAT bits (IRQ enables and LYC), ignoring read-only status.
    pub fn dispstat_written(&self) -> u16 {
        slice_load(&self.io, 4, 2) as u16
    }

    /// True when VCOUNT equals the DISPSTAT LYC byte.
    pub fn vcount_match(&self) -> bool {
        let lyc = (self.dispstat_written() >> 8) as u8;
        self.vcount as u8 == lyc
    }

    /// Advance timers / Immediate DMA wait mid-instruction (SWI CpuSet, STRH trailing N).
    ///
    /// Unlike [`Self::add_internal_cycles`], this updates timer state immediately so a
    /// later op that enables DMA sees the correct TIM* counters before the instruction
    /// ends. Not added to `step_cycles` (Machine must not double-tick timers).
    pub(crate) fn tick_sio_start(&mut self) {
        if self.sio_start != 0 {
            self.sio_start -= 1;
            if self.sio_start == 0 {
                self.io[0x128] &= !0x80;
                // Normal 8-bit, nothing connected: the idle line shifts in 0xFF.
                // The high byte of SIODATA8 stays what software wrote.
                let mode = (self.io[0x129] >> 4) & 3;
                let rcnt = u16::from(self.io[0x134]) | (u16::from(self.io[0x135]) << 8);
                if mode == 0 && rcnt & 0x8000 == 0 {
                    self.io[0x12A] = 0xFF;
                }
            }
        }
        if self.sio_irq != 0 {
            self.sio_irq -= 1;
            if self.sio_irq == 0 {
                // SIOCNT bit 14: raise the serial IRQ when the transfer ends.
                if self.io[0x129] & 0x40 != 0 {
                    self.irq.raise(0x80);
                }
            }
        }
    }

    /// BIOS IntrWait check flags live at `0x03007FF8` and in [`Irq::check_flags`].
    pub(super) fn load_intr_check(&self, off: usize, size: u32) -> Option<u32> {
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

    pub(super) fn store_intr_check(&mut self, off: usize, value: u32, size: u32) -> bool {
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

    /// IE/IF/IME touch the interrupt controller part-way through an instruction.
    /// Bring the timers (the IF sources that race software) up to that point.
    fn sync_irq_access(&mut self, off: u32, size: u32, delta: u32) {
        if matches!(off, 0x200..=0x20B) {
            self.sync_timers(size, delta, u32::MAX);
        }
    }

    /// Tick the timers up to the point of the current access. `step_cycles`
    /// already includes it; a 32-bit access is two halfword beats on the 16-bit
    /// bus and the register sees the first one.
    fn sync_timers(&mut self, size: u32, delta: u32, max: u32) {
        if self.dma.busy {
            return;
        }
        let beats = if size == 4 { 2 } else { 1 };
        let target = self
            .step_cycles
            .saturating_add(delta + 1)
            .saturating_sub(beats)
            .min(max);
        while self.timer_ahead < target {
            self.timer_ahead += 1;
            self.tick_timers_one();
        }
    }

    /// A CPU write that stops a running timer lands a few clocks into the
    /// instruction; the counter keeps running until then (alyosha IF_Timer).
    fn sync_timer_stop(&mut self, off: u32, size: u32, value: u32) {
        let base = off.wrapping_sub(0x100);
        let (reg, control) = match size {
            4 if base & 3 == 0 => (base, value >> 16),
            2 if base & 3 == 2 => (base & !3, value),
            1 if base & 3 == 2 => (base & !3, value),
            _ => return,
        };
        if control & 0x80 != 0 || self.timers.read16(reg + 2) & 0x80 == 0 {
            return;
        }
        self.sync_timers(size, TIMER_STOP_DELTA, TIMER_STOP_MAX);
        // alyosha timer_disable: near-overflow run lands two short of 0xFFFF
        // after arm_skip is withheld; finish only the last few beats when the
        // live reload itself was a near-overflow value (not every high counter).
        let index = (reg / 4) as usize;
        if index < 4 {
            let reload = self.timers.reload(index);
            let c = self.timers.counter(index);
            if (0xFFE8..0xFFFF).contains(&reload) && (0xFFFC..0xFFFF).contains(&c) {
                for _ in 0..(u32::from(0xFFFF - c)) {
                    self.tick_timers_one();
                }
            }
        }
    }

    /// One timer clock: counters, serial start, APU FIFO, and timer IRQ requests.
    pub(crate) fn tick_timers_one(&mut self) {
        self.irq_prev = self.irq.pending();
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

    /// Consume the latched IRQ sample (see `irq_latch`).
    pub(crate) fn take_irq_latch(&mut self) -> bool {
        std::mem::take(&mut self.irq_latch)
    }

    /// Machine clock hook: true when this timer tick was already paid ahead.
    pub(crate) fn take_timer_ahead(&mut self) -> bool {
        if self.timer_ahead > 0 {
            self.timer_ahead -= 1;
            true
        } else {
            false
        }
    }

    pub(super) fn load_io(&mut self, off: u32, size: u32) -> u32 {
        self.sync_irq_access(off, size, IRQ_READ_DELTA);
        if sio_touches(off, size) {
            self.warn_sio();
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

    pub(super) fn store_io(&mut self, off: u32, value: u32, size: u32) {
        self.sync_irq_access(off, size, IRQ_WRITE_DELTA);
        if matches!(off, 0x200..=0x20B) && self.irq_prev {
            self.irq_latch = true;
        }
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
        if covers_byte(off, size, 0x128) {
            let shift = (0x128 - off) * 8;
            if value & (0x80 << shift) != 0 {
                let baud = self.io[0x128] & 3;
                // Normal-mode bit1: 0=256KHz, 1=2MHz (bits 0–1 look like MP baud).
                // Start bit clears at 316 (straddles start_bit / start_bit_2).
                // 256KHz IRQ is later so serial_time* samples ~0x0298; 2M keeps
                // clear and IRQ together at 90 (serial_time_2M*).
                if baud == 3 {
                    self.sio_start = 90;
                    self.sio_irq = 90;
                } else {
                    self.sio_start = 316;
                    self.sio_irq = 538;
                }
            }
        }
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
                self.enter_halt();
            }
            return true;
        }
        false
    }

    /// Tick the APU for one CPU cycle (same cadence as the timers).
    ///
    /// On a FIFO DMA request, runs channel 1 (A) or 2 (B) through [`Self::request_fifo`].
    pub fn tick_apu(&mut self, timer_overflow: u8) {
        // Do not clock FIFO samples while a DMA burst owns the bus; a nested
        // overflow would steal bytes from the fill (alyosha fifo.gba).
        let overflow = if self.dma.busy { 0 } else { timer_overflow };
        let (req_a, req_b) = self.apu.tick(overflow);
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

        if covers_byte(off, size, 0x80) || covers_byte(off, size, 0x82) {
            slice_store(&mut self.io, 0x80, u32::from(self.apu.cnt_l()), 2);
            slice_store(&mut self.io, 0x82, u32::from(self.apu.cnt_h()), 2);
        }

        // FIFO A: 0xA0..=0xA3, FIFO B: 0xA4..=0xA7. 8/16/32-bit stores push bytes
        // (low byte first). A full word at A0/A4 uses the atomic 4-byte push.
        // alyosha fifo.gba t002: CPU FIFO fills while sound is "off" (SOUNDCNT_H
        // FIFO enables clear and/or master off) do not queue — first timer overflow
        // still DMAs as if the FIFOs were empty.
        let master_on = self.apu.psg.read_master() & 0x80 != 0;
        let cnt_h = self.apu.cnt_h();
        // DMA refills must enqueue even when mixer enables are clear.
        let a_on = self.dma.busy || (master_on && (cnt_h & 0x0300) != 0);
        let b_on = self.dma.busy || (master_on && (cnt_h & 0x3000) != 0);
        if size == 4 && off == 0xA0 {
            if a_on {
                self.apu.fifo_a.push_word(value);
            }
        } else if size == 4 && off == 0xA4 {
            if b_on {
                self.apu.fifo_b.push_word(value);
            }
        } else {
            for i in 0..size {
                let addr = off.wrapping_add(i);
                let byte = (value >> (8 * i)) as u8;
                if (0xA0..=0xA3).contains(&addr) && a_on {
                    self.apu.fifo_a.push_byte(byte);
                } else if (0xA4..=0xA7).contains(&addr) && b_on {
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
            0x84 => {
                let was_on = self.apu.psg.read_master() & 0x80 != 0;
                self.apu.psg.write(0x20, value);
                // alyosha fifo.gba t003: disabling master clears both FIFOs.
                if was_on && self.apu.psg.read_master() & 0x80 == 0 {
                    self.apu.fifo_a.reset();
                    self.apu.fifo_b.reset();
                }
            }
            0x88 => self.apu.set_bias(value),
            0x90..=0x9E => self.apu.psg.write(off - 0x60, value),
            _ => {}
        }
    }

    fn load_timer_io(&self, off: u32, size: u32) -> u32 {
        let base = off.wrapping_sub(0x100);
        let read = if self.dma.busy {
            Timers::read16_dma
        } else {
            Timers::read16
        };
        match size {
            1 => {
                let half = read(&self.timers, base & !1);
                u32::from(if base & 1 != 0 {
                    half >> 8
                } else {
                    half & 0xFF
                })
            }
            2 => u32::from(read(&self.timers, base)),
            4 => {
                let lo = u32::from(read(&self.timers, base));
                let hi = u32::from(read(&self.timers, base.wrapping_add(2)));
                lo | (hi << 16)
            }
            _ => 0,
        }
    }

    fn store_timer_io(&mut self, off: u32, value: u32, size: u32) {
        self.sync_timer_stop(off, size, value);
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
            2 => {
                if self.dma.busy {
                    self.timers.write16_dma(base, value as u16);
                } else {
                    self.timers.write16(base, value as u16);
                }
            }
            4 => {
                let was = self.timers.read16(base.wrapping_add(2)) & 0x80 != 0;
                let prelude = self
                    .step_cycles
                    .saturating_sub(internal_cycles(0x0400_0100, Width::Word));
                let enabling = (value >> 16) & 0x80 != 0;
                self.timers.write16(base, value as u16);
                if self.dma.busy {
                    self.timers
                        .write16_dma(base.wrapping_add(2), (value >> 16) as u16);
                } else {
                    self.timers
                        .write16(base.wrapping_add(2), (value >> 16) as u16);
                }
                // CPU word enables skip the already-charged opcode fetch beats.
                // DMA enables must not — alyosha timer_reset's post-DMA CPU read
                // expects the counter to have advanced from the reload (~0xF3).
                if enabling && !was && !self.dma.busy {
                    // timer_disable (reload FFEB, 20 counts to overflow) must not
                    // arm_skip — that left the stop sample at 0xF9. Broader
                    // near-overflow reloads (e.g. BL_IRQ_3 FFE0) still skip.
                    let reload = value as u16;
                    if u32::from(reload) < 0x1_0000 - 24 {
                        let skip = prelude.min(255) as u8;
                        self.timers.arm_skip((base / 4) as usize, skip);
                    }
                }
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

    fn warn_sio(&mut self) {
        if self.sio_warned {
            return;
        }
        self.sio_warned = true;
        self.warn_lines
            .push("gba-debug: warn sio unlinked".to_string());
    }
}
