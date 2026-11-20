// STM32 I3C target with dynamic I2C fallback driver for Katapult
//
// Automatically detects standard I2C flasher (address 0x30) or
// I3C Controller broadcast (address 0x7E) on PB6/PB7 or PB3/PB4 and switches dynamically.
//
// This file may be distributed under the terms of the GNU GPLv3 license.

#include <string.h> // memmove, memcpy
#include "autoconf.h" // CONFIG_I2C_TARGET_ADDRESS, CONFIG_I3C_STATIC_ADDRESS, CONFIG_CLOCK_FREQ
#include "board/armcm_boot.h" // armcm_enable_irq, armcm_disable_irq
#include "board/io.h" // readb, writeb
#include "board/irq.h" // irq_save, irq_restore
#include "board/misc.h" // console_sendf
#include "board/pgm.h" // READP
#include "bootentry.h" // bootentry_cancel_timeout
#include "canboot.h" // set_bootup_code, REQUEST_CANBOOT
#include "command.h" // DECL_CONSTANT, command_find_block, command_dispatch, etc.
#include "internal.h" // enable_pclock, gpio_peripheral, GPIO, GPIO_FUNCTION, GPIO_OPEN_DRAIN
#include "sched.h" // DECL_INIT, DECL_TASK, sched_wake_tasks

#define RX_BUFFER_SIZE 192
#define TX_BUFFER_SIZE 128

#if CONFIG_STM32_I3C1_DYNAMIC_PB3_PB4
DECL_CONSTANT_STR("RESERVE_PINS_i2c_i3c", "PB3,PB4");
#define GPIO_SCL GPIO('B', 3)
#define GPIO_SDA GPIO('B', 4)
#define I2C_PERIPH I2C2
#define I2C_EV_IRQn I2C2_EV_IRQn
#define I2C_ER_IRQn I2C2_ER_IRQn
#define I2C_AF (GPIO_FUNCTION(8) | GPIO_OPEN_DRAIN)
#else
DECL_CONSTANT_STR("RESERVE_PINS_i2c_i3c", "PB6,PB7");
#define GPIO_SCL GPIO('B', 6)
#define GPIO_SDA GPIO('B', 7)
#define I2C_PERIPH I2C1
#define I2C_EV_IRQn I2C1_EV_IRQn
#define I2C_ER_IRQn I2C1_ER_IRQn
#define I2C_AF (GPIO_FUNCTION(4) | GPIO_OPEN_DRAIN)
#endif
DECL_CONSTANT("I2C_TARGET_ADDRESS", CONFIG_I2C_TARGET_ADDRESS);
DECL_CONSTANT("I3C_STATIC_ADDRESS", CONFIG_I3C_STATIC_ADDRESS);
DECL_CONSTANT("RECEIVE_WINDOW", RX_BUFFER_SIZE);
#define I3C_AF (GPIO_FUNCTION(3) | GPIO_OPEN_DRAIN)

#define MODE_I2C 0
#define MODE_I3C 1

#define CCC_DIRECT_ENEC    0x00
#define CCC_DIRECT_DISEC   0x01
#define CCC_DIRECT_RSTDAA  0x06
#define CCC_BCAST_ENTDAA   0x07
#define CCC_DIRECT_SETDASA 0x87
#define CCC_BCAST_RSTACT   0x2A
#define CCC_DIRECT_RSTACT  0x9A
// The MIPI direct form of RSTACT is (0x2A | 0x80); accept it as well.
#define CCC_DIRECT_RSTACT_ALT 0xAA
#define RSTACT_DEF_TGT_RESET 0x02 // defining byte: Target System Reset

static volatile uint8_t active_mode = MODE_I2C;
static uint8_t receive_buf[RX_BUFFER_SIZE];
static uint8_t receive_pos;
static uint8_t transmit_buf[TX_BUFFER_SIZE];
static uint8_t transmit_pos;
static uint8_t transmit_max;
// One-shot RSTACT detector: RMR holds the CCC code, its defining byte
// arrives in the RX FIFO and is consumed in I3C2_EV_IRQHandler().
static volatile uint8_t rstact_pending;
static uint8_t last_ccc_code;

