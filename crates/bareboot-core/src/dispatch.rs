use bareboot_protocol::{
    self, FrameParse, ProtocolError, commands, responses, read_u32_le, write_u32_le,
    build_connect_payload,
};
use crate::CoreError;
use crate::writer::PagedWriter;

const MAX_FRAME: usize = bareboot_protocol::MAX_FRAME_BYTES;

pub fn dispatch_one<const PAGE: usize, F>(
    writer: &mut PagedWriter<F, PAGE>,
    input: &[u8],
    output: &mut [u8],
) -> Result<usize, CoreError>
where
    F: crate::FlashTarget,
{
    let parsed = bareboot_protocol::scan(input);
    let (command, word_count, payload_offset) = match parsed {
        FrameParse::Complete {
            command,
            word_count,
            payload_offset,
            ..
        } => (command, word_count, payload_offset),
        FrameParse::NeedMore => return Ok(0),
        FrameParse::Invalid { .. } => return Err(CoreError::InvalidAddress),
    };

    let payload_len = word_count * 4;
    if payload_offset + payload_len > input.len() || output.len() < MAX_FRAME {
        return Err(CoreError::BufferTooSmall);
    }
    let payload = &input[payload_offset..payload_offset + payload_len];

    match command {
        commands::CONNECT => {
            writer.reset_session();
            let mut ack_payload = [0u8; MAX_FRAME];
            let cfg = writer.config();
            let len = build_connect_payload(
                commands::CONNECT,
                cfg.protocol_version,
                cfg.app_base,
                cfg.block_size as u32,
                cfg.mcu_type,
                cfg.software_version,
                &mut ack_payload,
            )
            .map_err(|_| CoreError::BufferTooSmall)?;
            encode_ack(responses::ACK, &ack_payload[..len], output)
        }
        commands::RX_BLOCK => {
            if payload.len() < 4 {
                return encode_command_error(output);
            }
            let addr = read_u32_le(payload, 0);
            let data = &payload[4..];
            let block_size = writer.config().block_size;
            if data.len() != block_size {
                return encode_command_error(output);
            }
            match writer.write_block(addr, data) {
                Ok(()) => {
                    let mut ack_payload = [0u8; 8];
                    write_u32_le(&mut ack_payload, 0, commands::RX_BLOCK as u32);
                    write_u32_le(&mut ack_payload, 4, addr);
                    encode_ack(responses::ACK, &ack_payload, output)
                }
                Err(_) => encode_command_error(output),
            }
        }
        commands::RX_EOF => {
            match writer.finish() {
                Ok(pages) => {
                    let mut ack_payload = [0u8; 8];
                    write_u32_le(&mut ack_payload, 0, commands::RX_EOF as u32);
                    write_u32_le(&mut ack_payload, 4, pages);
                    encode_ack(responses::ACK, &ack_payload, output)
                }
                Err(_) => encode_command_error(output),
            }
        }
        commands::REQ_BLOCK => {
            if payload.len() < 4 {
                return encode_command_error(output);
            }
            let addr = read_u32_le(payload, 0);
            let block_size = writer.config().block_size;
            let mut data = [0u8; 128];
            if block_size > data.len() {
                return encode_command_error(output);
            }
            match writer.read_bytes(addr, &mut data[..block_size]) {
                Ok(()) => {
                    let mut ack_payload = [0u8; 8 + 128];
                    write_u32_le(&mut ack_payload, 0, commands::REQ_BLOCK as u32);
                    write_u32_le(&mut ack_payload, 4, addr);
                    ack_payload[8..8 + block_size].copy_from_slice(&data[..block_size]);
                    bareboot_protocol::encode_frame(responses::ACK, &ack_payload[..8 + block_size], output)
                        .map_err(|_| CoreError::BufferTooSmall)
                }
                Err(_) => encode_command_error(output),
            }
        }
        commands::COMPLETE => {
            if writer.finish().is_err() || writer.commit_valid().is_err() {
                return encode_command_error(output);
            }
            writer.set_completed();
            let mut ack_payload = [0u8; 4];
            write_u32_le(&mut ack_payload, 0, commands::COMPLETE as u32);
            encode_ack(responses::ACK, &ack_payload, output)
        }
        commands::GET_CANBUS_ID => {
            let uid = writer.config().uid;
            let mut ack_payload = [0u8; 12];
            write_u32_le(&mut ack_payload, 0, commands::GET_CANBUS_ID as u32);
            ack_payload[4..10].copy_from_slice(&uid[..6]);
            encode_ack(responses::ACK, &ack_payload, output)
        }
        _ => encode_command_error(output),
    }
}

