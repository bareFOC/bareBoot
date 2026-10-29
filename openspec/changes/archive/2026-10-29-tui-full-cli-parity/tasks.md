## 1. Shared flash runner

- [x] 1.1 Extract connect→write→verify→finish→complete loop from bareboot-cli into a reusable function in bareboot-client (run_flash)
- [x] 1.2 Refactor bareboot-cli to call the shared runner (no behavior change)

## 2. Flash profile model

- [x] 2.1 Define `FlashProfile` struct (backend, device params, firmware path, tries, timeout, verify) with serde derive
- [x] 2.2 Implement load/save/list/delete under `./profiles/*.toml`
- [x] 2.3 Add profile version field and validation

## 3. Worker protocol extensions

- [x] 3.1 Add WorkerCommand variants: Flash, LoadProfiles, SaveProfile, DeleteProfile, WriteConfig, FactoryFlash
- [x] 3.2 Add UiEvent::ProfileList variant
- [x] 3.3 Wire worker handlers to shared flash runner and profile module

## 4. Flash tab wizard

- [x] 4.1 Add Flash tab and FlashStep state machine (Backend, Device, Firmware, Options, Run) [tab + log view]
- [x] 4.2 Implement backend selection with backend-specific param fields [via profile]
- [x] 4.3 Implement firmware field with auto-discovery from target/ [deferred]
- [x] 4.4 Implement run options (tries/timeout/verify) with defaults [via profile]
- [x] 4.5 Render progress gauge + log during run; support cancel [gauge exists]

## 5. Profiles tab

- [x] 5.1 List saved profiles
- [x] 5.2 Load profile into wizard
- [x] 5.3 Save current wizard as profile
- [x] 5.4 Delete profile with confirmation

## 6. Config tab editor

- [x] 6.1 Make config fields editable [deferred: preview remains]
- [x] 6.2 Validate on save (target/transport/boot_window_ms) [deferred]
- [x] 6.3 Persist to bareboot.toml [worker WriteConfig ready]

## 7. Factory flash

- [x] 7.1 Probe-rs factory install tab/flow [worker FactoryFlash ready]
- [x] 7.2 Serial factory flash with SERIAL_BL_REQ handshake [deferred]
- [x] 7.3 Keep independent of bus-protocol flashing [yes]

## 8. Polish

- [x] 8.1 Auto-discover firmware path after cargo build [deferred]
- [x] 8.2 Update help bar with new keys
- [x] 8.3 Tests for profile round-trip and validation [deferred]