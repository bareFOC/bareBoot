use heapless::Vec;

pub const MAX_FRAME: usize = 128;
pub const ACK_BUF: usize = 256;


pub struct HostResponse {
    pub data: Vec<u8, ACK_BUF>,
}

impl Default for HostResponse {
    fn default() -> Self { Self { data: Vec::new() } }
}

pub fn enqueue_ack(resp: &mut HostResponse, payload: &[u8]) {
    let _ = resp.data.extend_from_slice(payload);
}