fn encode_ack(code: u8, payload: &[u8], output: &mut [u8]) -> Result<usize, CoreError> {
    bareboot_protocol::encode_frame(code, payload, output)
        .map_err(|e| match e {
            ProtocolError::BufferTooSmall => CoreError::BufferTooSmall,
            _ => CoreError::Flash,
        })
}

fn encode_command_error(output: &mut [u8]) -> Result<usize, CoreError> {
    bareboot_protocol::encode_frame(responses::COMMAND_ERROR, &[], output)
        .map_err(|_| CoreError::BufferTooSmall)
}

use crate::TransportKind;
use bareboot_protocol::SoftResetMailbox;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArbitrationState {
    Arbitrating,
    Locked(TransportKind),
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArbitrationAction {
    Listen(TransportKind),
    SwitchInterface { from: TransportKind, to: TransportKind },
    StaySelected(TransportKind),
    TimeoutExpired,
}

/// Multi-interface transport arbiter supporting concurrent polling,
/// shared-pin time-slicing, and directed soft-reset mailbox jumps.
#[derive(Debug, Clone)]
pub struct MultiTransportArbiter {
    pub boot_window_ms: u32,
    pub elapsed_ms: u32,
    pub state: ArbitrationState,
    pub time_slice_ms: u32,
    pub slice_elapsed_ms: u32,
    pub shared_transports: [Option<TransportKind>; 4],
    pub shared_count: usize,
    pub active_shared_idx: usize,
    pub is_infinite_window: bool,
}

impl MultiTransportArbiter {
    /// Create a new arbiter with the given boot window timeout in milliseconds.
    pub fn new(boot_window_ms: u32) -> Self {
        Self {
            boot_window_ms,
            elapsed_ms: 0,
            state: ArbitrationState::Arbitrating,
            time_slice_ms: 50,
            slice_elapsed_ms: 0,
            shared_transports: [None; 4],
            shared_count: 0,
            active_shared_idx: 0,
            is_infinite_window: false,
        }
    }

    /// Construct arbiter from a RAM mailbox hint.
    /// If valid, skips arbitration, locks to the requested transport, and sets an infinite boot window.
    pub fn from_mailbox(mailbox: &SoftResetMailbox) -> Option<Self> {
        if !mailbox.is_valid() {
            return None;
        }

        let transport = match mailbox.bus_type {
            SoftResetMailbox::BUS_TYPE_I2C => TransportKind::I2c,
            SoftResetMailbox::BUS_TYPE_I3C => TransportKind::I2c,
            SoftResetMailbox::BUS_TYPE_CAN => TransportKind::Can,
            SoftResetMailbox::BUS_TYPE_UART => TransportKind::Uart,
            SoftResetMailbox::BUS_TYPE_USB => TransportKind::Usb,
            _ => return None,
        };

        Some(Self {
            boot_window_ms: u32::MAX,
            elapsed_ms: 0,
            state: ArbitrationState::Locked(transport),
            time_slice_ms: 0,
            slice_elapsed_ms: 0,
            shared_transports: [Some(transport), None, None, None],
            shared_count: 1,
            active_shared_idx: 0,
            is_infinite_window: true,
        })
    }

    /// Configure shared-pin time-slicing across the provided transports.
    pub fn with_shared_transports(mut self, time_slice_ms: u32, transports: &[TransportKind]) -> Self {
        self.time_slice_ms = time_slice_ms;
        self.shared_count = transports.len().min(4);
        for i in 0..self.shared_count {
            self.shared_transports[i] = Some(transports[i]);
        }
        self.active_shared_idx = 0;
        self.slice_elapsed_ms = 0;
        self
    }

    /// The currently active or locked transport.
    pub fn active_transport(&self) -> Option<TransportKind> {
        match self.state {
            ArbitrationState::Locked(kind) => Some(kind),
            ArbitrationState::Arbitrating => {
                if self.shared_count > 0 {
                    self.shared_transports[self.active_shared_idx]
                } else {
                    None
                }
            }
            ArbitrationState::Expired => None,
        }
    }

    /// Whether arbitration has locked onto a selected transport.
    pub fn is_locked(&self) -> bool {
        matches!(self.state, ArbitrationState::Locked(_))
    }

    /// Step arbitration forward by dt_ms milliseconds.
    pub fn tick(&mut self, dt_ms: u32) -> ArbitrationAction {
        if let ArbitrationState::Locked(kind) = self.state {
            return ArbitrationAction::StaySelected(kind);
        }
        if self.state == ArbitrationState::Expired {
            return ArbitrationAction::TimeoutExpired;
        }

        if !self.is_infinite_window {
            self.elapsed_ms = self.elapsed_ms.saturating_add(dt_ms);
            if self.elapsed_ms >= self.boot_window_ms {
                self.state = ArbitrationState::Expired;
                return ArbitrationAction::TimeoutExpired;
            }
        }

        if self.shared_count > 1 {
            self.slice_elapsed_ms = self.slice_elapsed_ms.saturating_add(dt_ms);
            if self.slice_elapsed_ms >= self.time_slice_ms {
                self.slice_elapsed_ms = 0;
                let old = self.shared_transports[self.active_shared_idx].unwrap();
                self.active_shared_idx = (self.active_shared_idx + 1) % self.shared_count;
                let new = self.shared_transports[self.active_shared_idx].unwrap();
                return ArbitrationAction::SwitchInterface { from: old, to: new };
            }
        }

        let current = self.active_transport().unwrap_or(TransportKind::Can);
        ArbitrationAction::Listen(current)
    }

    /// Process incoming frame on a given transport. If a valid CONNECT command is detected
    /// during arbitration, locks to that transport. Returns true if frame should be processed.
    pub fn handle_incoming_frame(&mut self, transport: TransportKind, frame: &[u8]) -> bool {
        match self.state {
            ArbitrationState::Locked(active) => active == transport,
            ArbitrationState::Arbitrating => {
                let parsed = bareboot_protocol::scan(frame);
                if let FrameParse::Complete { command, .. } = parsed {
                    if command == commands::CONNECT {
                        self.state = ArbitrationState::Locked(transport);
                        return true;
                    }
                }
                false
            }
            ArbitrationState::Expired => false,
        }
    }
}

#[cfg(test)]
mod multi_tests {
    use super::*;

    #[test]
    fn test_mailbox_directed_jump() {
        let mb = SoftResetMailbox::new(
            bareboot_protocol::REQUEST_CANBOOT,
            0x30,
            SoftResetMailbox::BUS_TYPE_I2C,
        );
        let arb = MultiTransportArbiter::from_mailbox(&mb).unwrap();
        assert!(arb.is_locked());
        assert_eq!(arb.active_transport(), Some(TransportKind::I2c));
        assert!(arb.is_infinite_window);
        assert_eq!(arb.boot_window_ms, u32::MAX);
    }

    #[test]
    fn test_shared_pin_time_slicing() {
        let mut arb = MultiTransportArbiter::new(250)
            .with_shared_transports(50, &[TransportKind::Uart, TransportKind::I2c]);

        assert_eq!(arb.active_transport(), Some(TransportKind::Uart));

        let act = arb.tick(40);
        assert_eq!(act, ArbitrationAction::Listen(TransportKind::Uart));

        let act = arb.tick(15);
        assert_eq!(
            act,
            ArbitrationAction::SwitchInterface {
                from: TransportKind::Uart,
                to: TransportKind::I2c,
            }
        );
        assert_eq!(arb.active_transport(), Some(TransportKind::I2c));

        // CONNECT arrives on I2C during its slice
        let mut frame = [0u8; 64];
        let flen = bareboot_protocol::encode_frame(commands::CONNECT, &[], &mut frame).unwrap();
        assert!(arb.handle_incoming_frame(TransportKind::I2c, &frame[..flen]));

        assert!(arb.is_locked());
        assert_eq!(arb.active_transport(), Some(TransportKind::I2c));

        // Subsequent ticks stay selected
        let act = arb.tick(100);
        assert_eq!(act, ArbitrationAction::StaySelected(TransportKind::I2c));
    }

    #[test]
    fn test_concurrent_polling_locks_winner() {
        let mut arb = MultiTransportArbiter::new(200);

        let mut connect_frame = [0u8; 64];
        let flen = bareboot_protocol::encode_frame(commands::CONNECT, &[], &mut connect_frame).unwrap();

        // CAN receives connect first
        assert!(arb.handle_incoming_frame(TransportKind::Can, &connect_frame[..flen]));
        assert_eq!(arb.active_transport(), Some(TransportKind::Can));

        // Frame on other transport (USB) is rejected
        assert!(!arb.handle_incoming_frame(TransportKind::Usb, &connect_frame[..flen]));
    }

    #[test]
    fn test_arbitration_timeout_expires() {
        let mut arb = MultiTransportArbiter::new(100);
        assert_eq!(arb.tick(50), ArbitrationAction::Listen(TransportKind::Can));
        assert_eq!(arb.tick(60), ArbitrationAction::TimeoutExpired);
        assert_eq!(arb.state, ArbitrationState::Expired);
    }
}
