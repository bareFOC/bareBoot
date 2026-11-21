# Design

## Context

BareBoot provides Katapult-compatible bootloading across multi-drop buses. See [proposal.md](proposal.md) for background and motivation. Existing targets use either direct flash traits or bespoke C glue. In flash-constrained microcontrollers, dual-partition A/B swapping is impractical. Furthermore, upstream Katapult does not support I2C or MIPI I3C targets. This design establishes a unified architecture for single-slot in-situ Katapult flashing with watchdog safety, monotonic commit state tracking, non-blocking I2C/I3C target communication, guaranteed NVM retention across OTAs, and hardware verification using FishyServo and AgentLA.

## Goals / Non-Goals

**Goals:**
- Single application slot in-situ flash programming with 100% Katapult framing compatibility (`bareboot-protocol`).
- Non-Volatile Memory (NVM) persistence: protect calibration data, motor zero offsets, PID parameters, and node IDs in `H5FlashStorage` (`0x0801_C000..0x0802_0000`) across all OTA transfers.
- Power-fail safe recovery trap: an interrupted transfer leaves BareBoot in recovery indefinitely; corrupt images are never executed.
- Zero-erase monotonic commit state updates to eliminate vulnerability to brownouts during sector erases.
- Continuous hardware watchdog feeding during multi-sector erase and write operations using the `WatchdogFlash` pattern.
- Robust processor handoff on ARM Cortex-M7/M33 and RISC-V targets including instruction cache invalidation.
- Non-blocking I2C target framing that eliminates SCL clock stretching on master controllers.
- Native MIPI I3C dynamic target driver with CCC `RSTACT` hardware reset and dynamic address assignment (`ENTDAA`).
- Live hardware verification on the connected FishyServo actuator via the AgentLA test harness.

**Non-Goals:**
- Implementing dual-bank A/B slot swapping or automatic firmware rollbacks (single-slot recovery only).
- Modifying stock Katapult wire framing or command opcodes (`0x11` through `0x16`).
- Erasing or repaving NVM sectors during standard firmware updates.

## Decisions

### 1. Single Application Slot with Guaranteed Recovery Trap
- **Decision**: Program incoming Katapult 64-byte blocks directly into the target application flash slot via `PagedWriter`. When an update starts, mark commit state `UPDATE_IN_PROGRESS`. Only mark `STATE_VALID` upon successful completion and verification.
- **Rationale**: Eliminates the 50% flash storage penalty of A/B staging partitions while guaranteeing that any power loss or communication failure leaves the device in bootloader recovery mode waiting for host re-flashing.
- **Alternatives Considered**: Dual-partition swap (rejected due to excessive flash overhead on microcontrollers with 16 KB to 128 KB flash).

### 2. NVM Flash Partition Isolation
- **Decision**: The bootloader flash backend defines strict bounds:
  - `APP_BASE = 0x0800_2000`
  - `APP_END = 0x0801_C000` (104 KB maximum application size)
  - `NVM_STORAGE = 0x0801_C000 .. 0x0802_0000` (16 KB reserved for `H5FlashStorage`)
  All flash erase and write operations check target addresses against `[APP_BASE, APP_END)`. Any block targeting `addr >= APP_END` is rejected with `CoreError::InvalidAddress`.
- **Rationale**: Actuator nodes require that factory calibration (motor phase resistance, encoder zero offset, PID tuning) and assigned `node_id` persist permanently across OTA firmware updates. Isolating NVM outside the application write window guarantees safety.
- **Alternatives Considered**: Storing calibration in application flash (rejected because every update would overwrite calibration).

### 3. Monotonic Bit-Flipping Commit Journal
- **Decision**: Dedicate a 4 KB flash sector where update state transitions write sequential 32-bit words (`0xFFFFFFFF` erased -> `0x4242_0001` in progress -> `0x4242_0002` valid) without erasing the sector on every state change.
- **Rationale**: NOR flash allows programming bits from 1 to 0 without erasing. Performing a 30 ms flash sector erase during a state change leaves an open window where brownouts can corrupt the sector. Sequential word writes reduce erase wear and take microseconds.
- **Alternatives Considered**: Erasing the sector on every state change (rejected due to brownout vulnerability and unnecessary sector endurance degradation).

