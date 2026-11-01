//! Minimal manual SEGGER RTT block so hosts (probe-rs RTT) can find and read
//! our up-channel without an external rtt crate.
//!
//! IMPORTANT: the control block is declared in assembly (global_asm!) because
//! release optimization grate/dead-store-eliminates `static mut` initializers
//! and the "SEGGER RTT" magic would never land in the image.
//! Layout (SEGGER RTT reference, all words little-endian u32):
//!   id[16]="SEGGER RTT", max_up=1, max_down=1,
//!   up[0]:   name_ptr, buf_ptr, size, wr_off, rd_off, flags   (6 words)
//!   down[0]: 6 zero words
//!   then "status" name string, then the 128-byte ring buffer.
//! rtt_place.ld pins .rtt_place (which contains the whole block) at
//! 0x20004E00 in RAM.

core::arch::global_asm! {
    ".section .rtt_block, \"aw\", %nobits",
    ".p2align 2",
    ".global rtt_control",
    "rtt_control:",
    ".space 16",              // id[16] magic, filled by rtt_init()
    ".space 8",               // max_up / max_down, filled by rtt_init()
    ".space 24",              // up[0] descriptor (name, buf, size, wr, rd, flags)
    ".space 24",              // down[0] descriptor, all zero
    ".global rtt_wrap_name",
    "rtt_wrap_name:",
    ".space 8",
    ".global rtt_wrap_buf",
    "rtt_wrap_buf:",
    ".space 1024",
}

unsafe extern "C" {
    static rtt_control: u32;
}

const NAME_OFF: u32 = 0x48;
const BUF_OFF: u32 = 0x50;

/// Fill the RTT control block at runtime (NOBITS section is zero-init).
pub fn rtt_init() {
    let base = unsafe { core::ptr::addr_of!(rtt_control) } as *mut u32;
    unsafe {
        // magic
        let magic = b"SEGGER RTT\0\0\0\0\0\0";
        let idp = base as *mut u8;
        for (i, b) in magic.iter().enumerate() {
            core::ptr::write_volatile(idp.add(i), *b);
        }
        // counters
        let counters = base.add(4); // 16 bytes / 4 = word index 4
        core::ptr::write_volatile(counters.add(0), 1); // max up buffers
        core::ptr::write_volatile(counters.add(1), 1); // max down buffers
        // up desc at base + 0x18 words
        let up = base.add(UP_DESC_OFF as usize / 4);
        core::ptr::write_volatile(up.add(0), (base as u32) + NAME_OFF);
        core::ptr::write_volatile(up.add(1), (base as u32) + BUF_OFF);
        core::ptr::write_volatile(up.add(2), 1024);
        core::ptr::write_volatile(up.add(3), 0);
        core::ptr::write_volatile(up.add(4), 0);
        core::ptr::write_volatile(up.add(5), 0);
        // name "status"
        let np = ((base as u32) + NAME_OFF) as *mut u8;
        for (i, b) in b"status\0".iter().enumerate() {
            core::ptr::write_volatile(np.add(i), *b);
        }
        cortex_m::asm::dmb();
    }
}

/// Up-channel descriptor starts 0x18 into the control block:
/// id[16] (0x10) + max_up (4) + max_down (4) = 0x18.
/// Descriptor fields: 0 name, 1 buf, 2 size, 3 wr_off, 4 rd_off, 5 flags.
const UP_DESC_OFF: u32 = 0x18;

#[inline]
fn desc(index: u32) -> *mut u32 {
    unsafe {
        (core::ptr::addr_of!(rtt_control) as *mut u32)
            .add((UP_DESC_OFF / 4) as usize)
            .add(index as usize)
    }
}

pub fn rtt_write(bytes: &[u8]) {
    unsafe {
        let size = *desc(2);
        if size == 0 {
            return;
        }
        let buf = *desc(1) as *mut u8;
        if buf.is_null() {
            return;
        }
        let mut wr = *desc(3);
        for b in bytes {
            core::ptr::write_volatile(buf.offset(wr as isize), *b);
            wr = (wr + 1) % size;
        }
        cortex_m::asm::dmb();
        *desc(3) = wr;
    }
}

pub fn rtt_write_line(s: &[u8]) {
    rtt_write(s);
    rtt_write(b"\n");
}

pub struct BufWriter {
    pub buf: [u8; 96],
    pub len: usize,
}

impl core::fmt::Write for BufWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let avail = self.buf.len() - self.len;
        let n = bytes.len().min(avail);
        self.buf[self.len..self.len + n].copy_from_slice(&bytes[..n]);
        self.len += n;
        Ok(())
    }
}

macro_rules! rtt_line {
    ($($arg:tt)*) => {{
        use core::fmt::Write as _;
        let mut w = crate::rtt::BufWriter { buf: [0u8; 96], len: 0 };
        let _ = core::write!(w, $($arg)*);
        let mut line = [0u8; 97];
        line[..w.len].copy_from_slice(&w.buf[..w.len]);
        line[w.len] = 0xA; // '\n'
        crate::rtt::rtt_write(&line[..w.len + 1]);
    }};
}
pub(crate) use rtt_line;
