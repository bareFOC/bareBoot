/* Pin the RTT control block + its buffer to a low scratch slot in RAM,
 * clear of .bss (starts at 0x20000000) and the stack (descends from
 * 0x20005000). Block spans 0xD0 bytes so it ends well below the stack. */
SECTIONS
{
    .rtt_place 0x20000400 :
    {
        KEEP(*(.rtt_block))
        KEEP(*(.rtt_buffer))
    } > RAM
}
INSERT BEFORE .data;
