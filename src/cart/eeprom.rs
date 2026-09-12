//! EEPROM bitstream (512 B / 8 KiB) — Game Pak ROM-bus serial protocol.
//!
//! Cited: GBATEK — GBA Cart Backup EEPROM
//!   https://problemkaputt.de/gbatek.htm
//! Research: Project store `docs/graycart-gba/06-cart-bios-saves.md` §5.3
//! Note: unit-tested storage + serial helpers; DMA3 ROM-bus wiring is glue-owned.

/// 512-byte EEPROM (4Kbit).
pub const EEPROM_512: usize = 512;
/// 8 KiB EEPROM (64Kbit).
pub const EEPROM_8K: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EepromSize {
    B512,
    K8,
}

impl EepromSize {
    #[must_use]
    pub const fn bytes(self) -> usize {
        match self {
            Self::B512 => EEPROM_512,
            Self::K8 => EEPROM_8K,
        }
    }

    /// Address bits after the 2-bit command (6 for 512B, 14 for 8K).
    #[must_use]
    pub const fn addr_bits(self) -> u32 {
        match self {
            Self::B512 => 6,
            Self::K8 => 14,
        }
    }

    #[must_use]
    pub const fn block_count(self) -> usize {
        self.bytes() / 8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    WaitStart,
    Cmd1 { first: u8 },
    Addr { cmd: u8, got: u32, value: u32 },
    WriteData { block: usize, got: u32, value: u64 },
    WriteStop { block: usize, value: u64 },
    ReadStop { block: usize },
    ReadOut { block: usize, bit: u32 },
}

/// Serial EEPROM image (8-byte blocks).
#[derive(Debug, Clone)]
pub struct Eeprom {
    pub data: Vec<u8>,
    pub size: EepromSize,
    /// Serial output bit sampled by ROM reads (1 = ready / high when idle).
    pub out_bit: u8,
    phase: Phase,
}

impl Eeprom {
    #[must_use]
    pub fn new(size: EepromSize) -> Self {
        Self {
            data: vec![0xFF; size.bytes()],
            size,
            out_bit: 1,
            phase: Phase::WaitStart,
        }
    }

    /// Direct block write (test / sav helpers). `block` is 0..block_count.
    pub fn write_block(&mut self, block: usize, bytes: &[u8; 8]) {
        let base = block.saturating_mul(8);
        if base + 8 <= self.data.len() {
            self.data[base..base + 8].copy_from_slice(bytes);
        }
    }

    /// Direct block read.
    #[must_use]
    pub fn read_block(&self, block: usize) -> [u8; 8] {
        let mut out = [0xFF; 8];
        let base = block.saturating_mul(8);
        if base + 8 <= self.data.len() {
            out.copy_from_slice(&self.data[base..base + 8]);
        }
        out
    }

    /// Ingest one serial bit (LSB of a Game Pak ROM write).
    pub fn ingest_bit(&mut self, bit: u8) {
        let bit = bit & 1;
        match self.phase {
            Phase::WaitStart => {
                if bit == 1 {
                    // Leading 1 is the first command bit (write=10, read=11).
                    self.phase = Phase::Cmd1 { first: 1 };
                }
            }
            Phase::Cmd1 { first } => {
                let cmd = (first << 1) | bit;
                self.phase = Phase::Addr {
                    cmd,
                    got: 0,
                    value: 0,
                };
            }
            Phase::Addr { cmd, got, value } => {
                let value = (value << 1) | u32::from(bit);
                let got = got + 1;
                if got == self.size.addr_bits() {
                    let block = (value as usize).min(self.size.block_count().saturating_sub(1));
                    if cmd == 0b10 {
                        self.phase = Phase::WriteData {
                            block,
                            got: 0,
                            value: 0,
                        };
                    } else if cmd == 0b11 {
                        self.phase = Phase::ReadStop { block };
                    } else {
                        self.phase = Phase::WaitStart;
                        self.out_bit = 1;
                    }
                } else {
                    self.phase = Phase::Addr { cmd, got, value };
                }
            }
            Phase::WriteData { block, got, value } => {
                let value = (value << 1) | u64::from(bit);
                let got = got + 1;
                if got == 64 {
                    self.phase = Phase::WriteStop { block, value };
                } else {
                    self.phase = Phase::WriteData { block, got, value };
                }
            }
            Phase::WriteStop { block, value } => {
                // Stop bit (expected 0); commit either way.
                let mut bytes = [0u8; 8];
                for (i, b) in bytes.iter_mut().enumerate() {
                    *b = ((value >> (56 - i * 8)) & 0xFF) as u8;
                }
                self.write_block(block, &bytes);
                self.phase = Phase::WaitStart;
                self.out_bit = 1;
                let _ = bit;
            }
            Phase::ReadStop { block } => {
                let _ = bit;
                self.phase = Phase::ReadOut { block, bit: 0 };
                self.out_bit = 0;
            }
            Phase::ReadOut { .. } => {
                // Abort read on unexpected write.
                self.phase = Phase::WaitStart;
                self.out_bit = 1;
            }
        }
    }

    /// Clock one output bit during a read stream (ROM bus read).
    pub fn clock_read(&mut self) -> u8 {
        match self.phase {
            Phase::ReadOut { block, bit: idx } => {
                let out = if idx < 4 {
                    0
                } else {
                    let data_bit = idx - 4;
                    if data_bit < 64 {
                        let bytes = self.read_block(block);
                        let byte = bytes[(data_bit as usize) / 8];
                        (byte >> (7 - (data_bit % 8))) & 1
                    } else {
                        1
                    }
                };
                let next = idx + 1;
                if next >= 4 + 64 {
                    self.phase = Phase::WaitStart;
                    self.out_bit = 1;
                } else {
                    self.phase = Phase::ReadOut { block, bit: next };
                    self.out_bit = out;
                }
                out
            }
            _ => {
                self.out_bit = 1;
                1
            }
        }
    }

    #[must_use]
    pub fn to_sav(&self) -> Vec<u8> {
        self.data.clone()
    }

    pub fn from_sav(size: EepromSize, bytes: &[u8]) -> Self {
        let mut e = Self::new(size);
        let n = bytes.len().min(e.data.len());
        e.data[..n].copy_from_slice(&bytes[..n]);
        e
    }
}
