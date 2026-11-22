#[cfg(unix)]
use std::os::fd::RawFd;

#[cfg(not(unix))]
pub type RawFd = i32;

use std::time::Duration;
use crate::error::HostError;
use crate::transport::HostBus;

pub struct AndroidUsbBus {
    pub fd: RawFd,
    pub ep_in: u8,
    pub ep_out: u8,
    pub timeout: Duration,
}

impl AndroidUsbBus {
    pub fn new(fd: RawFd, ep_in: u8, ep_out: u8) -> Self {
        Self {
            fd,
            ep_in,
            ep_out,
            timeout: Duration::from_secs(2),
        }
    }
}

impl HostBus for AndroidUsbBus {
    fn exchange(&mut self, frame: &[u8], _timeout: Duration) -> Result<Vec<u8>, HostError> {
        #[cfg(any(target_os = "android", target_os = "linux"))]
        {
            let written = unsafe {
                libc::write(self.fd, frame.as_ptr() as *const libc::c_void, frame.len())
            };
            if written < 0 {
                return Err(HostError(format!("USB bulk write failed: {}", std::io::Error::last_os_error())));
            }

            let mut buf = vec![0u8; 1024];
            let n = unsafe {
                libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
            };
            if n < 0 {
                return Err(HostError(format!("USB bulk read failed: {}", std::io::Error::last_os_error())));
            }
            buf.truncate(n as usize);
            Ok(buf)
        }
        #[cfg(not(any(target_os = "android", target_os = "linux")))]
        {
            let _ = frame;
            Err(HostError("AndroidUsbBus only supported on Android and Linux".to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_android_usb_bus_struct() {
        let bus = AndroidUsbBus::new(1, 0x81, 0x02);
        assert_eq!(bus.fd, 1);
        assert_eq!(bus.ep_in, 0x81);
        assert_eq!(bus.ep_out, 0x02);
    }
}
