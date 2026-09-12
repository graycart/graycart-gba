//! Flash backup FSM (64 KiB / 128 KiB banked).
//!
//! Cited: GBATEK — GBA Cart Backup Flash ROM
//!   https://problemkaputt.de/gbatek.htm
//! Cited: jsmolka/gba-tests `save/flash.asm` (MIT) — command sequences
//!   https://github.com/jsmolka/gba-tests
//! Research: Project store `docs/graycart-gba/06-cart-bios-saves.md` §5.2
//! Note: SST/Macronix-class command set; erase completes immediately (soft timing).

use crate::bus::mirror::sram_window_offset;

/// 64 KiB (512 Kbit) Flash.
pub const FLASH_64K: usize = 64 * 1024;
/// 128 KiB (1 Mbit) Flash — two 64 KiB banks.
pub const FLASH_128K: usize = 128 * 1024;

const CMD_ADDR_5555: u32 = 0x0E00_5555;
const CMD_ADDR_2AAA: u32 = 0x0E00_2AAA;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ready,
    AfterAa,
    After55,
    /// After `AA/55/80` — expect erase confirm.
    EraseSetup,
    /// After erase-setup `AA` …
    EraseAfterAa,
    /// After erase-setup `AA/55` — expect `10` chip or `30` sector.
    EraseAfter55,
    /// Byte program: next write is data.
    Program,
    /// Bank select (128K): next write at `0E000000` is bank.
    BankSelect,
    IdMode,
}

/// Flash chip identity returned in ID mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlashId {
    pub manufacturer: u8,
    pub device: u8,
}

impl FlashId {
    /// SST 64K — GBATEK `D4BFh` → man=`BFh` device=`D4h` at even/odd.
    pub const SST_64K: Self = Self {
        manufacturer: 0xBF,
        device: 0xD4,
    };
    /// Macronix 128K banked — common `C2` / `09` pair used by many emus.
    pub const MACRONIX_128K: Self = Self {
        manufacturer: 0xC2,
        device: 0x09,
    };
}

/// Flash backup memory with command FSM.
#[derive(Debug, Clone)]
pub struct Flash {
    pub data: Vec<u8>,
    pub bank: u8,
    pub size: usize,
    id: FlashId,
    phase: Phase,
}

impl Flash {
    #[must_use]
    pub fn new_64k() -> Self {
        Self::with_size(FLASH_64K, FlashId::SST_64K)
    }

    #[must_use]
    pub fn new_128k() -> Self {
        Self::with_size(FLASH_128K, FlashId::MACRONIX_128K)
    }

    fn with_size(size: usize, id: FlashId) -> Self {
        Self {
            data: vec![0xFF; size],
            bank: 0,
            size,
            id,
            phase: Phase::Ready,
        }
    }

    fn bank_base(&self) -> usize {
        if self.size > FLASH_64K {
            usize::from(self.bank & 1) * FLASH_64K
        } else {
            0
        }
    }

    fn offset(&self, addr: u32) -> usize {
        self.bank_base() + (sram_window_offset(addr) & (FLASH_64K - 1))
    }

    #[must_use]
    pub fn read8(&self, addr: u32) -> u8 {
        if self.phase == Phase::IdMode {
            let off = sram_window_offset(addr) & 1;
            return if off == 0 {
                self.id.manufacturer
            } else {
                self.id.device
            };
        }
        let i = self.offset(addr);
        self.data.get(i).copied().unwrap_or(0xFF)
    }

    pub fn write8(&mut self, addr: u32, value: u8) {
        let at_5555 = (addr & 0xFFFF) == (CMD_ADDR_5555 & 0xFFFF);
        let at_2aaa = (addr & 0xFFFF) == (CMD_ADDR_2AAA & 0xFFFF);
        let at_base = (addr & 0xFFFF) == 0;

        match self.phase {
            Phase::Program => {
                let i = self.offset(addr);
                if let Some(slot) = self.data.get_mut(i) {
                    // Flash program can only clear bits.
                    *slot &= value;
                }
                self.phase = Phase::Ready;
            }
            Phase::BankSelect => {
                if at_base {
                    self.bank = value & 1;
                }
                self.phase = Phase::Ready;
            }
            Phase::Ready => {
                if at_5555 && value == 0xAA {
                    self.phase = Phase::AfterAa;
                }
            }
            Phase::AfterAa => {
                if at_2aaa && value == 0x55 {
                    self.phase = Phase::After55;
                } else {
                    self.phase = Phase::Ready;
                }
            }
            Phase::After55 => {
                if at_5555 {
                    match value {
                        0x90 => self.phase = Phase::IdMode,
                        0xA0 => self.phase = Phase::Program,
                        0x80 => self.phase = Phase::EraseSetup,
                        0xB0 if self.size > FLASH_64K => self.phase = Phase::BankSelect,
                        0xF0 => self.phase = Phase::Ready, // reset
                        _ => self.phase = Phase::Ready,
                    }
                } else {
                    self.phase = Phase::Ready;
                }
            }
            Phase::IdMode => {
                // Exit ID: AA/55/F0 or single F0.
                if at_5555 && value == 0xAA {
                    self.phase = Phase::AfterAa;
                } else if value == 0xF0 {
                    self.phase = Phase::Ready;
                }
            }
            Phase::EraseSetup => {
                if at_5555 && value == 0xAA {
                    self.phase = Phase::EraseAfterAa;
                } else {
                    self.phase = Phase::Ready;
                }
            }
            Phase::EraseAfterAa => {
                if at_2aaa && value == 0x55 {
                    self.phase = Phase::EraseAfter55;
                } else {
                    self.phase = Phase::Ready;
                }
            }
            Phase::EraseAfter55 => {
                if at_5555 && value == 0x10 {
                    // Chip erase (current bank for 128K; whole chip for 64K).
                    if self.size > FLASH_64K {
                        let base = self.bank_base();
                        for b in &mut self.data[base..base + FLASH_64K] {
                            *b = 0xFF;
                        }
                    } else {
                        self.data.fill(0xFF);
                    }
                    self.phase = Phase::Ready;
                } else if value == 0x30 {
                    // Sector erase — 4 KiB sector containing `addr`.
                    let off = self.offset(addr) & !0xFFF;
                    let end = (off + 0x1000).min(self.data.len());
                    for b in &mut self.data[off..end] {
                        *b = 0xFF;
                    }
                    self.phase = Phase::Ready;
                } else {
                    self.phase = Phase::Ready;
                }
            }
        }
    }

    #[must_use]
    pub fn to_sav(&self) -> Vec<u8> {
        self.data.clone()
    }

    pub fn from_sav_64k(bytes: &[u8]) -> Self {
        let mut f = Self::new_64k();
        let n = bytes.len().min(f.data.len());
        f.data[..n].copy_from_slice(&bytes[..n]);
        f
    }

    pub fn from_sav_128k(bytes: &[u8]) -> Self {
        let mut f = Self::new_128k();
        let n = bytes.len().min(f.data.len());
        f.data[..n].copy_from_slice(&bytes[..n]);
        f
    }
}
