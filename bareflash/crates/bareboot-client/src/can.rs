#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use crate::error::HostError;
#[cfg(target_os = "linux")]
use crate::transport::HostBus;

#[cfg(target_os = "linux")]
pub struct SocketCanBus {
    fd: std::os::unix::io::RawFd,
    node_id: u32,
}

#[cfg(target_os = "linux")]
impl SocketCanBus {
    pub fn new(ifname: &str, node_id: u32) -> Result<Self, HostError> {
        use std::ffi::CString;

        let ifindex = unsafe {
            let name = CString::new(ifname).map_err(|e| HostError(e.to_string()))?;
            libc::if_nametoindex(name.as_ptr())
        };
        if ifindex == 0 {
            return Err(HostError(format!("unknown CAN interface: {ifname}")));
        }

        let fd = unsafe { libc::socket(libc::PF_CAN, libc::SOCK_RAW, libc::CAN_RAW) };
        if fd < 0 {
            return Err(HostError("failed to create CAN socket".to_string()));
        }

        let mut addr: libc::sockaddr_can = unsafe { std::mem::zeroed() };
        addr.can_family = libc::AF_CAN as u16;
        addr.can_ifindex = ifindex as i32;

        let bind_result = unsafe {
            libc::bind(
                fd,
                &addr as *const libc::sockaddr_can as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_can>() as libc::socklen_t,
            )
        };
        if bind_result < 0 {
            unsafe { libc::close(fd); }
            return Err(HostError("failed to bind CAN socket".to_string()));
        }

        // A short receive timeout keeps the exchange loop responsive.
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

        Ok(Self { fd, node_id })
    }
}

#[cfg(target_os = "linux")]
impl Drop for SocketCanBus {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd); }
    }
}

#[cfg(target_os = "linux")]
impl HostBus for SocketCanBus {
    fn exchange(&mut self, frame: &[u8], timeout: Duration) -> Result<Vec<u8>, HostError> {
        // Send the frame in 8-byte CAN packets.
        for chunk in frame.chunks(8) {
            let mut cf: libc::can_frame = unsafe { std::mem::zeroed() };
            cf.can_id = self.node_id;
            cf.can_dlc = chunk.len() as u8;
            cf.data[..chunk.len()].copy_from_slice(chunk);
            let written = unsafe {
                libc::write(
                    self.fd,
                    &cf as *const libc::can_frame as *const libc::c_void,
                    std::mem::size_of::<libc::can_frame>(),
                )
            };
            if written < 0 {
                return Err(HostError("CAN write failed".to_string()));
            }
        }

        let start = Instant::now();
        let mut rx_buf: Vec<u8> = Vec::new();
        while start.elapsed() < timeout {
            let mut cf: libc::can_frame = unsafe { std::mem::zeroed() };
            let read = unsafe {
                libc::read(
                    self.fd,
                    &mut cf as *mut libc::can_frame as *mut libc::c_void,
                    std::mem::size_of::<libc::can_frame>(),
                )
            };
            if read < 0 {
                continue; // timeout or EAGAIN
            }
            if read == 0 {
                continue;
            }
            if cf.can_id != self.node_id + 1 {
                continue;
            }
            let dlc = (cf.can_dlc as usize).min(8);
            rx_buf.extend_from_slice(&cf.data[..dlc]);

            match bareboot_protocol::scan(&rx_buf) {
                bareboot_protocol::FrameParse::Complete { frame_len, .. } => {
                    let frame = rx_buf[..frame_len].to_vec();
                    return Ok(frame);
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
        Err(HostError("CAN response timeout".to_string()))
    }
}
