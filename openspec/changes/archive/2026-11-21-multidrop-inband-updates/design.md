## Context

Modern robotics architectures connect multiple joint actuators to a single shared bus (e.g. I2C, I3C, CAN-FD, RS-485). When updating firmware in production, each actuator node must be individually addressable, flashable, and verifiable over this shared medium without physical access or bus conflicts.

## Goals / Non-Goals

**Goals:**
- Provide selective single-node handsfree bootloader entry over shared I2C (`0xF0` strobe), I3C (`RSTACT` CCC or `0xF0`), and MultiBus.
- Prevent clock stretching on shared I2C/I3C lines to avoid starving or locking other bus traffic.
- Use top-of-RAM mailbox tokens (`0x5984E3FA6CA1589B`) across warm resets to park the targeted node in BareBoot indefinitely while unaddressed nodes remain running.
- Provide a Python batch flasher that sequences multi-node rollouts safely.

**Non-Goals:**
- Simultaneous parallel flashing of multiple nodes on a single half-duplex bus (updates are strictly serialized to avoid packet collision).

## Decisions

### Decision 1: Serialized Node Rollout with Pre-Flight Inventory
* **Rationale**: Flashing node-by-node ($Node_1 \rightarrow Node_2 \rightarrow \dots$) eliminates bus collisions and reduces host memory footprint. Pre-flight scanning verifies all nodes are reachable before taking any node offline.

### Decision 2: Elimination of Clock Stretching
* **Rationale**: If a node in bootloader mode holds SCL low while writing flash quad-words, the entire I2C/I3C bus freezes. BareBoot must buffer incoming blocks and only commit to flash when the bus transaction completes, or complete quad-word commits in < 15 µs between transfers.

### Decision 3: Failsafe Blank-Vector Parking
* **Rationale**: If flashing is interrupted midway, the application sector vector table will be invalid. BareBoot detects invalid stack pointers or entry addresses and stays parked in bootloader mode permanently upon power-on, allowing handsfree recovery without an SWD debugger.

## Risks / Trade-offs

- **[Risk] Sibling Node Interference**: An unaddressed node misinterpreting flash traffic as normal commands.
  - *Mitigation*: Flashing frames use Katapult / BareBoot frame protocol headers (`CONNECT 0x11`, `SEND_BLOCK 0x12`) exclusively directed to the target node's hardware slave address. Sibling nodes ignore transactions not addressed to them.
- **[Risk] Bus Collision on Simultaneous Reset**:
  - *Mitigation*: Only the specifically addressed node receives the reset trigger. Broadcast reset is forbidden during targeted updates.