#define FLASH_STORAGE_MAGIC 0x46495348 // "FISH"
#define FLASH_SECTOR_14_ADDR 0x0801C000
#define FLASH_SECTOR_15_ADDR 0x0801E000
#define FLASH_HEADER_SIZE 32
#define FLASH_REG_NODE_ID_OFFSET 0x10

struct flash_sector_header {
    uint32_t magic;
    uint16_t version;
    uint16_t payload_len;
    uint32_t seq;
    uint32_t crc32;
    uint32_t reserved[4];
};

static uint8_t
read_configured_node_id(void)
{
    const struct flash_sector_header *h14 =
        (const struct flash_sector_header *)FLASH_SECTOR_14_ADDR;
    const struct flash_sector_header *h15 =
        (const struct flash_sector_header *)FLASH_SECTOR_15_ADDR;

    int v14 = (h14->magic == FLASH_STORAGE_MAGIC && h14->version == 1
               && h14->payload_len == 96);
    int v15 = (h15->magic == FLASH_STORAGE_MAGIC && h15->version == 1
               && h15->payload_len == 96);

    const struct flash_sector_header *best = NULL;
    if (v14 && v15) {
        best = (h14->seq >= h15->seq) ? h14 : h15;
    } else if (v14) {
        best = h14;
    } else if (v15) {
        best = h15;
    }

    if (best) {
        const uint8_t *payload = ((const uint8_t *)best) + FLASH_HEADER_SIZE;
        uint8_t stored_id = payload[FLASH_REG_NODE_ID_OFFSET];
        if (stored_id >= 0x08 && stored_id <= 0x77) {
            return stored_id;
        }
    }

#ifdef CONFIG_I2C_TARGET_ADDRESS
    return CONFIG_I2C_TARGET_ADDRESS;
#else
    return 0x40;
#endif
}

static void switch_to_i3c(void);

// ---------------------------------------------------------
// I2C1 Interrupt Handlers
// ---------------------------------------------------------
void
I2C1_EV_IRQHandler(void)
{
    uint32_t isr = I2C_PERIPH->ISR;

    // Address matched interrupt
    if (isr & I2C_ISR_ADDR) {
        uint8_t addcode = (isr & I2C_ISR_ADDCODE_Msk) >> 17;
        if (addcode == 0x7E) {
            // I3C Controller broadcast (0x7E) detected: switch pins to I3C2!
            I2C_PERIPH->ICR = I2C_ICR_ADDRCF;
            switch_to_i3c();
            return;
        }

        // Clear standard address match flag
        I2C_PERIPH->ICR = I2C_ICR_ADDRCF;

        if (isr & I2C_ISR_DIR) {
            // Master read (slave transmit)
            uint8_t tpos = readb(&transmit_pos);
            uint8_t tmax = readb(&transmit_max);
            if (tpos < tmax) {
                I2C_PERIPH->TXDR = transmit_buf[tpos];
                writeb(&transmit_pos, tpos + 1);
                I2C_PERIPH->CR1 |= I2C_CR1_TXIE;
            } else {
                I2C_PERIPH->TXDR = 0xFF;
                I2C_PERIPH->CR1 &= ~I2C_CR1_TXIE;
            }
        } else {
            // Master write (slave receive)
            I2C_PERIPH->CR1 &= ~I2C_CR1_TXIE;
            I2C_PERIPH->ISR = I2C_ISR_TXE;
        }
    }

    // Byte received from master
    if (isr & I2C_ISR_RXNE) {
        uint8_t data = I2C_PERIPH->RXDR;
        uint8_t rpos = readb(&receive_pos);
        if (rpos < sizeof(receive_buf)) {
            receive_buf[rpos] = data;
            writeb(&receive_pos, rpos + 1);
        }
    }

    // Transmit data register empty
    if ((isr & I2C_ISR_TXIS) && (I2C_PERIPH->CR1 & I2C_CR1_TXIE)) {
        uint8_t tpos = readb(&transmit_pos);
        uint8_t tmax = readb(&transmit_max);
        if (tpos < tmax) {
            I2C_PERIPH->TXDR = transmit_buf[tpos];
            writeb(&transmit_pos, tpos + 1);
        } else {
            I2C_PERIPH->TXDR = 0xFF;
            I2C_PERIPH->CR1 &= ~I2C_CR1_TXIE;
        }
    }

    // Master NACK received
    if (isr & I2C_ISR_NACKF) {
        I2C_PERIPH->ICR = I2C_ICR_NACKCF;
        I2C_PERIPH->CR1 &= ~I2C_CR1_TXIE;
        I2C_PERIPH->ISR = I2C_ISR_TXE;
    }

    // STOP condition detected
    if (isr & I2C_ISR_STOPF) {
        I2C_PERIPH->ICR = I2C_ICR_STOPCF;
        I2C_PERIPH->CR1 &= ~I2C_CR1_TXIE;
        I2C_PERIPH->ISR = I2C_ISR_TXE;
        sched_wake_tasks();
    }
}

