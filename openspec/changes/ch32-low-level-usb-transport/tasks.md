## 1. Survey and boundaries
- [x] 1.1 Confirm which RISC-V register assumptions are identical to the existing low-level path.
- [x] 1.2 Confirm the minimum CDC descriptor tables and alternate endpoints needed by the target.
- [x] 1.3 Confirm the exact watchdog transaction playbook (CONNECT, BLOCK, EOF, COMPLETE).

## 2. Low-level transport
- [x] 2.1 Implement register-driven EP0/EP1/EP2/CDC descriptors in Rust.
- [x] 2.2 Implement byte enqueue into a host-neutral response buffer.
- [x] 2.3 Implement an explicit ACK staging pass for each catapult command.

## 3. Hardware validation
- [ ] 3.1 Flash a basic USB CDC transaction and confirm host discovery.
- [ ] 3.2 Verify one transaction through EOF with a paced host sequence.
- [ ] 3.3 Verify the app vector at `0x08001000` and an observable application jump.

## 4. Review
- [x] 4.1 Review the low-level PDF against Katapult-style flow for worst-case timing.
