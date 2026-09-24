//! BIOS CpuSet / CpuFastSet (SWI 0x0B / 0x0C).
//!
//! Cited: GBATEK BIOS Functions — CpuSet / CpuFastSet.
//! <https://problemkaputt.de/gbatek.htm>

use crate::bus::Bus;

/// Length is `ctrl` bits 0–20. Bit 24 = fill (source does not advance). Bit 26 = 32-bit units, else 16-bit.
pub fn cpu_set(bus: &mut Bus, mut src: u32, mut dst: u32, ctrl: u32) {
    let mut count = ctrl & 0x001F_FFFF;
    if count == 0 {
        return;
    }
    let fill = ctrl & (1 << 24) != 0;
    let word = ctrl & (1 << 26) != 0;
    if word {
        if fill {
            let unit = bus.read32(src);
            while count > 0 {
                bus.write32(dst, unit);
                dst = dst.wrapping_add(4);
                count -= 1;
            }
        } else {
            while count > 0 {
                let value = bus.read32(src);
                bus.write32(dst, value);
                src = src.wrapping_add(4);
                dst = dst.wrapping_add(4);
                count -= 1;
            }
        }
    } else if fill {
        let unit = bus.read16(src);
        while count > 0 {
            bus.write16(dst, unit);
            dst = dst.wrapping_add(2);
            count -= 1;
        }
    } else {
        while count > 0 {
            let value = bus.read16(src);
            bus.write16(dst, value);
            src = src.wrapping_add(2);
            dst = dst.wrapping_add(2);
            count -= 1;
        }
    }
}

/// Always 32-bit units. Length (bits 0–20) is rounded up to a multiple of 8 words. Bit 24 = fill.
pub fn cpu_fast_set(bus: &mut Bus, mut src: u32, mut dst: u32, ctrl: u32) {
    let mut count = ctrl & 0x001F_FFFF;
    if count == 0 {
        return;
    }
    count = count.wrapping_add(7) & !7;
    let fill = ctrl & (1 << 24) != 0;
    if fill {
        let unit = bus.read32(src);
        while count > 0 {
            bus.write32(dst, unit);
            dst = dst.wrapping_add(4);
            count -= 1;
        }
    } else {
        while count > 0 {
            let value = bus.read32(src);
            bus.write32(dst, value);
            src = src.wrapping_add(4);
            dst = dst.wrapping_add(4);
            count -= 1;
        }
    }
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
