//! Flash save chip (64 KiB / 128 KiB).
//!
//! Cited: GBATEK Cartridges, flash.
//! <https://problemkaputt.de/gbatek.htm>
//!
//! Command sequences match jsmolka `save/flash.inc` / `flash.asm` / `flash128.asm`
//! (Macronix-style AA@5555, 55@2AAA, then command).

const BANK_SIZE: usize = 0x10000;
const SECTOR_SIZE: usize = 0x1000;

const ADDR_5555: u32 = 0x5555;
const ADDR_2AAA: u32 = 0x2AAA;

/// Macronix 64K: manufacturer C2h, device 1Ch (GBATEK ID 1CC2h).
const ID_MAN_64: u8 = 0xC2;
const ID_DEV_64: u8 = 0x1C;
/// Macronix 128K: manufacturer C2h, device 09h (GBATEK ID 09C2h).
const ID_MAN_128: u8 = 0xC2;
const ID_DEV_128: u8 = 0x09;

/// Flash save capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlashSize {
    /// 64 KiB, one bank.
    K64,
    /// 128 KiB, two 64 KiB banks.
    K128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Ready,
    SawAa,
    ExpectCommand,
    ExpectArgument,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArgumentKind {
    Program,
    Bank,
}

/// GBA flash save memory.
#[derive(Debug)]
pub struct Flash {
    size: FlashSize,
    data: Vec<u8>,
    bank: u8,
    phase: Phase,
    id_mode: bool,
    erase_primed: bool,
    argument: ArgumentKind,
}

impl Flash {
    /// Create an erased flash chip of the given size.
    pub fn new(size: FlashSize) -> Self {
        let len = match size {
            FlashSize::K64 => BANK_SIZE,
            FlashSize::K128 => BANK_SIZE * 2,
        };
        Self {
            size,
            data: vec![0xFF; len],
            bank: 0,
            phase: Phase::Ready,
            id_mode: false,
            erase_primed: false,
            argument: ArgumentKind::Program,
        }
    }

    /// Read a byte. `addr` is masked to the low 16 bits of the SRAM region.
    pub fn read(&mut self, addr: u32) -> u8 {
        let offset = (addr & 0xFFFF) as usize;
        if self.id_mode {
            return match offset {
                0 => self.manufacturer(),
                1 => self.device(),
                _ => self.data[self.physical(offset)],
            };
        }
        self.data[self.physical(offset)]
    }

    /// Write a byte / command. `addr` is masked to the low 16 bits of the SRAM region.
    pub fn write(&mut self, addr: u32, value: u8) {
        let addr = addr & 0xFFFF;

        match self.phase {
            Phase::ExpectArgument => {
                match self.argument {
                    ArgumentKind::Program => {
                        let idx = self.physical((addr & 0xFFFF) as usize);
                        self.data[idx] = value;
                    }
                    ArgumentKind::Bank => {
                        if self.size == FlashSize::K128 {
                            self.bank = value & 1;
                        }
                        // 64K: bank switch is ignored (argument still consumed).
                    }
                }
                self.phase = Phase::Ready;
            }
            Phase::Ready => {
                if addr == ADDR_5555 && value == 0xAA {
                    self.phase = Phase::SawAa;
                }
            }
            Phase::SawAa => {
                if addr == ADDR_2AAA && value == 0x55 {
                    self.phase = Phase::ExpectCommand;
                } else {
                    self.phase = Phase::Ready;
                }
            }
            Phase::ExpectCommand => {
                self.handle_command(addr, value);
            }
        }
    }

    /// Backing store (all banks).
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    /// Replace backing store from `data` (padded with 0xFF / truncated to chip size).
    pub fn load_bytes(&mut self, data: &[u8]) {
        self.data.fill(0xFF);
        let n = data.len().min(self.data.len());
        self.data[..n].copy_from_slice(&data[..n]);
    }

    fn manufacturer(&self) -> u8 {
        match self.size {
            FlashSize::K64 => ID_MAN_64,
            FlashSize::K128 => ID_MAN_128,
        }
    }

    fn device(&self) -> u8 {
        match self.size {
            FlashSize::K64 => ID_DEV_64,
            FlashSize::K128 => ID_DEV_128,
        }
    }

