//! G7-lle unit tests (no BIOS image required).
//!
//! Cited: Project store `docs/graycart-gba/06-cart-bios-saves.md` §3.4

use super::lle::{validate_bios_bytes, BIOS_SIZE, GBA_BIOS_ENV};

#[test]
fn wrong_size_errors() {
    let err = validate_bios_bytes(&[0u8; 100]).unwrap_err();
    assert!(err.contains(&BIOS_SIZE.to_string()) || err.contains("16"));
}

#[test]
fn exact_size_ok() {
    let img = vec![0u8; BIOS_SIZE];
    assert_eq!(validate_bios_bytes(&img).unwrap().len(), BIOS_SIZE);
}

#[test]
fn env_name_stable() {
    assert_eq!(GBA_BIOS_ENV, "GBA_BIOS");
}

#[test]
fn sha256_empty_bios_deterministic() {
    let img = vec![0u8; BIOS_SIZE];
    let a = super::lle::sha256_hex(&img);
    let b = super::lle::sha256_hex(&img);
    assert_eq!(a, b);
    assert_eq!(a.len(), 64);
}
