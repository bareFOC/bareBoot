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

fn parse_num_u32(raw: &str) -> Option<u32> {
    let s = raw.split('#').next()?.trim().replace('_', "");
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u32>().ok()
    }
}

fn strip_toml_str(raw: &str) -> Option<String> {
    let val = raw.split('#').next()?.trim();
    let quote = val.chars().next()?;
    if quote == '"' || quote == '\'' {
        let rest = &val[1..];
        if let Some(end) = rest.find(quote) {
            return Some(rest[..end].to_string());
        }
    }
    None
}

fn pin_spec_to_safe_inactive(spec: &str) -> Option<(char, u8, &'static str)> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('^') || trimmed.starts_with('~') {
        return parse_klipper_pin(trimmed);
    }
    // In barefoc.toml [gate_driver], `!PIN` means active-low (safe inactive = OutputHigh),
    // while `PIN` means active-high (safe inactive = OutputLow).
    let (port, pin, _) = parse_klipper_pin(trimmed)?;
    let mode = if trimmed.starts_with('!') {
        "OutputHigh"
    } else {
        "OutputLow"
    };
    Some((port, pin, mode))
}

fn extract_gate_driver_safe_pins_from_toml(toml_text: &str) -> Vec<(char, u8, &'static str)> {
    const PIN_KEYS: &[&str] = &[
        "high_pwm_pin",
        "low_pwm_pin",
        "pwm_pin",
        "en_pin",
        "dir_pin",
        "sleep_pin",
        "cs_pin",
        "fault_pin",
        "inh_a",
        "inh_b",
        "inh_c",
        "inl_a",
        "inl_b",
        "inl_c",
    ];
    let mut pins: Vec<(char, u8, &'static str)> = Vec::new();
    let mut in_gate_driver = false;
    for raw_line in toml_text.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_gate_driver = line == "[gate_driver]"
                || line.starts_with("[gate_driver.")
                || line.starts_with("[[gate_driver.");
            continue;
        }
        if !in_gate_driver {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim();
        if !PIN_KEYS.contains(&key) {
            continue;
        }
        let val = v.trim();
        if val.starts_with('[') {
            let inner = val.trim_start_matches('[').trim_end_matches(']');
            for item in inner.split(',') {
                if let Some(s) = strip_toml_str(item) {
                    if let Some(entry) = pin_spec_to_safe_inactive(&s) {
                        if !pins.iter().any(|&(p, n, _)| p == entry.0 && n == entry.1) {
                            pins.push(entry);
                        }
                    }
                }
            }
        } else if let Some(s) = strip_toml_str(val) {
            if let Some(entry) = pin_spec_to_safe_inactive(&s) {
                if !pins.iter().any(|&(p, n, _)| p == entry.0 && n == entry.1) {
                    pins.push(entry);
                }
            }
        }
    }
    pins
}

fn parse_gpio_port_pin(spec: &str) -> Option<(char, u8)> {
    let (port, pin, _) = parse_status_led_pin(spec)?;
    Some((port, pin))
}

struct ParsedBoardToml {
    flash_origin: u32,
    flash_size: u32,
    ram_origin: u32,
    ram_size: u32,
    bootloader_origin: u32,
    bootloader_size: u32,
    app_origin: u32,
    app_size: u32,
    page_size: u32,
    mailbox_addr: u32,
    boot_window_ms: u32,
    host_peripheral: String,
    host_scl: (char, u8),
    host_sda: (char, u8),
    host_static_addr: u8,
    status_led: Option<String>,
    bl_port_count: usize,
}

impl Default for ParsedBoardToml {
    fn default() -> Self {
        Self {
            flash_origin: 0x0800_0000,
            flash_size: 131_072,
            ram_origin: 0x2000_0000,
            ram_size: 32_768,
            bootloader_origin: 0x0800_0000,
            bootloader_size: 16_384,
            app_origin: 0x0800_4000,
            app_size: 90_112,
            page_size: 8_192,
            mailbox_addr: 0x2000_7FE0,
            boot_window_ms: 1500,
            host_peripheral: "I3C2".to_string(),
            host_scl: ('B', 3),
            host_sda: ('B', 4),
            host_static_addr: 0x40,
            status_led: Some("!PA1".to_string()),
            bl_port_count: 1,
        }
    }
}

