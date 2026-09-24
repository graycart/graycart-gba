//! BIOS decompression SWIs (LZ77, Huffman, RLE, Diff).
//!
//! Cited: GBATEK BIOS Decompression Functions.
//! <https://problemkaputt.de/gbatek-bios-decompression-functions.htm>
//!
//! Huffman tree (GBATEK): after the 4-byte header, one tree-size byte `T` where the
//! tree table is `(T << 1) + 1` bytes (bitstream follows immediately). Nodes are
//! 8-bit. For an internal/root node, bits 0–5 are the child-pair offset; child0 is
//! at `(node_addr & !1) + offset * 2 + 2`, child1 is that plus one. Bit 7 set means
//! child0 is a data leaf; bit 6 set means child1 is a data leaf. The bitstream is
//! 32-bit words, MSB first: 0 selects child0, 1 selects child1. After emitting a
//! symbol the walk returns to the root. Symbols are `header & 0xF` bits wide
//! (normally 4 or 8) and are packed into 32-bit destination writes.

use crate::bus::Bus;

/// SWI 0x11 — LZ77UnCompWram. Type byte `0x10`. Destination may use 8-bit stores.
pub fn lz77_wram(bus: &mut Bus, src: u32, dst: u32) {
    lz77(bus, src, dst, false);
}

/// SWI 0x12 — LZ77UnCompVram. Same stream; destination is written as halfwords only.
pub fn lz77_vram(bus: &mut Bus, src: u32, dst: u32) {
    lz77(bus, src, dst, true);
}

/// SWI 0x13 — HuffUnComp. Header low nibble is symbol width (4 or 8); high nibble is 2.
pub fn huff(bus: &mut Bus, src: u32, dst: u32) {
    let src = src & !3;
    let header = bus.read32(src);
    let mut remaining = header >> 8;
    let bits = header & 0xF;
    if bits == 0 || 32 % bits != 0 {
        return;
    }
    let tree_size_byte = u32::from(bus.read8(src.wrapping_add(4)));
    let tree_bytes = (tree_size_byte << 1) + 1;
    // Corrupt trees that never mark a leaf must not spin: abort after this many
    // bit-steps without emitting one symbol.
    let max_steps = tree_bytes.max(256);
    let tree_base = src.wrapping_add(5);
    let mut stream = src.wrapping_add(5).wrapping_add(tree_bytes);
    let mut dest = dst;
    let mut node_addr = tree_base;
    let mut node = bus.read8(node_addr);
    let mut out_word = 0u32;
    let mut bits_seen = 0u32;
    let mut walk_steps = 0u32;

    while remaining > 0 {
        let mut bitstream = bus.read32(stream);
        stream = stream.wrapping_add(4);
        for _ in 0..32 {
            if remaining == 0 {
                break;
            }
            walk_steps += 1;
            if walk_steps > max_steps {
                return;
            }
            let next = (node_addr & !1).wrapping_add((u32::from(node & 0x3F) << 1).wrapping_add(2));
            let take_right = bitstream & 0x8000_0000 != 0;
            bitstream <<= 1;
            let symbol = if take_right {
                if node & 0x40 != 0 {
                    bus.read8(next.wrapping_add(1))
                } else {
                    node_addr = next.wrapping_add(1);
                    node = bus.read8(node_addr);
                    continue;
                }
            } else if node & 0x80 != 0 {
                bus.read8(next)
            } else {
                node_addr = next;
                node = bus.read8(node_addr);
                continue;
            };

            walk_steps = 0;
            let mask = (1u32 << bits) - 1;
            out_word |= (u32::from(symbol) & mask) << bits_seen;
            bits_seen += bits;
            node_addr = tree_base;
            node = bus.read8(node_addr);
            if bits_seen == 32 {
                bus.write32(dest, out_word);
                dest = dest.wrapping_add(4);
                remaining = remaining.saturating_sub(4);
                out_word = 0;
                bits_seen = 0;
            }
        }
    }
}

/// SWI 0x14 — RLUnCompWram. Type byte `0x30`.
pub fn rl_wram(bus: &mut Bus, src: u32, dst: u32) {
    rl(bus, src, dst, false);
}

