// BareBoot persistent update commit state (RP2350 and STM32)
//
// The commit record lives in a dedicated flash sector outside the application slot:
// - RP2350: last 4 KB flash sector of the 32 KB bootloader slot (flash offset 0x7000)
// - STM32: high NVM flash sector (Sector 15 / high flash)
//
// Sequential monotonic records allow multiple updates before an erase is needed.
// A transfer that is interrupted before COMPLETE leaves the state
// UPDATE_IN_PROGRESS (or blank/erased), so application_check_valid refuses
// to jump to a torn or uncommitted application image.
#include "commit_state.h"
#include "autoconf.h" // CONFIG_FLASH_START, CONFIG_FLASH_SIZE

#if CONFIG_MACH_RP2350
#include "hardware/flash.h"
#define COMMIT_OFFSET 0x7000
#define COMMIT_SECTOR_SIZE 4096
#define COMMIT_ADDR (CONFIG_FLASH_START + COMMIT_OFFSET)
#elif CONFIG_MACH_STM32
#define COMMIT_SECTOR_SIZE 8192
#define COMMIT_ADDR (CONFIG_FLASH_START + CONFIG_FLASH_SIZE - COMMIT_SECTOR_SIZE)
void flash_erase_page_raw(uint32_t page_address);
void flash_write_record_raw(uint32_t addr, const uint32_t *words, uint32_t count);
#endif

struct commit_record {
    uint32_t magic;
    uint32_t state;
    uint32_t complement;
    uint32_t inverse_magic;
};

#define RECORD_WORDS (sizeof(struct commit_record) / 4)

static void
write_record(uint32_t state)
{
    struct commit_record rec = {
        .magic = BAREBOOT_COMMIT_MAGIC,
        .state = state,
        .complement = ~state,
        .inverse_magic = ~((uint32_t)BAREBOOT_COMMIT_MAGIC),
    };

    const volatile struct commit_record *p =
        (const volatile struct commit_record *)COMMIT_ADDR;
    uint32_t max_records = COMMIT_SECTOR_SIZE / sizeof(struct commit_record);
    uint32_t target_slot = max_records;

    for (uint32_t i = 0; i < max_records; i++) {
        if (p[i].magic == 0xFFFFFFFF) {
            target_slot = i;
            break;
        }
    }

#if CONFIG_MACH_RP2350
    if (target_slot >= max_records) {
        flash_range_erase(COMMIT_OFFSET, COMMIT_SECTOR_SIZE);
        target_slot = 0;
    }
    flash_range_program(COMMIT_OFFSET + target_slot * sizeof(struct commit_record),
                        (const uint8_t *)&rec, sizeof(rec));
#elif CONFIG_MACH_STM32
    if (target_slot >= max_records) {
        flash_erase_page_raw(COMMIT_ADDR);
        target_slot = 0;
    }
    flash_write_record_raw(COMMIT_ADDR + target_slot * sizeof(struct commit_record),
                           (const uint32_t *)&rec, RECORD_WORDS);
#endif
}

int
commit_state_valid(void)
{
    const volatile struct commit_record *p =
        (const volatile struct commit_record *)COMMIT_ADDR;
    uint32_t max_records = COMMIT_SECTOR_SIZE / sizeof(struct commit_record);
    const volatile struct commit_record *last_rec = 0;

    for (uint32_t i = 0; i < max_records; i++) {
        if (p[i].magic == 0xFFFFFFFF) {
            break;
        }
        last_rec = &p[i];
    }

    if (!last_rec)
        return 0; // Sector is erased or blank

    if (last_rec->magic != BAREBOOT_COMMIT_MAGIC)
        return 0;
    if (last_rec->inverse_magic != (uint32_t)~BAREBOOT_COMMIT_MAGIC)
        return 0;
    if (last_rec->complement != (uint32_t)~last_rec->state)
        return 0;

    return last_rec->state == BAREBOOT_STATE_VALID;
}

void
commit_state_begin(void)
{
    write_record(BAREBOOT_STATE_UPDATE_IN_PROGRESS);
}

void
commit_state_commit(void)
{
    write_record(BAREBOOT_STATE_VALID);
}