void
I2C1_ER_IRQHandler(void)
{
    uint32_t isr = I2C_PERIPH->ISR;
    if (isr & I2C_ISR_BERR)
        I2C_PERIPH->ICR = I2C_ICR_BERRCF;
    if (isr & I2C_ISR_ARLO)
        I2C_PERIPH->ICR = I2C_ICR_ARLOCF;
    if (isr & I2C_ISR_OVR)
        I2C_PERIPH->ICR = I2C_ICR_OVRCF;
}

// ---------------------------------------------------------
// I3C2 Interrupt Handlers
// ---------------------------------------------------------
void
I3C2_EV_IRQHandler(void)
{
    uint32_t evr = I3C2->EVR;

    // Dynamic Address Update
    if (evr & I3C_EVR_DAUPDF) {
        I3C2->CEVR = I3C_CEVR_CDAUPDF;
    }

    // CCC Command handler (RSTDAA and RSTACT)
    uint32_t rmr = I3C2->RMR;
    uint8_t ccc_code = (rmr & I3C_RMR_RCODE_Msk) >> I3C_RMR_RCODE_Pos;
    if (ccc_code == CCC_DIRECT_RSTDAA) {
        I3C2->DEVR0 &= ~I3C_DEVR0_DAVAL;
    }
    // RSTACT carries a defining byte in the RX FIFO.  Arm a one-shot
    // detector only when RMR first shows the RSTACT code, so a stale RMR
    // value on later, unrelated traffic cannot spuriously reset the board.
    if (ccc_code != last_ccc_code) {
        last_ccc_code = ccc_code;
        if (ccc_code == CCC_BCAST_RSTACT || ccc_code == CCC_DIRECT_RSTACT
            || ccc_code == CCC_DIRECT_RSTACT_ALT)
            rstact_pending = 1;
        else
            rstact_pending = 0;
    }

    // Receive data from I3C master
    while (I3C2->EVR & I3C_EVR_RXFNEF) {
        uint8_t data = I3C2->RDR & 0xFF;
        if (rstact_pending) {
            // First byte after RSTACT is its defining byte; do not feed it
            // to the command stream.
            rstact_pending = 0;
            if (data == RSTACT_DEF_TGT_RESET) {
                // Keep the bootloader resident across the system reset:
                // the mailbox at the initial SP is checked after reboot.
                set_bootup_code(REQUEST_CANBOOT);
                irq_disable();
                NVIC_SystemReset();
            }
            continue;
        }
        uint8_t rpos = readb(&receive_pos);
        if (rpos < sizeof(receive_buf)) {
            receive_buf[rpos] = data;
            writeb(&receive_pos, rpos + 1);
        }
    }

    // Transmit data to I3C master
    if ((I3C2->EVR & I3C_EVR_TXFNFF) && (I3C2->IER & I3C_IER_TXFNFIE)) {
        uint8_t tpos = readb(&transmit_pos);
        uint8_t tmax = readb(&transmit_max);
        if (tpos < tmax) {
            I3C2->TDR = transmit_buf[tpos];
            writeb(&transmit_pos, tpos + 1);
        } else {
            I3C2->IER &= ~I3C_IER_TXFNFIE;
        }
    }

    // Frame Complete or Reception Target End
    if (evr & (I3C_EVR_FCF | I3C_EVR_RXTGTENDF)) {
        I3C2->CEVR = I3C_CEVR_CFCF | I3C_CEVR_CRXTGTENDF;
        sched_wake_tasks();
    }
}

