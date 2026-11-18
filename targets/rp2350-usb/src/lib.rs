//! Pure boot-decision helpers for the RP2350 BareBoot target.
//!
//! This library is host-testable (`cargo test`) and contains no hardware
//! access. The firmware binary in `src/main.rs` consumes these helpers.

#![no_std]

#[cfg(test)]
extern crate std;

pub mod boot;

pub use boot::*;
