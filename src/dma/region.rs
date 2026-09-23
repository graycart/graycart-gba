//! DMA address classes. Page 8 fills this in.
//!
//! Cited: GBATEK DMA Transfers.
//! <https://problemkaputt.de/gbatek.htm>

/// Address class for a DMA transfer after region checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Internal,
    GamePak,
}

const SRAM_START: u32 = 0x0E00_0000;
const SRAM_END: u32 = 0x0FFF_FFFF;
const GAME_PAK_START: u32 = 0x0800_0000;
const GAME_PAK_END: u32 = 0x0DFF_FFFF;

const SRAM_REJECT: &str = "gba-debug: sram-dma: rejected";
const DMA0_GAMEPAK_WARN: &str = "gba-debug: warn dma gamepak channel=0";

/// Exact debug line used when SRAM is touched by DMA.
pub fn sram_reject_line() -> &'static str {
    SRAM_REJECT
}

fn in_sram(addr: u32) -> bool {
    (SRAM_START..=SRAM_END).contains(&addr)
}

fn in_game_pak(addr: u32) -> bool {
    (GAME_PAK_START..=GAME_PAK_END).contains(&addr)
}

/// Classify DMA source/destination addresses for a channel.
///
/// - `Err(sram_reject_line())` if `src` or `dst` is in SRAM.
/// - `Err("gba-debug: warn dma gamepak channel=0")` if channel 0 touches the Game Pak.
/// - `Ok(GamePak)` if channel != 0 and `src` or `dst` is Game Pak.
/// - `Ok(Internal)` otherwise.
pub fn access(channel: usize, src: u32, dst: u32) -> Result<Kind, &'static str> {
    if in_sram(src) || in_sram(dst) {
        return Err(sram_reject_line());
    }

    let touches_game_pak = in_game_pak(src) || in_game_pak(dst);
    if touches_game_pak {
        if channel == 0 {
            return Err(DMA0_GAMEPAK_WARN);
        }
        return Ok(Kind::GamePak);
    }

    Ok(Kind::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iwram_to_iwram_channel0_is_internal() {
        assert_eq!(access(0, 0x0300_0000, 0x0300_1000), Ok(Kind::Internal));
    }

    #[test]
    fn rom_to_iwram_channel3_is_gamepak() {
        assert_eq!(access(3, 0x0800_0000, 0x0300_0000), Ok(Kind::GamePak));
    }

    #[test]
    fn rom_to_iwram_channel0_warns() {
        assert_eq!(access(0, 0x0800_0000, 0x0300_0000), Err(DMA0_GAMEPAK_WARN));
    }

    #[test]
    fn sram_source_on_channel3_rejected() {
        assert_eq!(access(3, 0x0E00_0000, 0x0300_0000), Err(sram_reject_line()));
        assert_eq!(sram_reject_line(), "gba-debug: sram-dma: rejected");
    }

    #[test]
    fn sram_dest_on_channel3_rejected() {
        assert_eq!(
            access(3, 0x0300_0000, 0x0E00_0000),
            Err("gba-debug: sram-dma: rejected")
        );
    }

    #[test]
    fn eeprom_sram_mirror_at_0f_rejected() {
        assert_eq!(access(3, 0x0F00_0000, 0x0300_0000), Err(sram_reject_line()));
        assert_eq!(
            access(3, 0x0300_0000, 0x0F00_0000),
            Err("gba-debug: sram-dma: rejected")
        );
    }
}