/// SWI 0x15 — RLUnCompVram. Halfword destination stores only.
pub fn rl_vram(bus: &mut Bus, src: u32, dst: u32) {
    rl(bus, src, dst, true);
}

/// SWI 0x16 — Diff8bitUnFilterWrite8bit. Type byte `0x81`.
pub fn diff8_wram(bus: &mut Bus, src: u32, dst: u32) {
    let header = bus.read32(src & !3);
    let mut remaining = header >> 8;
    let mut source = (src & !3).wrapping_add(4);
    let mut dest = dst;
    if remaining == 0 {
        return;
    }
    let mut value = bus.read8(source);
    source = source.wrapping_add(1);
    bus.write8(dest, value);
    dest = dest.wrapping_add(1);
    remaining -= 1;
    while remaining > 0 {
        value = value.wrapping_add(bus.read8(source));
        source = source.wrapping_add(1);
        bus.write8(dest, value);
        dest = dest.wrapping_add(1);
        remaining -= 1;
    }
}

/// SWI 0x17 — Diff8bitUnFilterWrite16bit. Same filter; halfword destination stores.
pub fn diff8_vram(bus: &mut Bus, src: u32, dst: u32) {
    let header = bus.read32(src & !3);
    let mut remaining = header >> 8;
    let mut source = (src & !3).wrapping_add(4);
    let mut dest = dst;
    let mut half = 0u16;
    if remaining == 0 {
        return;
    }
    let mut value = bus.read8(source);
    source = source.wrapping_add(1);
    emit_vram(bus, dest, &mut half, value);
    dest = dest.wrapping_add(1);
    remaining -= 1;
    while remaining > 0 {
        value = value.wrapping_add(bus.read8(source));
        source = source.wrapping_add(1);
        emit_vram(bus, dest, &mut half, value);
        dest = dest.wrapping_add(1);
        remaining -= 1;
    }
}

/// SWI 0x18 — Diff16bitUnFilter. Type byte `0x82`. Signed 16-bit deltas.
pub fn diff16(bus: &mut Bus, src: u32, dst: u32) {
    let header = bus.read32(src & !3);
    let mut remaining = header >> 8;
    let mut source = (src & !3).wrapping_add(4);
    let mut dest = dst;
    if remaining < 2 {
        return;
    }
    let mut value = bus.read16(source);
    source = source.wrapping_add(2);
    bus.write16(dest, value);
    dest = dest.wrapping_add(2);
    remaining -= 2;
    while remaining >= 2 {
        let delta = bus.read16(source) as i16;
        source = source.wrapping_add(2);
        value = value.wrapping_add(delta as u16);
        bus.write16(dest, value);
        dest = dest.wrapping_add(2);
        remaining -= 2;
    }
}

fn lz77(bus: &mut Bus, src: u32, dst: u32, vram: bool) {
    let header = bus.read32(src & !3);
    let mut remaining = header >> 8;
    let mut source = (src & !3).wrapping_add(4);
    let mut dest = dst;
    let mut half = 0u16;

    while remaining > 0 {
        let mut flags = bus.read8(source);
        source = source.wrapping_add(1);
        for _ in 0..8 {
            if remaining == 0 {
                break;
            }
            if flags & 0x80 != 0 {
                let b0 = bus.read8(source);
                let b1 = bus.read8(source.wrapping_add(1));
                source = source.wrapping_add(2);
                let block = (u16::from(b0) << 8) | u16::from(b1);
                let mut length = usize::from(block >> 12) + 3;
                let mut disp = dest.wrapping_sub(u32::from(block & 0x0FFF)).wrapping_sub(1);
                while length > 0 && remaining > 0 {
                    let byte = if vram {
                        let word = bus.read16(disp & !1);
                        if disp & 1 != 0 {
                            (word >> 8) as u8
                        } else {
                            word as u8
                        }
                    } else {
                        bus.read8(disp)
                    };
                    if vram {
                        emit_vram(bus, dest, &mut half, byte);
                    } else {
                        bus.write8(dest, byte);
                    }
                    disp = disp.wrapping_add(1);
                    dest = dest.wrapping_add(1);
                    length -= 1;
                    remaining -= 1;
                }
            } else {
                let byte = bus.read8(source);
                source = source.wrapping_add(1);
                if vram {
                    emit_vram(bus, dest, &mut half, byte);
                } else {
                    bus.write8(dest, byte);
                }
                dest = dest.wrapping_add(1);
                remaining -= 1;
            }
            flags <<= 1;
        }
    }
}

