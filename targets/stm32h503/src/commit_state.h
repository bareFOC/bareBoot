// BareBoot persistent update commit state (RP2350)
//
// The commit record lives in the last 4 KB flash sector of the 32 KB
// bootloader slot (flash offset 0x7000, XIP 0x10007000). It is written only
// by BareBoot and is never inside the application slot. A transfer that is
// interrupted before COMPLETE leaves the state UPDATE_IN_PROGRESS, so the
// bootrom-side check refuses to jump to a torn application image.
#ifndef __COMMIT_STATE_H
#define __COMMIT_STATE_H

#include <stdint.h> // uint32_t
#include "autoconf.h" // CONFIG_MACH_RP2350, CONFIG_MACH_STM32

#define BAREBOOT_COMMIT_MAGIC 0x42425354 // "BBST"
#define BAREBOOT_STATE_VALID 1
#define BAREBOOT_STATE_UPDATE_IN_PROGRESS 2

#if CONFIG_MACH_RP2350 || CONFIG_MACH_STM32
// Return 1 only when a complete, verified image has been committed.
int commit_state_valid(void);

// Mark the start of a transfer (erase + write UPDATE_IN_PROGRESS record).
void commit_state_begin(void);

// Mark a verified transfer complete (erase + write VALID record).
void commit_state_commit(void);
#else
static inline int commit_state_valid(void) { return 1; }
static inline void commit_state_begin(void) {}
static inline void commit_state_commit(void) {}
#endif

#endif // __COMMIT_STATE_H
