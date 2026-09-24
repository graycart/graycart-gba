//! EEPROM save chip.
//!
//! Cited: GBATEK Cartridges, EEPROM.
//! <https://problemkaputt.de/gbatek.htm>
//!
//! Size is learned from the first transfer's address width, not from the ROM
//! title: 6 address bits → 512 bytes (usual setup DMA of 9 halfwords), 14
//! address bits → 8 KiB (usual setup DMA of 17 halfwords). After that first
//! transfer, the size sticks.
//!
//! Each DMA halfword carries one serial bit in bit 0, MSB first. Command
//! `0b10` + address + 64 data bits + stop writes 8 bytes. Command `0b11` +
//! address + stop prepares a read; a later DMA clocks out 4 ignored bits and
//! 64 data bits via [`Eeprom::read_bit`].

/// Serial EEPROM backup (512 bytes or 8 KiB).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Eeprom {
    /// Backing store; empty until the first size-revealing transfer (or load).
    data: Vec<u8>,
    /// `6` → 512 bytes, `14` → 8192 bytes.
    addr_bits: Option<u8>,
    /// Bits shifted in during the current write-direction DMA.
    rx: Vec<u8>,
    /// Pending read reply: 4 dummy bits then 64 data bits, MSB first.
    read_out: Option<Vec<u8>>,
    read_pos: usize,
}

impl Eeprom {
    /// Create an unsized chip. The first transfer selects 512 B or 8 KiB.
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            addr_bits: None,
            rx: Vec::new(),
            read_out: None,
            read_pos: 0,
        }
    }

    /// Detected size in bytes, or `None` until the first informative transfer.
    pub fn size_bytes(&self) -> Option<usize> {
        self.addr_bits.map(|b| if b == 6 { 512 } else { 8192 })
    }

    /// Feed one halfword of a DMA write into the chip (bit 0 is the data bit).
    pub fn write_bit(&mut self, half: u16) {
        self.rx.push((half & 1) as u8);
    }

    /// Clock one bit out. Only meaningful after a read command has been
    /// shifted in. Returns the bit in bit 0.
    pub fn read_bit(&mut self) -> u16 {
        let len = match self.read_out.as_ref() {
            Some(out) => out.len(),
            None => return 1, // Idle / post-write ready (GBATEK ready = 1).
        };
        if self.read_pos >= len {
            self.read_out = None;
            self.read_pos = 0;
            return 1;
        }
        let bit = self.read_out.as_ref().unwrap()[self.read_pos] as u16;
        self.read_pos += 1;
        if self.read_pos >= len {
            self.read_out = None;
            self.read_pos = 0;
        }
        bit
    }

    /// End the current DMA. Call when the DMA completes so a short stream is
    /// parsed (read/write setup of 9 or 17 halfwords, or a full write).
    pub fn end_transfer(&mut self) {
        let n = self.rx.len();
        if n == 0 {
            return;
        }

        if self.addr_bits.is_none() {
            let inferred = match n {
                9 | 73 => Some(6u8),
                17 | 81 => Some(14u8),
                _ => None,
            };
            if let Some(bits) = inferred {
                self.settle_size(bits);
            }
        }

        let Some(addr_bits) = self.addr_bits else {
            self.rx.clear();
            return;
        };

        if n < 2 {
            self.rx.clear();
            return;
        }

        let cmd = (self.rx[0] << 1) | self.rx[1];
        let addr_width = addr_bits as usize;

        match cmd {
            0b10 => {
                // Write: 2 + n + 64 + 1
                let need = 2 + addr_width + 64 + 1;
                if n >= need {
                    let addr = bits_to_u32(&self.rx[2..2 + addr_width]);
                    let data_bits: Vec<u8> = self.rx[2 + addr_width..2 + addr_width + 64].to_vec();
                    self.program_block(addr, &data_bits);
                }
            }
            0b11 => {
                // Read request: 2 + n + 1
                let need = 2 + addr_width + 1;
                if n >= need {
                    let addr = bits_to_u32(&self.rx[2..2 + addr_width]);
                    self.begin_read(addr);
                }
            }
            _ => {}
        }

        self.rx.clear();
    }

    /// Raw save bytes (erased = `0xFF`). Empty until size is known.
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    /// Replace save contents. Any non-empty buffer locks size when still unset:
    /// `len > 512` → 8 KiB, otherwise 512 bytes. Shorter/longer files pad or
    /// truncate; an empty buffer is a no-op (do not wipe an unknown chip and
    /// later flush over a real sidecar).
    pub fn load_bytes(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        if self.addr_bits.is_none() {
            let bits = if data.len() > 512 { 14 } else { 6 };
            self.settle_size(bits);
        }
        let len = self.data.len().min(data.len());
        self.data[..len].copy_from_slice(&data[..len]);
        if len < self.data.len() {
            self.data[len..].fill(0xFF);
        }
    }

    fn settle_size(&mut self, addr_bits: u8) {
        if self.addr_bits.is_some() {
            return;
        }
        let size = if addr_bits == 6 { 512 } else { 8192 };
        self.addr_bits = Some(addr_bits);
        self.data = vec![0xFF; size];
    }

    fn program_block(&mut self, block: u32, data_bits: &[u8]) {
        let Some(size) = self.size_bytes() else {
            return;
        };
        // 8K chips expose 14 address bits but only the lower 10 are used.
        let mask = (size / 8 - 1) as u32;
        let off = ((block & mask) as usize) * 8;
        if off + 8 > self.data.len() {
            return;
        }
        for byte_i in 0..8 {
            let mut b = 0u8;
            for bit_i in 0..8 {
                b <<= 1;
                b |= data_bits[byte_i * 8 + bit_i] & 1;
            }
            self.data[off + byte_i] = b;
        }
    }

    fn begin_read(&mut self, block: u32) {
        let Some(size) = self.size_bytes() else {
            return;
        };
        let mask = (size / 8 - 1) as u32;
        let off = ((block & mask) as usize) * 8;
        let mut out = Vec::with_capacity(68);
        // 4 ignored bits, then 64 data bits MSB first.
        out.extend_from_slice(&[0, 0, 0, 0]);
        if off + 8 <= self.data.len() {
            for &byte in &self.data[off..off + 8] {
                for i in (0..8).rev() {
                    out.push((byte >> i) & 1);
                }
            }
        } else {
            out.extend(std::iter::repeat_n(1, 64));
        }
        self.read_out = Some(out);
        self.read_pos = 0;
    }
}

