use std::time::Duration;

use crate::error::HostError;

pub trait HostBus {
    fn exchange(&mut self, frame: &[u8], timeout: Duration) -> Result<Vec<u8>, HostError>;
}

pub struct InMemoryBus {
    pub responses: std::collections::VecDeque<Vec<u8>>,
    pub sent: Vec<Vec<u8>>,
}

impl InMemoryBus {
    pub fn new() -> Self {
        Self {
            responses: std::collections::VecDeque::new(),
            sent: Vec::new(),
        }
    }

    pub fn push_response(&mut self, response: Vec<u8>) {
        self.responses.push_back(response);
    }
}

impl HostBus for InMemoryBus {
    fn exchange(&mut self, frame: &[u8], _timeout: Duration) -> Result<Vec<u8>, HostError> {
        self.sent.push(frame.to_vec());
        self.responses
            .pop_front()
            .ok_or_else(|| HostError("no queued response".to_string()))
    }
}

impl HostBus for Box<dyn HostBus> {
    fn exchange(&mut self, frame: &[u8], timeout: std::time::Duration) -> Result<Vec<u8>, crate::error::HostError> {
        (**self).exchange(frame, timeout)
    }
}
