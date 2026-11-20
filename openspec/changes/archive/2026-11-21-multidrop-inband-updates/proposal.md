## Why

In production multi-axis systems (e.g. robotic joints, multi-motor actuators), several nodes share a single communication bus (such as I2C, I3C, CAN-FD, or RS-485). Updating device firmware in production currently requires disconnecting boards, wiring individual SWD debuggers, or risking bus contention when multiple nodes reset simultaneously.

BareBoot requires a standardized multidrop in-band update protocol allowing a single host gateway to address and update individual nodes sequentially across a shared physical bus without hardware intervention, bus clock stretching, or disturbance to sibling nodes.

## What Changes

- Add multi-drop bus isolation and selective in-band bootloader entry:
  - Standard 1-byte strobe `[0xF0]` targeting individual node addresses (`0x40`..`0x4F`).
  - MIPI I3C Direct CCC `RSTACT` (`0x9A`, defining byte `0x02`) directed to assigned dynamic addresses.
  - MultiBus Control Table register trigger (`0x0B = 0x01`).
- Enforce non-blocking bus slave operation in BareBoot (DMA or zero-latency interrupt handling) to strictly prevent bus clock stretching on shared I2C/I3C lines.
- Implement the cross-reset SRAM mailbox protocol (`0x5984E3FA6CA1589B` at `bl_vectors[0]`) to bypass the 250 ms autoboot timeout for targeted nodes.
- Author a Python host multi-drop orchestrator script (`tools/multidrop_flashtool.py`) supporting sequential batch updating, pre-flight bus inventory, and per-node CRC verification.

## Capabilities

### New Capabilities
- `multidrop-inband-flashing`: Multi-node bus addressing, selective handsfree bootloader entry, collision-free block programming, and Python host batch updater.

### Modified Capabilities
- `stm32h503-i2c-bootloader`: Ensure I2C slave event handling avoids bus clock stretching and respects configured node addresses.

## Impact

- `bareboot` firmware: Non-blocking I2C/I3C event handling and node address binding.
- `host/scripts`: New `multidrop_flashtool.py` and enhancements to `flashtool.py`.
- Documentation: New handsfree multidrop deployment guide in `docs/`.
