## Context

The project starts from an empty workspace. The only existing behavior reference is the Katapult C bootloader and its newer STM32/I2C host updates under `ref/katapult`. Katapult is proven, byte-compatible across CAN/USB/UART/I2C, and its host scripts are not Rust-native. BareBoot needs to preserve that protocol while adding CH32V203 support, declarative configuration, and a Rust host/TUI.

Constraints that matter most:
- Firmware must fit in 4 KB flash.
- Firmware must avoid allocator use and remain testable off-target.
- Each target must drive PAC registers directly; no heavy HAL stack.
- Host must support multi-drop CAN discovery, fixed-address I2C nodes, ordinary serial, and probe-based factory flashing.
- Runtime behavior must not depend on an operating system.

## Goals / Non-Goals

**Goals:**
- Preserve Katapult wire compatibility on CAN, USB, UART, and STM32-style I2C.
- Make transport selection compile-time and allow at most two active transports per 4 KB image.
- Keep protocol parsing, CRC, and command handling in a pure `no_std` crate with host tests.
- Support page-buffered target flash writes, read-back verification, UID checks, and application jump.
- Replace menuconfig with `bareboot.toml` plus generated constants/linker scripts.
- Provide a Rust host engine and optional ratatui cockpit.

**Non-Goals:**
- Full I3C target support in the first implementation.
- Reproducing Klipper's complete hardware abstraction.
- Dynamic runtime plugin loading.
- Rewriting the host protocol to optimize transport throughput beyond Katapult limits.
- Supporting simultaneous active traffic on both configured interfaces; the arbiter locks to the first one.

## Decisions

### D1: Use CRC16-CCITT, not CRC32

The Katapult frame reserves exactly 2 CRC bytes and every host implementation validates the standard Katapult CRC16. The `CCITT-32 / ISO-HDLC` wording in the early specification conflicts with that evidence. Rust firmware will implement the exact Katapult CRC16 polynomial update and host-side tests will use the Python/Rust reference vectors. CRC32 would break interoperability, exceed the frame field size, and add avoidable code.

Alternative considered: keep CRC32 internally and translate on the host. Rejected because it would make `bareflash` incompatible with all existing Katapult tools and gain no meaningful integrity over CRC16 plus block read-back.

### D2: Byte-for-byte Katapult framing

All transports will feed raw bytes into the same buffered frame parser. The frame is:

`01 88 CMD WC [payload...] CRCL CRCH 99 03`

Block protocol behavior, response codes, `CONNECT` metadata, app start address, block size, `GET_CANBUS_ID`, UUID bytes, admin IDs, and CAN node assignment will match `ref/katapult`. I2C behavior will follow the newer I2C target implementation: byte stream into a RX buffer, command scan on STOP, and clock-stretched TX response.

Alternative considered: transport-native request/response APIs. Rejected because they would duplicate protocol logic and prevent using standard Katapult host scripts.

### D3: Transport matrix rather than any-two generics

The early design proposed `ActiveInterface<T1, T2>`. For any-two combinations across CAN, I2C, UART, and USB, concrete generic pairs create combinatorial build complexity. The firmware will instead use a feature-gated enum such as `ActiveInterface` with variants compiled only for enabled transports. Core code still reasons about an undecided/listening/selected state, but each image pays only for its selected transports.

Alternative considered: separate images for every pair. Rejected because generated memory/config work should remain centralized and feature-driven.

### D4: Block size defaults to 64 bytes

Katapult's frame limit and host parser favor 64-byte blocks. Firmware may report 128 bytes when a target has enough flash speed and margin, but 64 bytes is the interoperable baseline. Block sizes above 192 bytes are not recommended because a full frame plus metadata exceeds practical receive buffers.

Alternative considered: fixed 128-byte programmer blocks. Rejected until CH32 flash and timing tests prove headroom under 4 KB.

### D5: Flash page buffering lives in the target driver

Host block writes are 64 or 128 bytes, while target flash write granularity may differ. CH32 Fast-Flash may require a 128-byte staging buffer. STM32 flash may require page-bound buffering. The target `FlashTarget` implementation owns this buffering and commits at page boundaries or EOF. The protocol core remains unaware of flash geometry.

Alternative considered: require host block size to match flash page size. Rejected because it would break Katapult interoperability and complicate multi-node setups.

### D6: Polling firmware with countdown timers

Bare metal startup uses `riscv-rt`/`cortex-m-rt` entry points, enables only required peripheral clocks, runs a countdown using a hardware timer, and polls transport RX buffers. I2C target handling will either use interrupts where practical or carefully poll while stretching SCL. Flash operations must keep the countdown and response deadlines in mind; busy responses and retries are acceptable.

Alternative considered: an RTOS or async runtime on target. Rejected because the strict 4 KB budget cannot absorb it.

### D7: Configuration is compile-time data, not runtime state

`bareboot.toml` feeds `build.rs`, which emits `memory.x`, strongly typed constants, and Cargo feature selections. This removes menuconfig and allows dead-code elimination for unused transports and pins.

Alternative considered: runtime discovery of pins and clocks. Rejected because it would use RAM/flash and weaken type safety.

### D8: Factory provisioning is host-side

Initial bootloader installation uses `probe-rs` and target probes. The firmware itself only supports Katapult in-application flashing. WCH-LinkE/ST-Link details should remain host concerns; firmware provides normal boot and jump behavior.

Alternative considered: firmware SWD self-flashing. Rejected because it conflicts with bootloader size and safety.

## Risks / Trade-offs

- [Katapult debug strings consume flash] -> Keep MCU/version strings short and validate size on every build.
- [Two transport images can exceed 4 KB] -> Default production nodes to one transport; make multi-transport a measured feature combination.
- [CH32 I2C polling may not stretch clocks reliably under flash load] -> Reuse interrupt-style state discipline or fall back to smaller boards and shorter flash milestones until a CH32 I2C target test confirms timing.
- [Flash page buffering can lose data on failed transfers] -> Commit only complete page buffers, track CRC per block, and require host read-back before `COMPLETE`.
- [probe-rs may lack complete WCH support] -> Keep factory flashing modular so OpenOCD or vendor tools can be substituted.
- [Admin CAN node assignment contains host/firmware timing assumptions] -> Implement reference behavior and validate against `flash_can.py` before supporting custom discovery optimizations.

## Open Questions

- Are CH32V203 I2C targets expected to be interrupt-driven or strictly polled in production?
- Which CH32V203 variant and flash geometry will be first, and what page size does its fast-flash routine require?
- Should bareflash replicate `flashtool.py` node-name blobs, or expose addresses directly as the source of truth?
- Which image combinations are required under 4 KB: CAN only, I2C only, CAN+I2C, UART only, or USB+UART?
