#![no_std]

#[allow(unused)]
include!(concat!(env!("OUT_DIR"), "/config_generated.rs"));

#[cfg(transport_can)]
pub const HAS_CAN: bool = true;
#[cfg(not(transport_can))]
pub const HAS_CAN: bool = false;
