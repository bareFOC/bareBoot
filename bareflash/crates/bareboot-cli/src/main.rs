use bareboot_client::{run_flash, HostBus, InMemoryBus};
use std::env;
use std::process::ExitCode;
use std::time::Duration;

#[cfg(target_os = "linux")]
use bareboot_client::{SerialBus, I2cBus, SocketCanBus};

enum Backend {
    #[cfg(target_os = "linux")]
    Serial { device: String, baud: u32 },
    #[cfg(target_os = "linux")]
    I2c { device: String, address: u8 },
    #[cfg(target_os = "linux")]
    Can { interface: String, node_id: u32 },
    Memory(InMemoryBus),
}

struct Args {
    firmware: String,
    tries: usize,
    verify: bool,
    timeout: Duration,
    backend: Backend,
}

fn usage() -> String {
    concat!(
        "bareboot-cli --firmware <path> [--timeout <seconds>] [--tries <n>] ",
        "[--memory] | ",
        "[--serial <device> [--baud <baud>]] | ",
        "[--i2c <device> --address <hex>] | ",
        "[--can <interface> --node-id <hex>]\n"
    )
    .to_string()
}

fn parse_args() -> Result<Args, String> {
    let mut firmware = None;
    let mut tries = 5usize;
    let mut timeout = Duration::from_secs(2);
    let mut serial = None;
    let mut baud = 115200u32;
    let mut i2c = None;
    let mut address_hex = None;
    let mut can = None;
    let mut node_id_hex = None;
    let mut memory = false;
    let mut verify = true;

    let mut it = env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--firmware" => firmware = Some(it.next().ok_or("missing firmware path")?),
            "--tries" => tries = it.next().ok_or("missing tries")?.parse().map_err(|_| "invalid tries")?,
            "--timeout" => {
                let s = it.next().ok_or("missing timeout")?;
                timeout = Duration::from_secs(s.parse().map_err(|_| "invalid timeout")?);
            }
            "--memory" => memory = true,
            "--no-verify" => verify = false,
            "--serial" => serial = Some(it.next().ok_or("missing serial device")?),
            "--baud" => baud = it.next().ok_or("missing baud")?.parse().map_err(|_| "invalid baud")?,
            "--i2c" => i2c = Some(it.next().ok_or("missing i2c device")?),
            "--address" => address_hex = Some(it.next().ok_or("missing i2c address")?),
            "--can" => can = Some(it.next().ok_or("missing can interface")?),
            "--node-id" => node_id_hex = Some(it.next().ok_or("missing node id")?),
            "--help" | "-h" => return Err(usage()),
            other => return Err(format!("unknown argument: {other}\n{}", usage())),
        }
    }

    let firmware = firmware.ok_or_else(|| format!("missing --firmware\n{}", usage()))?;

    let backend = if memory {
        Backend::Memory(InMemoryBus::new())
    } else if let Some(device) = serial {
        #[cfg(target_os = "linux")]
        {
            Backend::Serial { device, baud }
        }
        #[cfg(not(target_os = "linux"))]
        {
            return Err("serial backend is only available on Linux".to_string());
        }
    } else if let Some(device) = i2c {
        #[cfg(target_os = "linux")]
        {
            let address = u8::from_str_radix(address_hex.as_deref().unwrap_or("40").trim_start_matches("0x"), 16)
                .map_err(|_| "invalid i2c address".to_string())?;
            Backend::I2c { device, address }
        }
        #[cfg(not(target_os = "linux"))]
        {
            return Err("i2c backend is only available on Linux".to_string());
        }
    } else if let Some(interface) = can {
        #[cfg(target_os = "linux")]
        {
            let node_id = u32::from_str_radix(node_id_hex.as_deref().unwrap_or("100").trim_start_matches("0x"), 16)
                .map_err(|_| "invalid node id".to_string())?;
            Backend::Can { interface, node_id }
        }
        #[cfg(not(target_os = "linux"))]
        {
            return Err("can backend is only available on Linux".to_string());
        }
    } else {
        return Err(format!("no backend selected\n{}", usage()));
    };

    Ok(Args {
        firmware,
        tries,
        timeout,
        backend,
        verify,
    })
}

#[cfg(target_os = "linux")]
fn make_bus(args: &Args) -> Result<Box<dyn HostBus>, String> {
    match &args.backend {
        Backend::Serial { device, baud } => Ok(Box::new(SerialBus::new(device, *baud).map_err(|e| e.to_string())?)),
        Backend::I2c { device, address } => Ok(Box::new(I2cBus::new(device, *address, args.timeout).map_err(|e| e.to_string())?)),
        Backend::Can { interface, node_id } => Ok(Box::new(SocketCanBus::new(interface, *node_id).map_err(|e| e.to_string())?)),
        Backend::Memory(_) => Ok(Box::new(InMemoryBus::new())),
    }
}

#[cfg(not(target_os = "linux"))]
fn make_bus(args: &Args) -> Result<Box<dyn HostBus>, String> {
    match &args.backend {
        Backend::Memory(_) => Ok(Box::new(InMemoryBus::new())),
    }
}

fn run(args: Args) -> Result<(), String> {
    let bytes = std::fs::read(&args.firmware).map_err(|e| format!("read firmware: {e}"))?;
    #[cfg(target_os = "linux")]
    let bus = make_bus(&args)?;
    #[cfg(not(target_os = "linux"))]
    let bus = make_bus(&args)?;

    run_flash(bus, &bytes, args.tries, args.verify, |_| {}, |line| println!("{line}"))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn main() -> ExitCode {
    match parse_args().and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