    fn physical(&self, offset: usize) -> usize {
        let offset = offset & 0xFFFF;
        match self.size {
            FlashSize::K64 => offset,
            FlashSize::K128 => (self.bank as usize) * BANK_SIZE + offset,
        }
    }

    fn handle_command(&mut self, addr: u32, value: u8) {
        if addr == ADDR_5555 {
            match value {
                0x90 => {
                    self.id_mode = true;
                    self.erase_primed = false;
                    self.phase = Phase::Ready;
                }
                0xF0 => {
                    self.id_mode = false;
                    self.erase_primed = false;
                    self.phase = Phase::Ready;
                }
                0x80 => {
                    self.erase_primed = true;
                    self.phase = Phase::Ready;
                }
                0x10 => {
                    if self.erase_primed {
                        self.data.fill(0xFF);
                    }
                    self.erase_primed = false;
                    self.phase = Phase::Ready;
                }
                0xA0 => {
                    self.argument = ArgumentKind::Program;
                    self.erase_primed = false;
                    self.phase = Phase::ExpectArgument;
                }
                0xB0 => {
                    self.argument = ArgumentKind::Bank;
                    self.erase_primed = false;
                    self.phase = Phase::ExpectArgument;
                }
                _ => {
                    self.erase_primed = false;
                    self.phase = Phase::Ready;
                }
            }
            return;
        }

        // Sector erase: after 80h + AA/55, write 30h at the sector base.
        if self.erase_primed && value == 0x30 {
            let base = (addr as usize) & 0xF000;
            let start = self.physical(base);
            for b in &mut self.data[start..start + SECTOR_SIZE] {
                *b = 0xFF;
            }
            self.erase_primed = false;
            self.phase = Phase::Ready;
            return;
        }

        self.erase_primed = false;
        self.phase = Phase::Ready;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(flash: &mut Flash, command: u8) {
        flash.write(ADDR_5555, 0xAA);
        flash.write(ADDR_2AAA, 0x55);
        flash.write(ADDR_5555, command);
    }

    #[test]
    fn erased_64k_reads_ff() {
        let mut flash = Flash::new(FlashSize::K64);
        assert_eq!(flash.bytes().len(), BANK_SIZE);
        assert_eq!(flash.read(0), 0xFF);
        assert_eq!(flash.read(0x1234), 0xFF);
        assert_eq!(flash.read(0x0E00_FFFF), 0xFF);
    }

    #[test]
    fn id_mode_macronix_then_exit() {
        let mut flash = Flash::new(FlashSize::K64);
        cmd(&mut flash, 0x90);
        assert_eq!(flash.read(0), ID_MAN_64);
        assert_eq!(flash.read(1), ID_DEV_64);

        cmd(&mut flash, 0xF0);
        // Back to data mode: erased byte, not the manufacturer ID.
        assert_eq!(flash.read(0), 0xFF);

        let mut flash128 = Flash::new(FlashSize::K128);
        cmd(&mut flash128, 0x90);
        assert_eq!(flash128.read(0), ID_MAN_128);
        assert_eq!(flash128.read(1), ID_DEV_128);
        cmd(&mut flash128, 0xF0);
        assert_eq!(flash128.read(0), 0xFF);
    }

    #[test]
    fn program_byte_then_chip_erase() {
        let mut flash = Flash::new(FlashSize::K64);
        cmd(&mut flash, 0xA0);
        flash.write(0x0042, 0x5A);
        assert_eq!(flash.read(0x0042), 0x5A);

        cmd(&mut flash, 0x80);
        cmd(&mut flash, 0x10);
        assert_eq!(flash.read(0x0042), 0xFF);
    }

    #[test]
    fn bank_switch_128k_isolates_banks() {
        let mut flash = Flash::new(FlashSize::K128);
        let offset = 0x0100u32;

        cmd(&mut flash, 0xA0);
        flash.write(offset, 0x11);
        assert_eq!(flash.read(offset), 0x11);

        // Switch to bank 1.
        cmd(&mut flash, 0xB0);
        flash.write(0, 1);
        assert_eq!(flash.read(offset), 0xFF);

        // Switch back to bank 0.
        cmd(&mut flash, 0xB0);
        flash.write(0, 0);
        assert_eq!(flash.read(offset), 0x11);
    }
}
