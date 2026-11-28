use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn parse_klipper_pin(spec: &str) -> Option<(char, u8, &'static str)> {
    let mut rest = spec.trim();
    if rest.is_empty() {
        return None;
    }
    let mut inverted = false;
    let mut pull_up = false;
    let mut pull_down = false;
    while let Some(b) = rest.as_bytes().first().copied() {
        match b {
            b'!' => inverted = true,
            b'^' => pull_up = true,
            b'~' => pull_down = true,
            _ => break,
        }
        rest = &rest[1..];
    }
    if pull_up && pull_down {
        return None;
    }
    let mode = if pull_up {
        "InputPullUp"
    } else if pull_down {
        "InputPullDown"
    } else if inverted {
        "OutputLow"
    } else {
        "OutputHigh"
    };
    let bytes = rest.as_bytes();
    if bytes.len() < 2 {
        return None;
    }
    let (port, num_start) = if bytes[0] == b'P' || bytes[0] == b'p' {
        ((bytes[1] as char).to_ascii_uppercase(), 2)
    } else {
        ((bytes[0] as char).to_ascii_uppercase(), 1)
    };
    if !('A'..='Z').contains(&port) {
        return None;
    }
    let pin: u8 = rest[num_start..].parse().ok()?;
    if pin > 31 {
        return None;
    }
    Some((port, pin, mode))
}

fn parse_status_led_pin(spec: &str) -> Option<(char, u8, bool)> {
    let mut rest = spec.trim();
    if rest.is_empty() {
        return None;
    }
    let mut active_low = false;
    if let Some(stripped) = rest.strip_prefix('!') {
        active_low = true;
        rest = stripped.trim();
    }
    let bytes = rest.as_bytes();
    if bytes.len() < 2 {
        return None;
    }
    let (port, num_start) = if bytes[0] == b'P' || bytes[0] == b'p' {
        ((bytes[1] as char).to_ascii_uppercase(), 2)
    } else {
        ((bytes[0] as char).to_ascii_uppercase(), 1)
    };
    if !('A'..='H').contains(&port) {
        return None;
    }
    let pin: u8 = rest[num_start..].parse().ok()?;
    if pin > 15 {
        return None;
    }
    Some((port, pin, active_low))
}

fn count_bootloader_ports_from_toml(toml_text: &str) -> usize {
    let mut in_comm_port = false;
    let mut is_bootloader = false;
    let mut count = 0usize;
    for raw_line in toml_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            if in_comm_port && is_bootloader {
                count += 1;
            }
            in_comm_port = line.starts_with("[communication.") && !line.starts_with("[[");
            is_bootloader = false;
            continue;
        }
        if in_comm_port {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == "bootloader" && v.trim() == "true" {
                    is_bootloader = true;
                }
            }
        }
    }
    if in_comm_port && is_bootloader {
        count += 1;
    }
    count
}

fn extract_status_led_from_toml(toml_text: &str) -> Option<String> {
    let mut in_bootloader = false;
    for raw_line in toml_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_bootloader = line == "[bootloader]"
                || line.starts_with("[bootloader]")
                    && line["[bootloader]".len()..].trim_start().starts_with('#');
            continue;
        }
        if !in_bootloader {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            if key.trim() == "status_led" {
                let val = val.trim();
                let quote = val.chars().next()?;
                if quote == '"' || quote == '\'' {
                    let rest = &val[1..];
                    if let Some(end) = rest.find(quote) {
                        return Some(rest[..end].to_string());
                    }
                }
            }
        }
    }
    None
}

fn main() {
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    File::create(out.join("device.x"))
        .unwrap()
        .write_all(b"")
        .unwrap();

    let mut generated = String::from(
        "pub static INITIAL_SAFE_PINS: &[bareboot_core::SafePin] = &bareboot_core::initial_safe_pins![\n",
    );
    if let Ok(raw) = env::var("BAREBOOT_SAFE_PINS") {
        for item in raw.split(',') {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some((port, pin, mode)) = parse_klipper_pin(trimmed) {
                generated.push_str(&format!("    ('{port}', {pin}, {mode}),\n"));
            } else {
                panic!("Invalid Klipper pin in BAREBOOT_SAFE_PINS: {trimmed}");
            }
        }
    }
    generated.push_str("];\n");
    File::create(out.join("safe_pins_generated.rs"))
        .unwrap()
        .write_all(generated.as_bytes())
        .unwrap();

    let config_path = env::var("BAREBOOT_CONFIG")
        .ok()
        .or_else(|| env::var("BAREFOC_BOARD").ok());
    let (status_led_spec, bl_port_count): (Option<String>, usize) = if let Some(ref path) = config_path {
        println!("cargo:rerun-if-changed={path}");
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read config {path}: {e}"));
        (
            extract_status_led_from_toml(&text),
            count_bootloader_ports_from_toml(&text),
        )
    } else {
        (Some("!PA1".to_string()), 1)
    };

    let use_multi_arbiter = bl_port_count >= 2;
    let transport_rs = format!(
        "#[allow(dead_code)]\npub const BOOTLOADER_PORT_COUNT: usize = {bl_port_count};\n#[allow(dead_code)]\npub const USE_MULTI_TRANSPORT_ARBITER: bool = {use_multi_arbiter};\n"
    );
    File::create(out.join("transport_arbiter_generated.rs"))
        .unwrap()
        .write_all(transport_rs.as_bytes())
        .unwrap();

    let status_led_rs = match status_led_spec.as_deref().map(str::trim) {
        Some("") | None => {
            "pub const STATUS_LED_CONFIG: Option<(char, u8, bool)> = None;\n".to_string()
        }
        Some(spec) => {
            let (port, pin, active_low) = parse_status_led_pin(spec)
                .unwrap_or_else(|| panic!("Invalid [bootloader].status_led pin: {spec}"));
            format!(
                "pub const STATUS_LED_CONFIG: Option<(char, u8, bool)> = Some(('{port}', {pin}, {active_low}));\n"
            )
        }
    };
    File::create(out.join("status_led_generated.rs"))
        .unwrap()
        .write_all(status_led_rs.as_bytes())
        .unwrap();

    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-env-changed=BAREBOOT_SAFE_PINS");
    println!("cargo:rerun-if-env-changed=BAREBOOT_CONFIG");
    println!("cargo:rerun-if-env-changed=BAREFOC_BOARD");
}
