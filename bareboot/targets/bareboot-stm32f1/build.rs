use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(serde::Deserialize)]
struct Config {
    target: String,
    mcu: String,
    #[serde(default = "default_clock")]
    system_clock_hz: u32,
    #[serde(default = "default_flash")]
    flash_size: usize,
    #[serde(default = "default_ram")]
    ram_size: usize,
    #[serde(default = "default_app")]
    app_base: u32,
    #[serde(default = "default_window")]
    boot_window_ms: u32,
    #[serde(default)]
    transports: Vec<String>,
    #[serde(default = "default_can")]
    can_bitrate: u32,
    #[serde(default = "default_i2c")]
    i2c_target_address: u8,
}

fn default_clock() -> u32 {
    144_000_000
}
fn default_flash() -> usize {
    4096
}
fn default_ram() -> usize {
    10240
}
fn default_app() -> u32 {
    0x0800_1000
}
fn default_window() -> u32 {
    250
}
fn default_can() -> u32 {
    1_000_000
}
fn default_i2c() -> u8 {
    0x40
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let config_path = env::var("BAREBOOT_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let candidate = manifest_dir.join("../../../bareboot.toml");
            if candidate.exists() {
                candidate
            } else {
                manifest_dir.join("../../../../bareboot.toml")
            }
        });

    let raw = fs::read_to_string(&config_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", config_path.display()));
    let cfg: Config = toml::from_str(&raw).expect("invalid bareboot.toml");

    for transport in &cfg.transports {
        println!("cargo:rustc-cfg=transport_{transport}");
    }

    for transport in ["can", "i2c", "uart", "usb"] {
        println!("cargo:rustc-check-cfg=cfg(transport_{transport})");
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    println!("cargo:rustc-link-arg=-Tdevice.x");
    println!(
        "cargo:rustc-link-search=native={}",
        manifest_dir.display()
    );
    println!("cargo:rustc-link-arg=-Trtt_store.ld");
    let memory_x = format!(
        "MEMORY {{\n  FLASH (rx) : ORIGIN = 0x08000000, LENGTH = {}K\n  RAM  (rwx) : ORIGIN = 0x20000000, LENGTH = {}K\n}}\n",
        cfg.flash_size / 1024,
        cfg.ram_size / 1024
    );
    fs::write(out_dir.join("memory.x"), memory_x).expect("failed to write memory.x");

    let mut constants = String::new();
    constants.push_str(&format!("pub const TARGET: &str = {:?};\n", cfg.target));
    constants.push_str(&format!("pub const MCU: &str = {:?};\n", cfg.mcu));
    constants.push_str(&format!(
        "pub const SYSTEM_CLOCK_HZ: u32 = {};\n",
        cfg.system_clock_hz
    ));
    constants.push_str(&format!("pub const APP_BASE: u32 = {:#x};\n", cfg.app_base));
    constants.push_str(&format!(
        "pub const BOOT_WINDOW_MS: u32 = {};\n",
        cfg.boot_window_ms
    ));
    constants.push_str(&format!("pub const FLASH_SIZE: usize = {};\n", cfg.flash_size));
    constants.push_str(&format!("pub const RAM_SIZE: usize = {};\n", cfg.ram_size));
    constants.push_str(&format!("pub const CAN_BITRATE: u32 = {};\n", cfg.can_bitrate));
    constants.push_str(&format!(
        "pub const I2C_TARGET_ADDRESS: u8 = {:#x};\n",
        cfg.i2c_target_address
    ));
    fs::write(out_dir.join("config_generated.rs"), constants).expect("failed to write config");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rerun-if-changed={}", config_path.display());
    println!("cargo:rerun-if-env-changed=BAREBOOT_CONFIG");
}
