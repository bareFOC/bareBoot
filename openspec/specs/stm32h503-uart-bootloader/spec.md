# stm32h503-uart-bootloader Specification

## Purpose
Provides Katapult-compatible UART bootloader transport, interrupt-driven reception, 8 KB sector flash programming, verification, and application handoff for STM32H503 microcontrollers.

## Requirements

### Requirement: H503 UART transport
The bootloader SHALL initialize USART1 on PB6 (TX) / PB7 (RX) at 115200 8N1 using AF7 and an internal 64 MHz HSI clock, and SHALL receive bytes through an interrupt-driven 256-byte ring buffer.

#### Scenario: Host sends a frame
- **WHEN** the host sends a valid Katapult frame over the WCH-LinkE UART bridge
- **THEN** the bootloader receives all bytes without loss even while a flash sector erase is in progress

### Requirement: H5 flash programming
The bootloader SHALL erase and program STM32H503 flash in 8 KB sectors using the non-secure flash controller, writing 16-byte words, and SHALL verify each programmed page by readback before reporting success.

#### Scenario: Program a page
- **WHEN** an RX_BLOCK frame targets a valid address within the application region
- **THEN** the bootloader erases the affected 8 KB sector on first write, programs 16-byte words, and reports success only if readback matches

#### Scenario: Invalid address
- **WHEN** a frame targets an address below `app_base` (0x08004000) or beyond flash size
- **THEN** the bootloader rejects the operation with an error and does not modify flash

### Requirement: Boot window and handoff
The bootloader SHALL wait 3000 ms for a valid CONNECT frame, and upon completion or timeout with a valid application vector table at `0x08004000`, SHALL jump to the application.

#### Scenario: Successful flash completes
- **WHEN** the host sends RX_EOF and COMPLETE
- **THEN** the bootloader jumps to the application at `0x08004000` after a short delay

#### Scenario: No host activity
- **WHEN** no valid frame arrives within 3000 ms and a valid vector table exists at `0x08004000`
- **THEN** the bootloader jumps to the application

#### Scenario: No valid application
- **WHEN** the initial stack pointer at `0x08004000` is outside SRAM
- **THEN** the bootloader stays in bootloader mode

### Requirement: Size budget
The bootloader binary SHALL fit within 16 KB so that `app_base` remains `0x08004000`.

#### Scenario: Build size check
- **WHEN** the release build completes
- **THEN** the resulting `.bin` is no larger than 16384 bytes