fn parse_board_toml(toml_text: &str) -> ParsedBoardToml {
    let mut cfg = ParsedBoardToml::default();
    cfg.status_led = extract_status_led_from_toml(toml_text);
    cfg.bl_port_count = count_bootloader_ports_from_toml(toml_text);

    let mut section = String::new();
    let mut app_origin_set = false;
    let mut app_size_set = false;
    let mut nvm_size_val: Option<u32> = None;

    for raw_line in toml_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && !line.starts_with("[[") {
            if let Some(end) = line.find(']') {
                section = line[1..end].trim().to_string();
            }
            continue;
        }
        let Some((k_raw, v_raw)) = line.split_once('=') else {
            continue;
        };
        let key = k_raw.trim();
        match section.as_str() {
            "memory" => match key {
                "flash_origin" => if let Some(v) = parse_num_u32(v_raw) { cfg.flash_origin = v; },
                "flash_size" => if let Some(v) = parse_num_u32(v_raw) { cfg.flash_size = v; },
                "ram_origin" => if let Some(v) = parse_num_u32(v_raw) { cfg.ram_origin = v; },
                "ram_size" => if let Some(v) = parse_num_u32(v_raw) { cfg.ram_size = v; },
                "bootloader_origin" => if let Some(v) = parse_num_u32(v_raw) { cfg.bootloader_origin = v; },
                "bootloader_size" => if let Some(v) = parse_num_u32(v_raw) { cfg.bootloader_size = v; },
                "app_origin" => if let Some(v) = parse_num_u32(v_raw) { cfg.app_origin = v; app_origin_set = true; },
                "app_size" => if let Some(v) = parse_num_u32(v_raw) { cfg.app_size = v; app_size_set = true; },
                "nvm_size" => { nvm_size_val = parse_num_u32(v_raw); },
                "page_size" => if let Some(v) = parse_num_u32(v_raw) { cfg.page_size = v; },
                _ => {}
            },
            "bootloader" => match key {
                "mailbox_address" => if let Some(v) = parse_num_u32(v_raw) { cfg.mailbox_addr = v; },
                "boot_window_ms" => if let Some(v) = parse_num_u32(v_raw) { cfg.boot_window_ms = v; },
                _ => {}
            },
            "communication.host" => match key {
                "peripheral" => if let Some(s) = strip_toml_str(v_raw) { cfg.host_peripheral = s.to_ascii_uppercase(); },
                "scl_pin" | "tx_pin" | "tx_scl_pin" => if let Some(s) = strip_toml_str(v_raw).and_then(|p| parse_gpio_port_pin(&p)) { cfg.host_scl = s; },
                "sda_pin" | "rx_pin" | "rx_sda_pin" => if let Some(s) = strip_toml_str(v_raw).and_then(|p| parse_gpio_port_pin(&p)) { cfg.host_sda = s; },
                "address" => if let Some(v) = parse_num_u32(v_raw) { cfg.host_static_addr = v as u8; },
                _ => {}
            },
            s if s.starts_with("communication.") => {
                let port_name = s.trim_start_matches("communication.").to_ascii_lowercase();
                if port_name.starts_with("i3c") || port_name.starts_with("i2c") {
                    if let Some(last_non_digit) = port_name.rfind(|c: char| !c.is_ascii_digit()) {
                        let (prefix, suffix) = port_name.split_at(last_non_digit + 1);
                        if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                            cfg.host_peripheral = format!("{}{}", prefix.to_ascii_uppercase(), suffix);
                        }
                    }
                    match key {
                        "peripheral" => if let Some(s) = strip_toml_str(v_raw) { cfg.host_peripheral = s.to_ascii_uppercase(); },
                        "scl_pin" | "tx_pin" | "tx_scl_pin" => if let Some(s) = strip_toml_str(v_raw).and_then(|p| parse_gpio_port_pin(&p)) { cfg.host_scl = s; },
                        "sda_pin" | "rx_pin" | "rx_sda_pin" => if let Some(s) = strip_toml_str(v_raw).and_then(|p| parse_gpio_port_pin(&p)) { cfg.host_sda = s; },
                        "address" => if let Some(v) = parse_num_u32(v_raw) { cfg.host_static_addr = v as u8; },
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    if !app_origin_set {
        cfg.app_origin = cfg.bootloader_origin + cfg.bootloader_size;
    }
    if !app_size_set {
        let nvm_sz = nvm_size_val.unwrap_or(16_384);
        let commit_sz = cfg.page_size;
        cfg.app_size = cfg
            .flash_size
            .saturating_sub(cfg.bootloader_size)
            .saturating_sub(commit_sz)
            .saturating_sub(nvm_sz);
    }

    cfg
}

fn main() {
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("device.x"))
        .unwrap()
        .write_all(b"")
        .unwrap();

    let config_path = env::var("BAREBOOT_CONFIG")
        .ok()
        .or_else(|| env::var("BAREFOC_BOARD").ok());
    let (parsed, toml_safe_pins) = if let Some(ref path) = config_path {
        println!("cargo:rerun-if-changed={path}");
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read config {path}: {e}"));
        (
            parse_board_toml(&text),
            extract_gate_driver_safe_pins_from_toml(&text),
        )
    } else {
        (ParsedBoardToml::default(), Vec::new())
    };

    let mut safe_pins = toml_safe_pins;
    if let Ok(raw) = env::var("BAREBOOT_SAFE_PINS") {
        for item in raw.split(',') {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(entry) = parse_klipper_pin(trimmed) {
                if let Some(existing) = safe_pins.iter_mut().find(|(p, n, _)| *p == entry.0 && *n == entry.1) {
                    *existing = entry;
                } else {
                    safe_pins.push(entry);
                }
            } else {
                panic!("Invalid Klipper pin in BAREBOOT_SAFE_PINS: {trimmed}");
            }
        }
    }
    let mut generated = String::from(
        "pub static INITIAL_SAFE_PINS: &[bareboot_core::SafePin] = &bareboot_core::initial_safe_pins![\n",
    );
    for (port, pin, mode) in safe_pins {
        generated.push_str(&format!("    ('{port}', {pin}, {mode}),\n"));
    }
    generated.push_str("];\n");
    File::create(out.join("safe_pins_generated.rs"))
        .unwrap()
        .write_all(generated.as_bytes())
        .unwrap();

    let stack_start = parsed.ram_origin + parsed.ram_size - 48;
    let ram_len = parsed.ram_size - 48;
    let memory_x = format!(
        "MEMORY\n{{\n  FLASH : ORIGIN = 0x{:08X}, LENGTH = {}K\n  RAM   : ORIGIN = 0x{:08X}, LENGTH = {}\n}}\n_stack_start = 0x{:08X};\n",
        parsed.bootloader_origin,
        parsed.bootloader_size / 1024,
        parsed.ram_origin,
        ram_len,
        stack_start,
    );
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(memory_x.as_bytes())
        .unwrap();

    let app_base = parsed.app_origin;
    let app_end = parsed.app_origin + parsed.app_size;
    let commit_base = app_end;
    let commit_size = parsed.page_size;
    let page_size = parsed.page_size;

    let (i3c_base, rcc_rstr_off, rcc_enr_off, rcc_bit, host_af): (u32, u32, u32, u32, u8) =
        match parsed.host_peripheral.as_str() {
            "I3C1" => (0x4000_5C00, 0x074, 0x09C, 23, 4), // APB1L
            _ => (0x4400_3000, 0x080, 0x0A8, 9, 3),       // I3C2 APB3
        };

    let board_cfg_rs = format!(
        "pub const APP_BASE: u32 = 0x{app_base:08X};\n\
         pub const APP_END: u32 = 0x{app_end:08X};\n\
         pub const COMMIT_BASE: u32 = 0x{commit_base:08X};\n\
         pub const COMMIT_SIZE: usize = {commit_size};\n\
         pub const PAGE_SIZE: usize = {page_size};\n\
         pub const RAM_START: u32 = 0x{:08X};\n\
         pub const RAM_END: u32 = 0x{:08X};\n\
         pub const MAILBOX_ADDR: u32 = 0x{:08X};\n\
         pub const BOOT_WINDOW_MS: u32 = {};\n\
         pub const HOST_STATIC_ADDR: u8 = 0x{:02X};\n\
         pub const I3C_BASE: u32 = 0x{i3c_base:08X};\n\
         pub const RCC_I3C_RSTR_OFFSET: u32 = 0x{rcc_rstr_off:03X};\n\
         pub const RCC_I3C_ENR_OFFSET: u32 = 0x{rcc_enr_off:03X};\n\
         pub const RCC_I3C_BIT: u32 = {rcc_bit};\n\
         pub const HOST_SCL_PORT: char = '{}';\n\
         pub const HOST_SCL_PIN: u8 = {};\n\
         pub const HOST_SDA_PORT: char = '{}';\n\
         pub const HOST_SDA_PIN: u8 = {};\n\
         pub const HOST_AF: u8 = {host_af};\n",
        parsed.ram_origin,
        parsed.ram_origin + parsed.ram_size,
        parsed.mailbox_addr,
        parsed.boot_window_ms,
        parsed.host_static_addr,
        parsed.host_scl.0,
        parsed.host_scl.1,
        parsed.host_sda.0,
        parsed.host_sda.1,
    );
    File::create(out.join("board_config_generated.rs"))
        .unwrap()
        .write_all(board_cfg_rs.as_bytes())
        .unwrap();

    let bl_port_count = parsed.bl_port_count;
    let use_multi_arbiter = bl_port_count >= 2;
    let transport_rs = format!(
        "#[allow(dead_code)]\npub const BOOTLOADER_PORT_COUNT: usize = {bl_port_count};\n#[allow(dead_code)]\npub const USE_MULTI_TRANSPORT_ARBITER: bool = {use_multi_arbiter};\n"
    );
    File::create(out.join("transport_arbiter_generated.rs"))
        .unwrap()
        .write_all(transport_rs.as_bytes())
        .unwrap();

    let status_led_rs = match parsed.status_led.as_deref().map(str::trim) {
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
