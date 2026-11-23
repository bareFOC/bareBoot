# Tasks

## 1. Engine & Client Export Stabilization

- [x] 1.1 Verify and clean `crates/bareboot-engine` public API exports (`WorkerHandles`, `WorkerCommand`, `UiEvent`, `FlashProfile`) and verify `cargo check -p bareboot-engine` passes
- [x] 1.2 Implement dual TOML parser in `crates/bareboot-engine` supporting both `bareboot.toml` and `barefoc.toml` (`[bootloader]` section) with unit tests
- [x] 1.3 Implement `initial_safe_pins` code generator / macro in `crates/bareboot-core` to drive safe gate-driver states on startup
- [x] 1.4 Implement multi-interface transport arbitration (concurrent polling and shared-pin time-slicing) in `crates/bareboot-core/src/dispatch.rs`
- [x] 1.5 Verify `host/bareboot-client` bus abstractions (`HostBus`, `SerialBus`, `SocketCanBus`, `I2cBus`) and verify `cargo test -p bareboot-client` passes

## 2. Workspace Deprecation & Cleanup

- [x] 2.1 Remove `host/bareboot-gui` from `bareBoot/Cargo.toml` workspace members and archive `host/bareboot-gui` assets
- [x] 2.2 Verify workspace compilation without `bareboot-gui` via `cargo check --workspace` and `cargo test --workspace`

## 3. Documentation & Link Redirection

- [x] 3.1 Update `bareBoot/README.md` to reference `bareConfig` in `bareFOC` as the official graphical workbench
- [x] 3.2 Update `docs/bareboot-gui.md` with migration note to `bareFOC/bareConfig`
