//! Unit copy.
//!
//! Cited: GBATEK DMA Transfers.
//! <https://problemkaputt.de/gbatek.htm>

/// One DMA unit-copy job.
pub struct Copy {
    pub src: u32,
    pub dst: u32,
    /// Number of units. Caller already expanded a zero count.
    pub units: u32,
    pub width32: bool,
    /// 0 increment, 1 decrement, 2 fixed. Any other value: do not adjust (treat as fixed).
    pub src_ctrl: u8,
    /// 0 increment, 1 decrement, 2 fixed, 3 increment (reload is the caller's job; during the copy, 3 steps like 0).
    pub dst_ctrl: u8,
}

fn step(addr: u32, ctrl: u8, unit_size: u32) -> u32 {
    match ctrl {
        0 | 3 => addr.wrapping_add(unit_size),
        1 => addr.wrapping_sub(unit_size),
        _ => addr,
    }
}

/// Copy `units` values. `read`/`write` see the full 32-bit bus value; for 16-bit units only the low 16 bits are stored.
/// Returns the number of units copied.
/// 16-bit steps the address by 2. 32-bit steps by 4.
pub fn copy_units(
    job: &Copy,
    read: &mut dyn FnMut(u32) -> u32,
    write: &mut dyn FnMut(u32, u32),
) -> u32 {
    let unit_size = if job.width32 { 4 } else { 2 };
    let mut src = job.src;
    let mut dst = job.dst;

    for _ in 0..job.units {
        let value = read(src);
        let stored = if job.width32 { value } else { value & 0xffff };
        write(dst, stored);
        src = step(src, job.src_ctrl, unit_size);
        dst = step(dst, job.dst_ctrl, unit_size);
    }

    job.units
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_read(mem: &[u8]) -> impl FnMut(u32) -> u32 + '_ {
        move |addr| {
            let i = addr as usize;
            u32::from_le_bytes([
                mem.get(i).copied().unwrap_or(0),
                mem.get(i + 1).copied().unwrap_or(0),
                mem.get(i + 2).copied().unwrap_or(0),
                mem.get(i + 3).copied().unwrap_or(0),
            ])
        }
    }

    fn mem_write(mem: &mut [u8]) -> impl FnMut(u32, u32) + '_ {
        move |addr, value| {
            let i = addr as usize;
            let bytes = value.to_le_bytes();
            for (offset, byte) in bytes.iter().enumerate() {
                if let Some(slot) = mem.get_mut(i + offset) {
                    *slot = *byte;
                }
            }
        }
    }

    #[test]
    fn four_halfwords_both_increment() {
        let src = [0x11u8, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let mut dst = [0u8; 8];
        let job = Copy {
            src: 0,
            dst: 0,
            units: 4,
            width32: false,
            src_ctrl: 0,
            dst_ctrl: 0,
        };
        let mut read = mem_read(&src);
        {
            let mut write = mem_write(&mut dst);
            assert_eq!(copy_units(&job, &mut read, &mut write), 4);
        }
        assert_eq!(dst, src);
    }

    #[test]
    fn destination_fixed_last_write_wins() {
        let src = [0x11u8, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let mut dst = [0u8; 8];
        let mut writes: Vec<(u32, u32)> = Vec::new();
        let job = Copy {
            src: 0,
            dst: 0x10,
            units: 4,
            width32: false,
            src_ctrl: 0,
            dst_ctrl: 2,
        };
        let mut read = mem_read(&src);
        let mut write = |addr, value| {
            writes.push((addr, value));
            // Mirror into local buffer at offset 0 for inspection of last value.
            let bytes = (value as u16).to_le_bytes();
            dst[0] = bytes[0];
            dst[1] = bytes[1];
        };
        assert_eq!(copy_units(&job, &mut read, &mut write), 4);
        assert_eq!(writes.len(), 4);
        assert!(writes.iter().all(|(addr, _)| *addr == 0x10));
        assert_eq!(writes.last().unwrap().1, 0x8877);
        assert_eq!(&dst[0..2], &[0x77, 0x88]);
    }

    #[test]
    fn one_word_steps_source_by_four() {
        let src = [0x11u8, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let mut dst = [0u8; 8];
        let mut src_addrs = Vec::new();
        let job = Copy {
            src: 0,
            dst: 0,
            units: 2,
            width32: true,
            src_ctrl: 0,
            dst_ctrl: 0,
        };
        let mut read = |addr| {
            src_addrs.push(addr);
            mem_read(&src)(addr)
        };
        {
            let mut write = mem_write(&mut dst);
            assert_eq!(copy_units(&job, &mut read, &mut write), 2);
        }
        assert_eq!(src_addrs, vec![0, 4]);
        assert_eq!(&dst[0..8], &src[0..8]);
    }

    #[test]
    fn decrement_source_walks_backward() {
        // Source units at 6,4,2,0 when starting at 6 with halfword decrement.
        let src = [0x11u8, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let mut dst = [0u8; 8];
        let mut src_addrs = Vec::new();
        let job = Copy {
            src: 6,
            dst: 0,
            units: 4,
            width32: false,
            src_ctrl: 1,
            dst_ctrl: 0,
        };
        let mut read = |addr| {
            src_addrs.push(addr);
            mem_read(&src)(addr)
        };
        {
            let mut write = mem_write(&mut dst);
            assert_eq!(copy_units(&job, &mut read, &mut write), 4);
        }
        assert_eq!(src_addrs, vec![6, 4, 2, 0]);
        assert_eq!(dst, [0x77, 0x88, 0x55, 0x66, 0x33, 0x44, 0x11, 0x22]);
    }
}
