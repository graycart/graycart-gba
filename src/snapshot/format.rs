//! GAS1 on-disk save-state container (header + CRC + postcard payload).

use super::SNAPSHOT_FORMAT_VERSION;
use crate::hw::ArmMachineState;
use crc32fast::Hasher as Crc32;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read, Write};

pub const MAGIC: &[u8; 4] = b"GAS1";

/// Slot UI metadata extracted from a GAS1 file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gas1Metadata {
    pub title: String,
    pub timestamp: i64,
    pub flags: u32,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gas1Error {
    BadMagic,
    Truncated,
    UnsupportedVersion(u16),
    RomMismatch,
    PayloadCrcMismatch,
    PostcardEncode(String),
    PostcardDecode(String),
    HeaderWrite(String),
}

pub fn rom_sha256(rom: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(rom);
    h.finalize().into()
}

pub fn sanitize_title(title: &str) -> String {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return "Graycart".to_string();
    }
    trimmed
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn write_len_prefixed_str(buf: &mut Vec<u8>, s: &str) -> std::io::Result<()> {
    let b = s.as_bytes();
    let len = u16::try_from(b.len()).map_err(|_| std::io::ErrorKind::InvalidInput)?;
    buf.write_all(&len.to_le_bytes())?;
    buf.write_all(b)?;
    Ok(())
}

fn read_len_prefixed_str(cursor: &mut Cursor<&[u8]>) -> std::io::Result<String> {
    let mut len_b = [0u8; 2];
    cursor.read_exact(&mut len_b)?;
    let len = u16::from_le_bytes(len_b) as usize;
    let mut s = vec![0u8; len];
    cursor.read_exact(&mut s)?;
    Ok(String::from_utf8_lossy(&s).into_owned())
}

struct Gas1Header {
    version: String,
    rom_sha256: [u8; 32],
    title: String,
    timestamp: i64,
    flags: u32,
    payload_len: u32,
    payload_crc32: u32,
}

fn parse_header(bytes: &[u8]) -> Result<(Gas1Header, usize), Gas1Error> {
    if bytes.len() < 4 {
        return Err(Gas1Error::Truncated);
    }
    if &bytes[0..4] != MAGIC {
        return Err(Gas1Error::BadMagic);
    }
    let mut cur = Cursor::new(&bytes[4..]);
    let mut ver = [0u8; 2];
    if cur.read_exact(&mut ver).is_err() {
        return Err(Gas1Error::Truncated);
    }
    let format_version = u16::from_le_bytes(ver);
    if format_version != SNAPSHOT_FORMAT_VERSION {
        return Err(Gas1Error::UnsupportedVersion(format_version));
    }
    let version = read_len_prefixed_str(&mut cur).map_err(|_| Gas1Error::Truncated)?;
    let mut sha = [0u8; 32];
    if cur.read_exact(&mut sha).is_err() {
        return Err(Gas1Error::Truncated);
    }
    let title = read_len_prefixed_str(&mut cur).map_err(|_| Gas1Error::Truncated)?;
    let mut ts_b = [0u8; 8];
    cur.read_exact(&mut ts_b)
        .map_err(|_| Gas1Error::Truncated)?;
    let timestamp = i64::from_le_bytes(ts_b);
    let mut flags_b = [0u8; 4];
    cur.read_exact(&mut flags_b)
        .map_err(|_| Gas1Error::Truncated)?;
    let flags = u32::from_le_bytes(flags_b);
    let mut len_b = [0u8; 4];
    cur.read_exact(&mut len_b)
        .map_err(|_| Gas1Error::Truncated)?;
    let payload_len = u32::from_le_bytes(len_b);
    let mut crc_b = [0u8; 4];
    cur.read_exact(&mut crc_b)
        .map_err(|_| Gas1Error::Truncated)?;
    let payload_crc32 = u32::from_le_bytes(crc_b);
    let payload_offset = 4 + cur.position() as usize;
    Ok((
        Gas1Header {
            version,
            rom_sha256: sha,
            title,
            timestamp,
            flags,
            payload_len,
            payload_crc32,
        },
        payload_offset,
    ))
}

pub fn encode_gas1(rom: &[u8], title: &str, state: &ArmMachineState) -> Result<Vec<u8>, Gas1Error> {
    let payload =
        postcard::to_allocvec(state).map_err(|e| Gas1Error::PostcardEncode(e.to_string()))?;
    let payload_crc32 = {
        let mut h = Crc32::new();
        h.update(&payload);
        h.finalize()
    };
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&SNAPSHOT_FORMAT_VERSION.to_le_bytes());
    write_len_prefixed_str(&mut out, env!("CARGO_PKG_VERSION"))
        .map_err(|e| Gas1Error::HeaderWrite(e.to_string()))?;
    out.extend_from_slice(&rom_sha256(rom));
    write_len_prefixed_str(&mut out, &sanitize_title(title))
        .map_err(|e| Gas1Error::HeaderWrite(e.to_string()))?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    out.extend_from_slice(&ts.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload_crc32.to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

pub fn decode_gas1(rom: &[u8], bytes: &[u8]) -> Result<(ArmMachineState, Gas1Metadata), Gas1Error> {
    let (header, payload_offset) = parse_header(bytes)?;
    if header.rom_sha256 != rom_sha256(rom) {
        return Err(Gas1Error::RomMismatch);
    }
    let payload_len = header.payload_len as usize;
    let file_payload = &bytes[payload_offset..];
    if file_payload.len() < payload_len {
        return Err(Gas1Error::Truncated);
    }
    let payload = &file_payload[..payload_len];
    let actual_crc = {
        let mut h = Crc32::new();
        h.update(payload);
        h.finalize()
    };
    if actual_crc != header.payload_crc32 {
        return Err(Gas1Error::PayloadCrcMismatch);
    }
    let state: ArmMachineState =
        postcard::from_bytes(payload).map_err(|e| Gas1Error::PostcardDecode(e.to_string()))?;
    Ok((
        state,
        Gas1Metadata {
            title: header.title,
            timestamp: header.timestamp,
            flags: header.flags,
            version: header.version,
        },
    ))
}
