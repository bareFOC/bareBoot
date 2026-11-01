## Context

BareBoot is a Katapult-compatible bootloader. The STM32F1 target (`bareboot/targets/bareboot-stm32f1`) is the closest existing template: raw-register no_std firmware, USART1 on PB6/PB7, `FlashTarget` + `PagedWriter`, boot window, application handoff. The H503 differs in three ways that force a new target rather than a feature flag:

- Cortex-M33 with H5 flash controller (non-secure/secure split, 8 KB sectors, 16-byte program granularity).
- HSI 64 MHz default clock (F1 runs at 8 MHz).
- Different GPIO/AF mux and RCC layout (AHB2 for GPIO, AHB2ENR).

Target hardware is STM32H503KBU6 (32-pin UFQFPN, 128 KB flash, 32 KB SRAM). Host link is a WCH-LinkE USB-UART bridge on USART1 (PB6 TX / PB7 RX). Verification uses an embassy-stm32 blink firmware toggling LED_B on PB8.

## Goals / Non-Goals

**Goals:**
- UART-only bootloader for H503 that speaks the existing Katapult frame protocol.
- Erase/program 8 KB H5 flash sectors with readback verification.
- Boot-window handoff to a valid application vector table at `0x08004000`.
- Size budget ≤ 16 KB so the bootloader fits below `app_base`.

**Non-Goals:**
- USB, I2C, CAN, or I3C transports on H5 (UART first; others later).
- Secure-mode flash operations or option-byte changes.
- Changing the Katapult wire protocol or host CLI.

## Decisions

1. **Transport scope: UART only.**
   - Rationale: matches the available WCH-LinkE bridge and the existing F1 UART path; minimizes risk on a new MCU family.
   - Alternative: port I2C too. Rejected for now to keep the first H5 image small and testable.

2. **App offset `0x08004000` (16 KB).**
   - Rationale: two 8 KB sectors give headroom for the larger H5 init code and the 16-byte flash program path; avoids a 8 KB squeeze.
   - Alternative: `0x08002000`. Rejected: leaves no margin if the H5 image exceeds 8 KB.

3. **Clock: internal HSI 64 MHz.**
   - Rationale: no external crystal dependency; `BRR = 0x22C` for 115200; `dwt_cycles_per_ms = 64000`.
   - Alternative: external HSE. Rejected: unknown board crystal; HSI is always available.

4. **Flash access via `stm32-metapac` PAC (non-secure registers).**
   - Rationale: H5 bit positions differ from F1; PAC reduces transcription errors.
   - Alternative: raw register addresses. Rejected: higher risk, harder to review.

5. **IRQ-driven RX ring buffer (256 bytes), copied from F1.**
   - Rationale: sector erase stalls the CPU for ~100 ms; polling would drop bytes.
   - Idle resync timeout 100 ms retained for paced WCH-Link sends.

6. **Page size 8192 in `PagedWriter`.**
   - Rationale: must equal H5 erase sector size (required by `PagedWriter` assertion `flash.page_size() == PAGE`). 8192 is a multiple of the 16-byte program granularity, so final-page padding is safe.

7. **Verification LED: LED_B on PB8.**
   - Rationale: PB8 is reliably bonded on the 32-pin package; PC14 (LED_R) may not be bonded on UFQFPN32.

## Risks / Trade-offs

- [PC14 not bonded on UFQFPN32] → Verification relies on PB8; LED_R story is best-effort.
- [Sector erase latency ~100 ms] → IRQ ring buffer + 100 ms idle resync; host must pace frames.
- [H5 non-secure mode assumption] → Assumes default option bytes; secured boards will fault on NS writes.
- [Bootloader size overrun] → Spike build first; fallback to 24 KB offset if needed.
- [Readback check is only 4 bytes] → Catches gross programming failures but not per-word corruption; acceptable for v1.

## Migration Plan

1. Add `bareboot/targets/bareboot-stm32h503` to the workspace and a `bareboot-stm32h503.toml` config.
2. Implement UART + flash + boot jump; build and measure size.
3. Flash bootloader via probe-rs over SWD.
4. Build and flash blink firmware at `0x08004000`.
5. Run host flash flow over WCH-LinkE UART; confirm LED_B blinks after `COMPLETE`.

Rollback: re-flash the previous firmware image over SWD; no persistent state changes beyond flash contents.

## Open Questions

- Exact H5 UID register address (verify against metapac before implementing `read_uid`).
- Whether the size spike fits under 16 KB; if not, revisit offset.
- Whether PC14 is usable on the Miniata board.