#[cfg(target_os = "linux")]
use std::fs::OpenOptions;
#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use crate::error::HostError;
#[cfg(target_os = "linux")]
use crate::transport::HostBus;
#[cfg(target_os = "linux")]
const I2C_SLAVE: libc::c_ulong = 0x0703;

#[cfg(target_os = "linux")]
pub struct I2cBus {
    file: std::fs::File,
    timeout: Duration,
}

#[cfg(target_os = "linux")]
impl I2cBus {
    pub fn new(device: &str, address: u8, timeout: Duration) -> Result<Self, HostError> {
        use std::os::unix::fs::OpenOptionsExt;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_RDWR)
            .open(device)
            .map_err(|e| HostError(format!("failed to open {device}: {e}")))?;

        let fd = file.as_raw_fd();
        let result = unsafe { libc::ioctl(fd, I2C_SLAVE as _, address as libc::c_int) };
        if result < 0 {
            return Err(HostError(format!("failed to set I2C slave 0x{address:02x}")));
        }

        let timeval = libc::timeval {
            tv_sec: 0,
            tv_usec: 80_000,
        };
        unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                &timeval as *const libc::timeval as *const libc::c_void,
                std::mem::size_of::<libc::timeval>() as libc::socklen_t,
            );
        }

        Ok(Self { file, timeout })
    }

    pub fn request_bootloader(device: &str, address: u8) -> Result<(), HostError> {
        let mut bus = Self::new(device, address, Duration::from_millis(100))?;
        use std::io::Write;
        let _ = bus.file.write(&[bareboot_protocol::I2C_CMD_ENTER_BOOTLOADER]);
        let _ = bus.file.flush();
        std::thread::sleep(Duration::from_millis(100));
        Ok(())
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

#[cfg(target_os = "linux")]
impl HostBus for I2cBus {
    fn exchange(&mut self, frame: &[u8], timeout: Duration) -> Result<Vec<u8>, HostError> {
        use std::io::{Read, Write};

        self.file
            .write_all(frame)
            .map_err(|e| HostError(format!("I2C write failed: {e}")))?;

        let start = Instant::now();
        let mut rx_buf = Vec::new();
        while start.elapsed() < timeout {
            let mut tmp = [0u8; 256];
            match self.file.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    rx_buf.extend_from_slice(&tmp[..n]);
                    match bareboot_protocol::scan(&rx_buf) {
                        bareboot_protocol::FrameParse::Complete { frame_len, .. } => {
                            return Ok(rx_buf[..frame_len].to_vec());
                        }
                        bareboot_protocol::FrameParse::Invalid { drop } => {
                            if drop > 0 && drop <= rx_buf.len() {
                                rx_buf.drain(..drop);
                            } else {
                                rx_buf.clear();
                            }
                        }
                        bareboot_protocol::FrameParse::NeedMore => {}
                    }
                }
                Err(_) => break,
            }
        }
        Err(HostError("I2C response timeout".to_string()))
    }
}
