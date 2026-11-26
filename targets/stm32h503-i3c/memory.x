MEMORY
{
  /* Bootloader slot: Sectors 0..1 (16 KiB: 0x0800_0000 .. 0x0800_4000) */
  FLASH : ORIGIN = 0x08000000, LENGTH = 16K
  /* Reserve top 48 bytes (0x2000_7FD0..0x2000_8000) for BootLoopGuard + SoftResetMailbox */
  RAM   : ORIGIN = 0x20000000, LENGTH = 32720
}
_stack_start = 0x20007FD0;