impl Default for Eeprom {
    fn default() -> Self {
        Self::new()
    }
}

fn bits_to_u32(bits: &[u8]) -> u32 {
    let mut v = 0u32;
    for &b in bits {
        v = (v << 1) | u32::from(b & 1);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Push `nbits` of `value`, MSB first, as halfwords with the bit in bit 0.
    fn push_u(bits: &mut Vec<u16>, value: u64, nbits: u32) {
        for i in (0..nbits).rev() {
            bits.push(((value >> i) & 1) as u16);
        }
    }

    fn push_bytes_msb(bits: &mut Vec<u16>, bytes: &[u8]) {
        for &b in bytes {
            push_u(bits, u64::from(b), 8);
        }
    }

    fn feed_write(e: &mut Eeprom, stream: &[u16]) {
        for &h in stream {
            e.write_bit(h);
        }
        e.end_transfer();
    }

    fn read_block(e: &mut Eeprom, addr_bits: u8, block: u32) -> [u8; 8] {
        let mut stream = Vec::new();
        push_u(&mut stream, 0b11, 2);
        push_u(&mut stream, u64::from(block), u32::from(addr_bits));
        push_u(&mut stream, 0, 1);
        feed_write(e, &stream);

        // 4 dummy + 64 data
        for _ in 0..4 {
            let _ = e.read_bit();
        }
        let mut out = [0u8; 8];
        for byte in &mut out {
            let mut b = 0u8;
            for _ in 0..8 {
                b = (b << 1) | (e.read_bit() as u8 & 1);
            }
            *byte = b;
        }
        out
    }

    #[test]
    fn nine_halfword_write_sets_512_and_programs_eight_bytes() {
        // Write stream for the 6-bit (9-halfword setup) size: 2+6+64+1 = 73.
        let payload = [0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x23, 0x45, 0x67];
        let mut stream = Vec::new();
        push_u(&mut stream, 0b10, 2);
        push_u(&mut stream, 0x15, 6); // block 0x15
        push_bytes_msb(&mut stream, &payload);
        push_u(&mut stream, 0, 1);
        assert_eq!(stream.len(), 73);

        let mut e = Eeprom::new();
        assert_eq!(e.size_bytes(), None);
        feed_write(&mut e, &stream);
        assert_eq!(e.size_bytes(), Some(512));
        assert_eq!(read_block(&mut e, 6, 0x15), payload);

        // Same size family: a 9-halfword setup alone also selects 512.
        let mut e2 = Eeprom::new();
        let mut setup = Vec::new();
        push_u(&mut setup, 0b11, 2);
        push_u(&mut setup, 0, 6);
        push_u(&mut setup, 0, 1);
        assert_eq!(setup.len(), 9);
        feed_write(&mut e2, &setup);
        assert_eq!(e2.size_bytes(), Some(512));
    }

    #[test]
    fn seventeen_halfword_write_sets_8192() {
        let mut stream = Vec::new();
        push_u(&mut stream, 0b11, 2);
        push_u(&mut stream, 0x0123, 14);
        push_u(&mut stream, 0, 1);
        assert_eq!(stream.len(), 17);

        let mut e = Eeprom::new();
        feed_write(&mut e, &stream);
        assert_eq!(e.size_bytes(), Some(8192));
        assert_eq!(e.bytes().len(), 8192);

        // Full write for the 14-bit size also sticks at 8192.
        let mut e2 = Eeprom::new();
        let payload = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let mut write = Vec::new();
        push_u(&mut write, 0b10, 2);
        push_u(&mut write, 0x0100, 14);
        push_bytes_msb(&mut write, &payload);
        push_u(&mut write, 0, 1);
        assert_eq!(write.len(), 81);
        feed_write(&mut e2, &write);
        assert_eq!(e2.size_bytes(), Some(8192));
        assert_eq!(read_block(&mut e2, 14, 0x0100), payload);
    }

    #[test]
    fn size_does_not_depend_on_title_string() {
        // new() takes no size; no title / game-code string is consulted.
        let mut a = Eeprom::new();
        let mut b = Eeprom::new();
        assert_eq!(a.size_bytes(), None);
        assert_eq!(b.size_bytes(), None);

        let mut nine = Vec::new();
        push_u(&mut nine, 0b11, 2);
        push_u(&mut nine, 0, 6);
        push_u(&mut nine, 0, 1);
        feed_write(&mut a, &nine);

        let mut seventeen = Vec::new();
        push_u(&mut seventeen, 0b11, 2);
        push_u(&mut seventeen, 0, 14);
        push_u(&mut seventeen, 0, 1);
        feed_write(&mut b, &seventeen);

        assert_eq!(a.size_bytes(), Some(512));
        assert_eq!(b.size_bytes(), Some(8192));
        // Size sticks after the first transfer.
        feed_write(&mut a, &seventeen);
        assert_eq!(a.size_bytes(), Some(512));
    }

    #[test]
    fn unwritten_bytes_are_ff() {
        let mut e = Eeprom::new();
        let mut stream = Vec::new();
        push_u(&mut stream, 0b10, 2);
        push_u(&mut stream, 0, 6);
        push_bytes_msb(&mut stream, &[0x00; 8]);
        push_u(&mut stream, 0, 1);
        feed_write(&mut e, &stream);

        assert_eq!(e.size_bytes(), Some(512));
        assert!(e.bytes()[8..].iter().all(|&b| b == 0xFF));
        assert_eq!(&e.bytes()[..8], &[0x00; 8]);
    }

    #[test]
    fn load_bytes_odd_lengths_settle_and_keep_prefix() {
        let mut small = Eeprom::new();
        small.load_bytes(&[0xA5; 100]);
        assert_eq!(small.size_bytes(), Some(512));
        assert_eq!(&small.bytes()[..100], &[0xA5; 100]);
        assert!(small.bytes()[100..].iter().all(|&b| b == 0xFF));

        let mut big = Eeprom::new();
        big.load_bytes(&[0x5A; 600]);
        assert_eq!(big.size_bytes(), Some(8192));
        assert_eq!(&big.bytes()[..600], &[0x5A; 600]);
        assert!(big.bytes()[600..].iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn load_bytes_empty_does_not_lock_size() {
        let mut e = Eeprom::new();
        e.load_bytes(&[]);
        assert_eq!(e.size_bytes(), None);
        assert!(e.bytes().is_empty());
    }
}
