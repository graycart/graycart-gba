//! WAITCNT N/S/I cycle costs. Page 11 fills this in.
//!
//! Cited: GBATEK Game Pak Memory Waitstates.
//! <https://problemkaputt.de/gbatek.htm>

/// Access width for cycle costing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    Byte,
    Half,
    Word,
}

const FIRST_WAIT: [u32; 4] = [4, 3, 2, 8];

fn first_wait(code: u16) -> u32 {
    FIRST_WAIT[(code & 3) as usize]
}

/// SRAM 8-bit wait from WAITCNT bits 0–1.
pub fn sram_cycles(waitcnt: u16) -> u32 {
    first_wait(waitcnt & 3)
}

fn ws0_n(waitcnt: u16) -> u32 {
    first_wait((waitcnt >> 2) & 3)
}

fn ws0_s(waitcnt: u16) -> u32 {
    if waitcnt & (1 << 4) != 0 { 1 } else { 2 }
}

fn ws1_n(waitcnt: u16) -> u32 {
    first_wait((waitcnt >> 5) & 3)
}

fn ws1_s(waitcnt: u16) -> u32 {
    if waitcnt & (1 << 7) != 0 { 1 } else { 4 }
}

fn ws2_n(waitcnt: u16) -> u32 {
    first_wait((waitcnt >> 8) & 3)
}

fn ws2_s(waitcnt: u16) -> u32 {
    if waitcnt & (1 << 10) != 0 { 1 } else { 8 }
}

fn rom_ns(waitcnt: u16, addr: u32) -> (u32, u32) {
    match addr {
        0x0800_0000..=0x09FF_FFFF => (ws0_n(waitcnt), ws0_s(waitcnt)),
        0x0A00_0000..=0x0BFF_FFFF => (ws1_n(waitcnt), ws1_s(waitcnt)),
        0x0C00_0000..=0x0DFF_FFFF => (ws2_n(waitcnt), ws2_s(waitcnt)),
        // Treat other Game Pak / cart mirrors like WS0 if misrouted.
        _ => (ws0_n(waitcnt), ws0_s(waitcnt)),
    }
}

/// ROM (WS0/WS1/WS2) access cycles for the given width.
///
/// 8/16-bit: one N or one S. 32-bit Game Pak: N+S (non-sequential) or S+S
/// (sequential), per GBATEK. Prefetch word hits stay cost 1 in the bus charge
/// path (`Prefetch::take_n`), not here.
pub fn rom_cycles(waitcnt: u16, addr: u32, width: Width, sequential: bool) -> u32 {
    let (n, s) = rom_ns(waitcnt, addr);
    match width {
        Width::Byte | Width::Half => {
            if sequential {
                s
            } else {
                n
            }
        }
        Width::Word => {
            if sequential {
                s.saturating_add(s)
            } else {
                n.saturating_add(s)
            }
        }
    }
}

/// Internal memory region access cycles (BIOS, EWRAM, IWRAM, I/O, palette, VRAM, OAM).
pub fn internal_cycles(addr: u32, width: Width) -> u32 {
    let region = (addr >> 24) & 0xFF;
    match region {
        // EWRAM
        0x02 => match width {
            Width::Byte | Width::Half => 3,
            Width::Word => 6,
        },
        // Palette / VRAM
        0x05 | 0x06 => match width {
            Width::Byte | Width::Half => 1,
            Width::Word => 2,
        },
        // BIOS, IWRAM, I/O, OAM
        0x00 | 0x03 | 0x04 | 0x07 => 1,
        _ => 1,
    }
}

/// WS0 first (N) and second (S) wait values for debug fields.
/// Reset WAITCNT (0) is n=4, s=2.
pub fn ws0_ns(waitcnt: u16) -> (u32, u32) {
    (ws0_n(waitcnt), ws0_s(waitcnt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waitcnt0_ws0_half_n_and_s() {
        assert_eq!(rom_cycles(0, 0x0800_0000, Width::Half, false), 4);
        assert_eq!(rom_cycles(0, 0x0800_0000, Width::Half, true), 2);
    }

    #[test]
    fn ws0_first_code2_second_set() {
        // bits 2-3 = 2 (N=2), bit 4 set (S=1)
        let waitcnt = (2u16 << 2) | (1 << 4);
        assert_eq!(ws0_ns(waitcnt), (2, 1));
        assert_eq!(rom_cycles(waitcnt, 0x0800_0000, Width::Half, false), 2);
        assert_eq!(rom_cycles(waitcnt, 0x0800_0000, Width::Half, true), 1);
    }

    #[test]
    fn waitcnt0_ws0_word_n_and_s() {
        // 32-bit Game Pak: N+S (non-seq) or S+S (seq). WAITCNT 0 → N=4 S=2.
        assert_eq!(rom_cycles(0, 0x0800_0000, Width::Word, false), 6);
        assert_eq!(rom_cycles(0, 0x0800_0000, Width::Word, true), 4);
    }

    #[test]
    fn sram_waitcnt0_and_code3() {
        assert_eq!(sram_cycles(0), 4);
        assert_eq!(sram_cycles(3), 8);
    }

    #[test]
    fn ewram_word_and_iwram_word() {
        assert_eq!(internal_cycles(0x0200_0000, Width::Word), 6);
        assert_eq!(internal_cycles(0x0300_0000, Width::Word), 1);
    }
}