fn rl(bus: &mut Bus, src: u32, dst: u32, vram: bool) {
    let header = bus.read32(src & !3);
    let mut remaining = header >> 8;
    let mut source = (src & !3).wrapping_add(4);
    let mut dest = dst;
    let mut half = 0u16;

    while remaining > 0 {
        let flag = bus.read8(source);
        source = source.wrapping_add(1);
        if flag & 0x80 != 0 {
            let mut count = u32::from(flag & 0x7F) + 3;
            let byte = bus.read8(source);
            source = source.wrapping_add(1);
            while count > 0 && remaining > 0 {
                if vram {
                    emit_vram(bus, dest, &mut half, byte);
                } else {
                    bus.write8(dest, byte);
                }
                dest = dest.wrapping_add(1);
                count -= 1;
                remaining -= 1;
            }
        } else {
            let mut count = u32::from(flag) + 1;
            while count > 0 && remaining > 0 {
                let byte = bus.read8(source);
                source = source.wrapping_add(1);
                if vram {
                    emit_vram(bus, dest, &mut half, byte);
                } else {
                    bus.write8(dest, byte);
                }
                dest = dest.wrapping_add(1);
                count -= 1;
                remaining -= 1;
            }
        }
    }
}

/// VRAM destination: `dest` is a byte cursor. Even address buffers the low byte;
/// odd address completes a little-endian halfword at `dest ^ 1`. No `write8`.
fn emit_vram(bus: &mut Bus, dest: u32, half: &mut u16, byte: u8) {
    if dest & 1 != 0 {
        *half |= u16::from(byte) << 8;
        bus.write16(dest ^ 1, *half);
    } else {
        *half = u16::from(byte);
    }
}

#[cfg(test)]
mod tests {
    use super::{diff8_wram, huff, lz77_vram, lz77_wram, rl_wram};
    use crate::bus::Bus;

    fn put(bus: &mut Bus, base: u32, bytes: &[u8]) {
        for (i, byte) in bytes.iter().enumerate() {
            bus.write8(base + i as u32, *byte);
        }
    }

    #[test]
    fn lz77_literals_land_in_iwram() {
        let mut bus = Bus::new(vec![0; 0x200]);
        let header = [0x10u8, 4, 0, 0, 0x00, 1, 2, 3, 4];
        for (i, byte) in header.iter().enumerate() {
            bus.write8(0x0300_0000 + i as u32, *byte);
        }
        lz77_wram(&mut bus, 0x0300_0000, 0x0300_0100);
        assert_eq!(bus.read8(0x0300_0100), 1);
        assert_eq!(bus.read8(0x0300_0103), 4);
    }

    #[test]
    fn lz77_backref_copies_prior_bytes() {
        // length 5: literals A B, then match of 3 from disp 1 → A B A B A
        // flag 0b0010_0000: lit, lit, backref (MSB first)
        let mut bus = Bus::new(vec![0; 0x200]);
        put(
            &mut bus,
            0x0300_0000,
            &[0x10, 5, 0, 0, 0b0010_0000, b'A', b'B', 0x00, 0x01],
        );
        lz77_wram(&mut bus, 0x0300_0000, 0x0300_0100);
        assert_eq!(bus.read8(0x0300_0100), b'A');
        assert_eq!(bus.read8(0x0300_0101), b'B');
        assert_eq!(bus.read8(0x0300_0102), b'A');
        assert_eq!(bus.read8(0x0300_0103), b'B');
        assert_eq!(bus.read8(0x0300_0104), b'A');
    }

