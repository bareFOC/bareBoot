# BareBoot

A Katapult-compatible bootloader, targeting STM32, CH32, RP, and WCH-compatible MCUs. Supporting interfaces include CAN, USB, UART, I2C, and I3C (availability depends on the selected target).

## Supported Devices

Legend: `[x]` verified on hardware, `[~]` supported / in progress, `[ ]` pending, `[-]` not available for this family.

### STM32

| MCU family | USB | UART | I2C | I3C | CAN |
|---|---:|---:|---:|---:|---:|
| STM32F0 | [~] | [~] | [~] | [-] | [ ] |
| STM32F1 | [~] | [x] | [~] | [-] | [~] |
| STM32F2 | [~] | [~] | [~] | [-] | [~] |
| STM32F4 | [~] | [~] | [~] | [-] | [~] |
| STM32F7 | [~] | [~] | [~] | [-] | [~] |
| STM32G0 | [ ] | [~] | [~] | [~] | [~] |
| STM32G4 | [~] | [~] | [~] | [ ] | [~] |
| STM32H5 | [~] | [x] | [x] | [~] | [~] |
| STM32H7 | [~] | [~] | [~] | [~] | [~] |
| STM32L4/L5 | [~] | [~] | [~] | [ ] | [~] |
| STM32U5 | [~] | [~] | [~] | [~] | [~] |

### WCH (CH32 / CH57x / CH58x / CH59x / CH64x)

| MCU family | USB | UART | I2C | I3C | CAN |
|---|---:|---:|---:|---:|---:|
| CH32F1 family | [ ] | [~] | [ ] | [-] | [ ] |
| CH32V00x family | [-] | [~] | [ ] | [-] | [ ] |
| CH32V103 | [~] | [~] | [~] | [-] | [~] |
| CH32V203 / CH32V205 / CH32V208 | [~] | [~] | [~] | [-] | [~] |
| CH32V303 / V305 / V307 | [~] | [~] | [~] | [-] | [~] |
| CH32V317 | [~] | [~] | [~] | [-] | [~] |
| CH32V4X7 / CH32H41X | [ ] | [~] | [~] | [ ] | [-] |
| CH32L103 / X0 / X3 | [ ] | [~] | [ ] | [-] | [ ] |
| CH641 / CH643 / CH645 | [ ] | [?] | [?] | [ ] | [ ] |
| CH56x / CH57x / CH58x / CH59x | [ ] | [~] | [-] | [-] | [ ] |

### Puya (PY32)

| MCU family | USB | UART | I2C | I3C | CAN |
|---|---:|---:|---:|---:|---:|
| PY32F0xx (PY32F002A / F003 / F030) | [-] | [~] | [~] | [-] | [-] |
| PY32F072 | [~] | [~] | [~] | [-] | [~] |

### Raspberry Pi

| MCU family | USB | UART | I2C | I3C | CAN |
|---|---:|---:|---:|---:|---:|
| RP2040 | [~] | [~] | [~] | [ ] | [~] |
| RP2350 | [x] | [~] | [~] | [ ] | [ ] |

## Monorepo Layout

```
bareBoot/
├── Cargo.toml               # Virtual workspace manifest
├── crates/                  # Common platform-independent firmware libraries
│   ├── bareboot-protocol    # Katapult-compatible wire framing, CRC, and commands
│   └── bareboot-core        # Core bootloader runtime traits, dispatcher, paged flash writer
├── targets/                 # Concrete MCU bootloader targets
│   ├── rp2350-usb           # RP2350 USB CDC bootloader with BareBoot recovery
│   ├── ch32v203-usb         # CH32V203 USB CDC bootloader
│   └── stm32h503            # STM32H503 C dynamic target sources, patches, and configs
├── host/                    # Host-side flashing tools and scripts
│   ├── bareboot-client      # Headless Katapult flashing client library
│   ├── bareboot-cli         # Command-line flashing tool
│   ├── bareboot-tui         # Interactive terminal cockpit for BareBoot
│   └── scripts/             # Flashing automation and helper scripts (Python, Bash)
├── configs/                 # Target board definition configuration profiles (*.toml)
├── artifacts/               # Precompiled bootloader binaries and ELF images
├── tests/                   # Hardware test apps, probes, and integration test crates
└── docs/                    # Technical and architecture documentation
```

## Build & Test

### Host Workspace (Core Crates + Host Tools)

```sh
# Build host tools and shared core libraries
cargo build --workspace

# Run all host unit tests
cargo test --workspace
```

### Host Flasher CLI

```sh
cargo run -p bareboot-cli -- --firmware path/to/firmware.bin --serial /dev/ttyACM0
# or --can can0 --node-id 0x100, --i2c /dev/i2c-1 --address 0x40, --memory
```

### Host TUI Cockpit

```sh
cargo run -p bareboot-tui
```

Tabs: Bus, Config, Build, Flash (wizard), Profiles. Keys: `1-5` tabs, `n/p` wizard steps, `m/e/i/c` backend, `r` run, `w` save config, `l/s/d` profiles, `q` quit.

### Target Firmware (Embedded)

Target bootloaders reside in `targets/` with isolated target toolchains:

```sh
# RP2350 USB CDC bootloader
cd targets/rp2350-usb
cargo test  # runs host unit tests
cargo build --release --target thumbv8m.main-none-eabihf

# CH32V203 USB bootloader
cd targets/ch32v203-usb
cargo build --release --target riscv32imac-unknown-none-elf
```

## Configuration

Target board definitions are stored under `configs/`:

```toml
# configs/bareboot.toml
target = "ch32v203"
transport = "can"   # can | i2c | uart | usb
boot_window_ms = 300
```

Flash profiles are stored under `./profiles/*.toml`.

## Documentation

- [Handsfree App Updates](docs/handsfree-app-updates.md): Detection, software jump into bootloader, cross-reset RAM mailbox, and Katapult interoperability.

## License

MIT OR Apache-2.0