### 4. Automated Watchdog Feeding (`WatchdogFlash` Pattern)
- **Decision**: Wrap the target flash driver with a decorator that services the hardware watchdog on every page erase and block write.
- **Rationale**: Writing large firmware over slow buses (e.g. 100 kbps I2C or CAN) can exceed watchdog timeout periods (often 1 to 2 seconds). Decorating flash operations guarantees the watchdog is fed transparently during long programming sequences.
- **Alternatives Considered**: Relying on periodic host packets (rejected because multi-sector erase operations can block execution longer than the watchdog threshold).

### 5. Instruction Cache Invalidation and Clean Bootload
- **Decision**: On Cortex-M7, Cortex-M33, and RISC-V targets, explicitly execute `p.SCB.invalidate_icache()`, reset the VTOR register, and de-initialize active communication peripherals before branching to the application entry.
- **Rationale**: Rewriting flash modifies code memory while CPU instruction caches hold stale prefetched lines. Invalidation prevents immediate `HardFault` errors upon application handoff.
- **Alternatives Considered**: Relying on software resets (not all MCUs reset cache state on soft resets).

### 6. Decoupled Non-Blocking I2C & I3C Target Architecture
- **Decision**: The I2C/I3C peripheral receives incoming command frames into an internal RAM buffer and acknowledges transfers immediately without clock-stretching SCL. Flash programming proceeds in the background, and responses are staged in a TX buffer. On I3C, completion triggers an In-Band Interrupt (IBI).
- **Rationale**: Host controllers such as the Broadcom SoC on Raspberry Pi suffer from hardware errata when I2C targets stretch SCL clocks. Decoupled buffering guarantees compatibility across all host master devices.
- **Alternatives Considered**: Hardware SCL clock stretching (rejected due to host controller incompatibilities).

### 7. Dynamic I2C / I3C Bus Switching
- **Decision**: On targets supporting both I2C and I3C on shared pins (e.g. STM32H5), start the peripheral in I2C target mode. If the broadcast address `0x7E` is received, dynamically reassign GPIO alternate functions to the I3C peripheral.
- **Rationale**: Allows the same bootloader firmware image to operate in legacy I2C environments or high-speed I3C topologies without re-compilation.

## Hardware Verification Setup (FishyServo + AgentLA)

The test bench consists of:
- **Host Controller**: AgentLA (`http://10.10.10.10`, RP2350A) managing I2C master on GPIO 10 (SDA) / GPIO 11 (SCL) and SWD debugging on GPIO 4 / 5.
- **Target Node**: FishyServo ([FishyServo](file:///home/sun/Filelink/Firmwares/FishyServo), STM32H503KBU6) running `n20-servo` firmware on host I2C2 pins PB3 (SCL) / PB4 (SDA) at address `0x40`.
- **Update Verification**:
  1. Bootloader flashing onto `0x0800_0000` via SWD.
  2. Initial application flashing into `0x0800_2000`.
  3. Handsfree update over I2C using `0xF0` strobe and `flash_app_over_i2c.py`.
  4. NVM verification: assert that `H5FlashStorage` sectors `0x0801_C000..0x0802_0000` retain calibration values after OTA update.
  5. Dynamic I3C switching and `RSTACT` reset verification via `flash_app_over_i3c.py`.
  6. Aborted update injection to verify permanent recovery parking.

## Risks / Trade-offs

- **[Risk: Flash Sector Exhaustion in Commit Sector]** → If updates are repeatedly aborted, the monotonic sector fills up.  
  *Mitigation*: When the sector offset reaches the end of the 4 KB page, the sector is cleanly erased and restarted from offset 0.
- **[Risk: I2C Host Read Before Flash Operation Complete]** → An I2C master may attempt to read the response frame before the target completes page writing.  
  *Mitigation*: The host client (`bareboot-cli`, `multidrop_flashtool.py`) implements a calibrated turnaround delay (20 ms), and the target returns `COMMAND_BUSY` (`0xf3`) if read prematurely.
- **[Risk: Accidental Erasure of NVM]** → Flashing a large binary could overrun into `H5FlashStorage`.  
  *Mitigation*: Hardware boundary checks reject any block at or above `0x0801_C000`, and `CONNECT` payload advertises a block limit corresponding strictly to `APP_SIZE`.

## Migration Plan

1. Implement `WatchdogFlash`, monotonic commit state, and address boundary enforcement in `crates/bareboot-core`.
2. Update target boot routines across STM32, RP2350, and CH32 targets with instruction cache invalidation.
3. Integrate the non-blocking I2C target and MIPI I3C dynamic target drivers into `targets/` and host tools (`bareboot-cli`, `bareboot-client`).
4. Execute hardware verification runs on FishyServo via AgentLA, confirming handsfree updates and NVM calibration persistence.
