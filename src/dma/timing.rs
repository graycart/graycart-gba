//! DMA start timing. Page 8 fills this in.
//!
//! Cited: GBATEK DMA Transfers.
//! <https://problemkaputt.de/gbatek.htm>

/// FIFO special always copies 4 units.
pub const FIFO_UNITS: u32 = 4;

/// Why a DMA transfer is allowed to start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Reason {
    Immediate,
    VBlank,
    HBlank,
    Fifo,
}

/// None when enable is clear, or when timing is special on a channel other than 1 or 2.
pub fn reason(channel: usize, cnt_h: u16) -> Option<Reason> {
    if cnt_h & (1 << 15) == 0 {
        return None;
    }
    match (cnt_h >> 12) & 0b11 {
        0 => Some(Reason::Immediate),
        1 => Some(Reason::VBlank),
        2 => Some(Reason::HBlank),
        3 if channel == 1 || channel == 2 => Some(Reason::Fifo),
        _ => None,
    }
}

pub fn repeat(cnt_h: u16) -> bool {
    cnt_h & (1 << 9) != 0
}

pub fn width32(cnt_h: u16) -> bool {
    cnt_h & (1 << 10) != 0
}

pub fn src_ctrl(cnt_h: u16) -> u8 {
    ((cnt_h >> 7) & 0b11) as u8
}

pub fn dst_ctrl(cnt_h: u16) -> u8 {
    ((cnt_h >> 5) & 0b11) as u8
}

pub fn irq_on_end(cnt_h: u16) -> bool {
    cnt_h & (1 << 14) != 0
}

/// IF bit for this channel. DMA0 is bit 8.
pub fn irq_mask(channel: usize) -> u16 {
    1 << (8 + channel)
}

/// CNT_L == 0 means 0x4000 units for channels 0-2 and 0x10000 units for channel 3.
/// Channels 0–2 only use the low 14 bits of CNT_L.
pub fn unit_count(channel: usize, cnt_l: u16) -> u32 {
    let count = if channel == 3 { cnt_l } else { cnt_l & 0x3FFF };
    if count != 0 {
        return u32::from(count);
    }
    if channel == 3 { 0x1_0000 } else { 0x4000 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_clear_reason_none() {
        assert_eq!(reason(0, 0), None);
        assert_eq!(reason(1, 0x3000), None); // timing special, enable clear
    }

    #[test]
    fn timing_0_1_2_with_enable() {
        let enable = 1u16 << 15;
        assert_eq!(reason(0, enable), Some(Reason::Immediate));
        assert_eq!(reason(0, enable | (1 << 12)), Some(Reason::VBlank));
        assert_eq!(reason(0, enable | (2 << 12)), Some(Reason::HBlank));
    }

    #[test]
    fn special_timing_fifo_only_ch1_ch2() {
        let enable_special = (1u16 << 15) | (3 << 12);
        assert_eq!(reason(1, enable_special), Some(Reason::Fifo));
        assert_eq!(reason(2, enable_special), Some(Reason::Fifo));
        assert_eq!(reason(0, enable_special), None);
        assert_eq!(reason(3, enable_special), None);
    }

    #[test]
    fn unit_count_zero_and_nonzero() {
        assert_eq!(unit_count(0, 0), 0x4000);
        assert_eq!(unit_count(1, 0), 0x4000);
        assert_eq!(unit_count(2, 0), 0x4000);
        assert_eq!(unit_count(3, 0), 0x1_0000);
        assert_eq!(unit_count(0, 42), 42);
        assert_eq!(unit_count(3, 100), 100);
        // DMA0–2 are 14-bit: bit 15 of CNT_L is ignored.
        assert_eq!(unit_count(0, 0x8001), 1);
        assert_eq!(unit_count(2, 0xC000), 0x4000); // masked to 0 → full count
        assert_eq!(unit_count(3, 0x8001), 0x8001);
    }

    #[test]
    fn irq_mask_dma0_and_dma3() {
        assert_eq!(irq_mask(0), 0x0100);
        assert_eq!(irq_mask(3), 0x0800);
    }
}
