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

    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-env-changed=BAREBOOT_SAFE_PINS");
}