    #[test]
    fn lz77_vram_pairs_two_literals_as_one_halfword() {
        // Two literals → one write16 LE. Odd trailing byte stays buffered (no write8).
        let mut bus = Bus::new(vec![0; 0x200]);
        put(
            &mut bus,
            0x0300_0000,
            &[0x10, 3, 0, 0, 0x00, 0x34, 0x12, 0x99],
        );
        bus.write16(0x0600_0000, 0);
        bus.write16(0x0600_0002, 0);
        lz77_vram(&mut bus, 0x0300_0000, 0x0600_0000);
        assert_eq!(bus.read16(0x0600_0000), 0x1234);
        // Trailing 0x99 must not land as an 8-bit store at +2.
        assert_eq!(bus.read16(0x0600_0002), 0);
    }

    #[test]
    fn rl_literals_copy_flag_plus_one() {
        let mut bus = Bus::new(vec![0; 0x200]);
        put(&mut bus, 0x0300_0000, &[0x30, 3, 0, 0, 2, 10, 20, 30]);
        rl_wram(&mut bus, 0x0300_0000, 0x0300_0100);
        assert_eq!(bus.read8(0x0300_0100), 10);
        assert_eq!(bus.read8(0x0300_0101), 20);
        assert_eq!(bus.read8(0x0300_0102), 30);
    }

    #[test]
    fn rl_repeat_writes_flag_masked_plus_three() {
        let mut bus = Bus::new(vec![0; 0x200]);
        put(&mut bus, 0x0300_0000, &[0x30, 4, 0, 0, 0x81, 0xAB]);
        rl_wram(&mut bus, 0x0300_0000, 0x0300_0100);
        for i in 0..4u32 {
            assert_eq!(bus.read8(0x0300_0100 + i), 0xAB);
        }
    }

    #[test]
    fn diff8_absolute_then_delta_yields_one_two() {
        let mut bus = Bus::new(vec![0; 0x200]);
        put(&mut bus, 0x0300_0000, &[0x81, 2, 0, 0, 1, 1]);
        diff8_wram(&mut bus, 0x0300_0000, 0x0300_0100);
        assert_eq!(bus.read8(0x0300_0100), 1);
        assert_eq!(bus.read8(0x0300_0101), 2);
    }

    #[test]
    fn huff_one_symbol_root_bit0_is_data_leaf() {
        // Header 0x28: 8-bit symbols, type 2. Length 4 (Huffman writes 32-bit units).
        // T=1 → tree is (1<<1)+1 = 3 bytes at +5..+7; bitstream at +8.
        // Root at +5 = 0xC0 (both children data, offset 0).
        // child0 at (5&!1)+2 = +6 → 0x42; child1 at +7.
        // Four MSB-zero bits select node0 four times → write32 0x42424242.
        let mut bus = Bus::new(vec![0; 0x200]);
        put(
            &mut bus,
            0x0300_0000,
            &[
                0x28, 0x04, 0x00, 0x00, // header: 8-bit, type 2, size 4
                0x01, // T
                0xC0, // root
                0x42, // node0 data
                0x00, // node1 data
                0x00, 0x00, 0x00, 0x00, // bitstream
            ],
        );
        huff(&mut bus, 0x0300_0000, 0x0300_0100);
        assert_eq!(bus.read8(0x0300_0100), 0x42);
        assert_eq!(bus.read32(0x0300_0100), 0x4242_4242);
    }

    #[test]
    fn huff_leafless_root_offset0_returns() {
        // Root 0x00: offset 0, bits 6/7 clear → every bit walks to another internal
        // node and never emits. Without the step cap this spins forever.
        let mut bus = Bus::new(vec![0; 0x200]);
        put(
            &mut bus,
            0x0300_0000,
            &[
                0x28, 0x04, 0x00, 0x00, // header: size 4
                0x01, // T → 3-byte tree
                0x00, // root: internal, offset 0, no leaf flags
                0x00, 0x00, // children (also leafless)
                0x00, 0x00, 0x00, 0x00, // short bitstream
            ],
        );
        huff(&mut bus, 0x0300_0000, 0x0300_0100);
    }
}
