## ADDED Requirements

### Requirement: STM32H503 16 KiB Bootloader Slot & Dual-Sector WRP
`bareboot-stm32h503-i3c` SHALL reside in Sectors 0–1 (`0x0800_0000..0x0800_3FFF`, `16 KiB`), set `APP_BASE = 0x0800_4000` (`88 KiB` app slot up to `0x0801_A000`), and write-protect both Sector 0 and Sector 1 (`WRP[1:0] = 00`) with `BOR = 3`.

#### Scenario: `CMD_CONNECT` reports `0x08004000` and WRP protects Sectors 0–1
- **WHEN** `bareboot-stm32h503-i3c` boots and responds to `CMD_CONNECT`
- **THEN** `app_base` is `0x0800_4000`, `FLASH_WRPSGN1R` bits `[1:0]` are `00`, and writes below `0x0800_4000` are rejected
