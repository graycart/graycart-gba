//! ROM / conformance harnesses (jsmolka must-gate + ignored edge matrices).
//!
//! ```bash
//! cargo test --test roms
//! cargo test --test roms jsmolka_must_matrix -- --nocapture
//! cargo test --test roms -- --ignored --nocapture
//! ```

mod alyosha;
mod fuzzarm;
mod hades;
mod harness;
mod jsmolka;
mod mgba_suite;
mod nba;
mod png183;