void
I3C2_ER_IRQHandler(void)
{
    uint32_t evr = I3C2->EVR;
    if (evr & I3C_EVR_ERRF) {
        I3C2->CEVR = I3C_CEVR_CERRF;
    }
}

// ---------------------------------------------------------
// Hardware Switching to I3C
// ---------------------------------------------------------
static void
switch_to_i3c(void)
{
    active_mode = MODE_I3C;

    // Disable I2C1
    I2C_PERIPH->CR1 = 0;
    NVIC_DisableIRQ(I2C_EV_IRQn);
    NVIC_DisableIRQ(I2C_ER_IRQn);

    // Switch PB6 and PB7 to AF3 (I3C2)
    gpio_peripheral(GPIO_SCL, I3C_AF, 1);
    gpio_peripheral(GPIO_SDA, I3C_AF, 1);

    // Ensure I3C2 peripheral clock is enabled
    RCC->APB3ENR |= RCC_APB3ENR_I3C2EN;
    (void)RCC->APB3ENR;

    // Initialize I3C2 peripheral as Target (PB3/PB4 AF3 maps to I3C2 on H503)
    uint8_t target_addr = read_configured_node_id();
    I3C2->CFGR = 0;
    I3C2->CFGR = I3C_CFGR_RSTPTRN | I3C_CFGR_EXITPTRN;
    I3C2->BCR = 0x06;
    I3C2->DCR = 0x63;
    I3C2->EPIDR = (0x0114 << 16) | 0x0001;
    I3C2->DEVR0 = (target_addr & 0x7F) << 1;
    I3C2->MAXRLR = MESSAGE_MAX;
    I3C2->MAXWLR = MESSAGE_MAX;
    I3C2->TIMINGR0 = (24 << I3C_TIMINGR0_SCLH_I2C_Pos) |
                     (24 << I3C_TIMINGR0_SCLL_OD_Pos) |
                     (3 << I3C_TIMINGR0_SCLH_I3C_Pos) |
                     (3 << I3C_TIMINGR0_SCLL_PP_Pos);
    I3C2->CEVR = 0xFFFFFFFF;
    I3C2->IER = I3C_IER_RXFNEIE | I3C_IER_FCIE | I3C_IER_RXTGTENDIE |
                I3C_IER_DAUPDIE | I3C_IER_ERRIE;

    armcm_enable_irq(I3C2_EV_IRQHandler, I3C2_EV_IRQn, 0);
    armcm_enable_irq(I3C2_ER_IRQHandler, I3C2_ER_IRQn, 0);

    I3C2->CFGR |= I3C_CFGR_EN;
}

// ---------------------------------------------------------
// Buffer & Task Management
// ---------------------------------------------------------
static void
console_pop_input(uint_fast8_t len)
{
    uint_fast8_t copied = 0;
    for (;;) {
        uint_fast8_t rpos = readb(&receive_pos);
        uint_fast8_t needcopy = rpos - len;
        if (needcopy) {
            memmove(&receive_buf[copied], &receive_buf[copied + len]
                    , needcopy - copied);
            copied = needcopy;
            sched_wake_tasks();
        }
        irqstatus_t flag = irq_save();
        if (rpos != readb(&receive_pos)) {
            irq_restore(flag);
            continue;
        }
        receive_pos = needcopy;
        irq_restore(flag);
        break;
    }
}

