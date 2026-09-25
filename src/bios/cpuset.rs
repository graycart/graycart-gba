//! BIOS CpuSet / CpuFastSet (SWI 0x0B / 0x0C).
//!
//! Cited: GBATEK BIOS Functions — CpuSet / CpuFastSet.
//! <https://problemkaputt.de/gbatek.htm>
//!
//! Real BIOS code runs from the BIOS region with a per-unit instruction loop.
//! HLE performs the memory ops directly and elapses approximate BIOS overhead so
//! timers keep step with hardware when a transfer enables DMA mid-SWI
//! (alyosha timing/dma_from_bios). Prologue/epilogue constants are approximate
//! HLE padding, not measurements from a counted BIOS instruction loop.

use crate::bus::Bus;
use crate::timing::{Width, internal_cycles};

/// Cycles for BIOS entry before the first transfer unit (SWI 0x0B/0x0C).
const BIOS_CPUSET_PROLOGUE: u32 = 0x46;
/// Cycles for BIOS exit after the last transfer unit.
const BIOS_CPUSET_EPILOGUE: u32 = 0x08;

/// Length is `ctrl` bits 0–20. Bit 24 = fill (source does not advance). Bit 26 = 32-bit units, else 16-bit.
pub fn cpu_set(bus: &mut Bus, mut src: u32, mut dst: u32, ctrl: u32) {
    let mut count = ctrl & 0x001F_FFFF;
    if count == 0 {
        return;
    }
    let fill = ctrl & (1 << 24) != 0;
    let word = ctrl & (1 << 26) != 0;
    bus.elapse(BIOS_CPUSET_PROLOGUE);
    if word {
        if fill {
            let unit = bus.read32(src);
            while count > 0 {
                bus.write32(dst, unit);
                dst = dst.wrapping_add(4);
                count -= 1;
                bus.elapse(unit_overhead(src, dst, true));
            }
        } else {
            while count > 0 {
                let value = bus.read32(src);
                bus.write32(dst, value);
                src = src.wrapping_add(4);
                dst = dst.wrapping_add(4);
                count -= 1;
                bus.elapse(unit_overhead(src, dst, true));
            }
        }
    } else if fill {
        let unit = bus.read16(src);
        while count > 0 {
            bus.write16(dst, unit);
            dst = dst.wrapping_add(2);
            count -= 1;
            bus.elapse(unit_overhead(src, dst, false));
        }
    } else {
        while count > 0 {
            let value = bus.read16(src);
            bus.write16(dst, value);
            src = src.wrapping_add(2);
            dst = dst.wrapping_add(2);
            count -= 1;
            bus.elapse(unit_overhead(src, dst, false));
        }
    }
    bus.elapse(BIOS_CPUSET_EPILOGUE);
}

fn unit_overhead(src: u32, dst: u32, word: bool) -> u32 {
    let width = if word { Width::Word } else { Width::Half };
    // BIOS loop: a few instruction fetches around each transfer beat.
    8 + internal_cycles(src, width) + internal_cycles(dst, width)
}

/// Always 32-bit units. Length (bits 0–20) is rounded up to a multiple of 8 words. Bit 24 = fill.
pub fn cpu_fast_set(bus: &mut Bus, mut src: u32, mut dst: u32, ctrl: u32) {
    let mut count = ctrl & 0x001F_FFFF;
    if count == 0 {
        return;
    }
    count = count.wrapping_add(7) & !7;
    let fill = ctrl & (1 << 24) != 0;
    bus.elapse(BIOS_CPUSET_PROLOGUE);
    if fill {
        let unit = bus.read32(src);
        while count > 0 {
            bus.write32(dst, unit);
            dst = dst.wrapping_add(4);
            count -= 1;
            bus.elapse(unit_overhead(src, dst, true));
        }
    } else {
        while count > 0 {
            let value = bus.read32(src);
            bus.write32(dst, value);
            src = src.wrapping_add(4);
            dst = dst.wrapping_add(4);
            count -= 1;
            bus.elapse(unit_overhead(src, dst, true));
        }
    }
    bus.elapse(BIOS_CPUSET_EPILOGUE);
}

#[cfg(test)]
mod tests {
    use super::{cpu_fast_set, cpu_set};
    use crate::bus::Bus;

    #[test]
    fn halfword_copy_writes_two_units() {
        let mut bus = Bus::new(vec![0; 0x200]);
        bus.write16(0x0300_0000, 0x1111);
        bus.write16(0x0300_0002, 0x2222);
        cpu_set(&mut bus, 0x0300_0000, 0x0300_0100, 2);
        assert_eq!(bus.read16(0x0300_0100), 0x1111);
        assert_eq!(bus.read16(0x0300_0102), 0x2222);
    }

    #[test]
    fn word_fill_repeats_source_word() {
        let mut bus = Bus::new(vec![0; 0x200]);
        bus.write32(0x0300_0000, 0xAABB_CCDD);
        cpu_set(
            &mut bus,
            0x0300_0000,
            0x0300_0100,
            2 | (1 << 24) | (1 << 26),
        );
        assert_eq!(bus.read32(0x0300_0100), 0xAABB_CCDD);
        assert_eq!(bus.read32(0x0300_0104), 0xAABB_CCDD);
    }

    #[test]
    fn fast_set_rounds_length_up_to_eight_words() {
        let mut bus = Bus::new(vec![0; 0x200]);
        for i in 0..8u32 {
            bus.write32(0x0300_0000 + i * 4, 0x1000 + i);
        }
        cpu_fast_set(&mut bus, 0x0300_0000, 0x0300_0100, 1);
        for i in 0..8u32 {
            assert_eq!(bus.read32(0x0300_0100 + i * 4), 0x1000 + i);
        }
    }

    #[test]
    fn fast_set_fill_of_length_one_writes_eight_words() {
        let mut bus = Bus::new(vec![0; 0x200]);
        bus.write32(0x0300_0000, 0xDEAD_BEEF);
        cpu_fast_set(&mut bus, 0x0300_0000, 0x0300_0100, 1 | (1 << 24));
        for i in 0..8u32 {
            assert_eq!(bus.read32(0x0300_0100 + i * 4), 0xDEAD_BEEF);
        }
    }
}
