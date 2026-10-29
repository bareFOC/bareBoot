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
pub struct SerialBus {
    file: std::fs::File,
}

#[cfg(target_os = "linux")]
impl SerialBus {
    pub fn write_raw(&mut self, data: &[u8]) -> Result<(), HostError> {
        use std::io::Write;
        self.file.write_all(data).map_err(|e| HostError(e.to_string()))?;
        self.file.flush().map_err(|e| HostError(e.to_string()))?;
        Ok(())
    }

    pub fn new(device: &str, baud: u32) -> Result<Self, HostError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(device)
            .map_err(|e| HostError(format!("failed to open {device}: {e}")))?;

        let fd = file.as_raw_fd();
        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(fd, &mut termios) != 0 {
                return Err(HostError("tcgetattr failed".to_string()));
            }
            libc::cfmakeraw(&mut termios);

            let speed = match baud {
                9600 => libc::B9600,
                19200 => libc::B19200,
                38400 => libc::B38400,
                57600 => libc::B57600,
                115200 => libc::B115200,
                _ => libc::B115200,
            };
            libc::cfsetispeed(&mut termios, speed);
            libc::cfsetospeed(&mut termios, speed);

            // Return after one decisecond when no bytes are available.
            termios.c_cc[libc::VMIN as usize] = 0;
            termios.c_cc[libc::VTIME as usize] = 1;

            if libc::tcsetattr(fd, libc::TCSANOW, &termios) != 0 {
                return Err(HostError("tcsetattr failed".to_string()));
            }
        }

        unsafe { libc::tcflush(file.as_raw_fd(), libc::TCIFLUSH); }
        Ok(Self { file })
    }
}

#[cfg(target_os = "linux")]
impl HostBus for SerialBus {
    fn exchange(&mut self, frame: &[u8], timeout: Duration) -> Result<Vec<u8>, HostError> {
        use std::io::{Read, Write};

        for &byte in frame {
            let mut one = [byte];
            self.file
                .write_all(&one)
                .map_err(|e| HostError(format!("serial write failed: {e}")))?;
            self.file
                .flush()
                .map_err(|e| HostError(format!("serial flush failed: {e}")))?;
            std::thread::sleep(std::time::Duration::from_micros(5000));
        }

        let start = std::time::Instant::now();
        let mut rx_buf = Vec::new();
        while start.elapsed() < timeout {
            let mut tmp = [0u8; 256];
            match self.file.read(&mut tmp) {
                Ok(0) => {
                    if rx_buf.is_empty() {
                        continue;
                    }
                    break;
                }
                Ok(n) => {

                    rx_buf.extend_from_slice(&tmp[..n]);
                    let scanres = bareboot_protocol::scan(&rx_buf);

                    match scanres {
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
                Err(e) => {

                    if !rx_buf.is_empty() {
                        break;
                    }
                }
            }
        }
        Err(HostError("serial response timeout".to_string()))
    }
}