void
console_task(void)
{
    uint_fast8_t rpos = readb(&receive_pos), pop_count;
    int_fast8_t ret = command_find_block(receive_buf, rpos, &pop_count);
    if (ret > 0) {
        bootentry_cancel_timeout();
        command_dispatch(receive_buf, pop_count);
    }
    if (ret) {
        console_pop_input(pop_count);
        if (ret > 0)
            command_send_ack();
    }
}
DECL_TASK(console_task);

void
console_sendf(const struct command_encoder *ce, va_list args)
{
    uint_fast8_t tpos = readb(&transmit_pos), tmax = readb(&transmit_max);
    if (tpos >= tmax) {
        tpos = tmax = 0;
        writeb(&transmit_max, 0);
        writeb(&transmit_pos, 0);
    }
    uint_fast8_t max_size = READP(ce->max_size);
    if (tmax + max_size > sizeof(transmit_buf)) {
        if (tmax + max_size - tpos > sizeof(transmit_buf))
            return;
        if (active_mode == MODE_I2C)
            I2C_PERIPH->CR1 &= ~I2C_CR1_TXIE;
        else
            I3C2->IER &= ~I3C_IER_TXFNFIE;
        tpos = readb(&transmit_pos);
        tmax -= tpos;
        memmove(&transmit_buf[0], &transmit_buf[tpos], tmax);
        writeb(&transmit_pos, 0);
        writeb(&transmit_max, tmax);
    }

    uint8_t *buf = &transmit_buf[tmax];
    uint_fast8_t msglen = command_encode_and_frame(buf, ce, args);
    writeb(&transmit_max, tmax + msglen);

    if (active_mode == MODE_I2C) {
        irqstatus_t flag = irq_save();
        if ((I2C_PERIPH->ISR & I2C_ISR_DIR) && (I2C_PERIPH->ISR & I2C_ISR_TXIS)) {
            I2C_PERIPH->CR1 |= I2C_CR1_TXIE;
        }
        irq_restore(flag);
    } else {
        I3C2->TGTTDR = msglen;
        irqstatus_t flag = irq_save();
        I3C2->IER |= I3C_IER_TXFNFIE;
        irq_restore(flag);
    }
}

// ---------------------------------------------------------
// Initializer
// ---------------------------------------------------------
void
i3c_dynamic_target_init(void)
{
    enable_pclock((uint32_t)I2C_PERIPH);
    enable_pclock((uint32_t)I3C2);
    RCC->APB3ENR |= RCC_APB3ENR_I3C2EN;
    (void)RCC->APB3ENR;

    // Start in I2C mode
    active_mode = MODE_I2C;

    I2C_PERIPH->CR1 = 0;
    I2C_PERIPH->TIMINGR = 0x10420F13; // 400 kHz fast-mode

    // OAR1: Target Address (e.g. 0x40 or configured from flash storage)
    uint8_t target_addr = read_configured_node_id();
    I2C_PERIPH->OAR1 = I2C_OAR1_OA1EN | ((target_addr & 0x7F) << 1);

    // OAR2: I3C Broadcast Address (0x7E)
    I2C_PERIPH->OAR2 = I2C_OAR2_OA2EN | (0x7E << 1);

    I2C_PERIPH->ICR = 0xFFFFFFFF;
    I2C_PERIPH->CR1 = I2C_CR1_PE | I2C_CR1_RXIE | I2C_CR1_ADDRIE |
                I2C_CR1_STOPIE | I2C_CR1_NACKIE | I2C_CR1_ERRIE |
                I2C_CR1_NOSTRETCH;

    armcm_enable_irq(I2C1_EV_IRQHandler, I2C_EV_IRQn, 0);
    armcm_enable_irq(I2C1_ER_IRQHandler, I2C_ER_IRQn, 0);

    // Configure pins for I2C target mode initially
    gpio_peripheral(GPIO_SCL, I2C_AF, 1);
    gpio_peripheral(GPIO_SDA, I2C_AF, 1);
}
DECL_INIT(i3c_dynamic_target_init);
