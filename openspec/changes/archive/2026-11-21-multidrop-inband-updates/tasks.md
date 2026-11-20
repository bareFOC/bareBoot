## 1. Bootloader Firmware Enhancements

- [x] 1.1 Support reading configured `NODE_ID` from flash storage (fallback to `0x40`)
- [x] 1.2 Optimize I2C slave event loop to commit blocks asynchronously without clock stretching
- [x] 1.3 Validate cross-reset RAM mailbox detection (`0x5984E3FA6CA1589B`) for indefinite timeout override
- [x] 1.4 Validate failsafe blank-vector parking logic when application sector is unprogrammed

## 2. Host Multidrop Tooling

- [x] 2.1 Implement `host/scripts/multidrop_flashtool.py` with multi-node configuration parsing
- [x] 2.2 Add pre-flight bus inventory discovery over I2C/I3C
- [x] 2.3 Implement per-node selective handsfree trigger (`0xF0` strobe), flashing, and readback verification
- [x] 2.4 Add simulation test harness verifying multi-node sequential update flow

## 3. Documentation

- [x] 3.1 Update `docs/handsfree-app-updates.md` with multi-drop bus topology recommendations
- [x] 3.2 Add example multi-node configuration file in `configs/multidrop_example.yaml`
