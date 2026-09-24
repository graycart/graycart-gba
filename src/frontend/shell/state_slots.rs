//! Slot files beside the ROM: SM83 `{rom}.gcsN`, ARM `{rom}.gasN`.

use crate::{ArmMachineState, decode_gas1, encode_gas1};
use graycart::{Gcs1Metadata, MachineStateV1, decode_gcs1, encode_gcs1};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const SLOT_COUNT: usize = 10;

#[derive(Debug, Clone)]
#[allow(dead_code)] // menu slot labels in a follow-up
pub struct SlotMeta {
    pub slot: u8,
    pub title: String,
    pub timestamp: i64,
    pub exists: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingSlotOp {
    Save(u8),
    Load(u8),
}

pub fn slot_path(rom_path: &Path, slot: u8) -> PathBuf {
    assert!(slot < SLOT_COUNT as u8);
    let stem = rom_path.with_extension("");
    stem.with_extension(format!("gcs{slot}"))
}

pub fn arm_slot_path(rom_path: &Path, slot: u8) -> PathBuf {
    assert!(slot < SLOT_COUNT as u8);
    let stem = rom_path.with_extension("");
    stem.with_extension(format!("gas{slot}"))
}

pub fn save_slot(
    rom: &[u8],
    rom_path: &Path,
    title: &str,
    slot: u8,
    state: &MachineStateV1,
) -> io::Result<()> {
    let path = slot_path(rom_path, slot);
    let bytes = encode_gcs1(rom, title, state);
    fs::write(path, bytes)
}

pub fn load_slot(
    rom: &[u8],
    rom_path: &Path,
    slot: u8,
) -> io::Result<(MachineStateV1, Gcs1Metadata)> {
    let path = slot_path(rom_path, slot);
    let bytes = fs::read(path)?;
    decode_gcs1(rom, &bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}")))
}

pub fn save_arm_slot(
    rom: &[u8],
    rom_path: &Path,
    title: &str,
    slot: u8,
    state: &ArmMachineState,
) -> io::Result<()> {
    let path = arm_slot_path(rom_path, slot);
    let bytes = encode_gas1(rom, title, state)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}")))?;
    fs::write(path, bytes)
}

pub fn load_arm_slot(
    rom: &[u8],
    rom_path: &Path,
    slot: u8,
) -> io::Result<(ArmMachineState, crate::Gas1Metadata)> {
    let path = arm_slot_path(rom_path, slot);
    let bytes = fs::read(&path)?;
    decode_gas1(rom, &bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}")))
}

#[allow(dead_code)] // menu slot labels in a follow-up
pub fn slot_meta(rom_path: &Path, slot: u8) -> Option<SlotMeta> {
    let path = slot_path(rom_path, slot);
    if !path.exists() {
        return Some(SlotMeta {
            slot,
            title: String::new(),
            timestamp: 0,
            exists: false,
        });
    }
    let rom = fs::read(rom_path).ok()?;
    let bytes = fs::read(&path).ok()?;
    match decode_gcs1(&rom, &bytes) {
        Ok((_, meta)) => Some(SlotMeta {
            slot,
            title: meta.title,
            timestamp: meta.timestamp,
            exists: true,
        }),
        Err(_) => Some(SlotMeta {
            slot,
            title: "(corrupt)".into(),
            timestamp: 0,
            exists: false,
        }),
    }
}

/// Menu helper: decode metadata for all slots (skip corrupt with `exists: false`).
#[allow(dead_code)] // menu slot labels in a follow-up
pub fn list_slot_meta(rom: &[u8], rom_path: &Path) -> Vec<SlotMeta> {
    (0..SLOT_COUNT as u8)
        .map(|slot| {
            let path = slot_path(rom_path, slot);
            if !path.exists() {
                return SlotMeta {
                    slot,
                    title: String::new(),
                    timestamp: 0,
                    exists: false,
                };
            }
            match fs::read(&path).and_then(|b| {
                decode_gcs1(rom, &b)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}")))
            }) {
                Ok((_, meta)) => SlotMeta {
                    slot,
                    title: meta.title,
                    timestamp: meta.timestamp,
                    exists: true,
                },
                Err(_) => SlotMeta {
                    slot,
                    title: "(corrupt)".into(),
                    timestamp: 0,
                    exists: false,
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
