# Design: `stm32h503-i3c-16kb-layout`

## STM32H503 128 KiB Flash Partitioning
- **Sectors 0–1 (`0x0800_0000..0x0800_3FFF`, 16 KiB)**: `bareboot-stm32h503-i3c`, hardware write-protected via `FLASH_WRPSGN1R` bits `[1:0] = 00`.
- **Sectors 2–12 (`0x0800_4000..0x0801_9FFF`, 88 KiB)**: Application slot (`APP_BASE = 0x0800_4000`, `APP_END = 0x0801_A000`).
- **Sector 13 (`0x0801_A000..0x0801_BFFF`, 8 KiB)**: `MonotonicCommitJournal` (`CommitStore`).
- **Sector 14 (`0x0801_C000..0x0801_DFFF`, 8 KiB)**: Reserved / backup.
- **Sector 15 (`0x0801_E000..0x0801_FFFF`, 8 KiB)**: `H5FlashStorage` NVM config.
