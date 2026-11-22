#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Tx,
    Rx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusType {
    Can,
    I2c,
    Serial,
    Memory,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedFrame {
    pub timestamp: String,
    pub direction: Direction,
    pub bus_type: BusType,
    pub node_id: String,
    pub dlc: usize,
    pub hex: String,
    pub opcode: String,
    pub detail: String,
    pub is_ack: bool,
    pub is_error: bool,
}

pub mod opcodes {
    pub const CONNECT: u8 = 0x11;
    pub const RX_BLOCK: u8 = 0x12;
    pub const RX_EOF: u8 = 0x13;
    pub const REQ_BLOCK: u8 = 0x14;
    pub const COMPLETE: u8 = 0x15;
    pub const GET_CANBUS_ID: u8 = 0x16;
    pub const ACK: u8 = 0xa0;
    pub const NACK: u8 = 0xf1;
    pub const COMMAND_ERROR: u8 = 0xf2;
    pub const COMMAND_BUSY: u8 = 0xf3;
    pub const I2C_ENTER_BL: u8 = 0xf0;
    pub const CAN_REBOOT: u8 = 0x02;
}

pub fn decode_packet(
    timestamp: &str,
    direction: Direction,
    bus_type: BusType,
    node_id: &str,
    data: &[u8],
) -> DecodedFrame {
    let dlc = data.len();
    let hex = data
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");

    let (opcode, detail, is_ack, is_error) = if data.is_empty() {
        ("EMPTY".to_string(), "0-byte payload".to_string(), false, false)
    } else {
        let cmd = if data.len() >= 3 && data[0] == 0x01 && data[1] == 0x88 {
            data[2]
        } else {
            data[0]
        };

        match cmd {
            opcodes::CONNECT => (
                "CONNECT".to_string(),
                "Handshake & query MCU parameters".to_string(),
                false,
                false,
            ),
            opcodes::RX_BLOCK => {
                let size = if data.len() >= 4 { data.len() - 4 } else { data.len() };
                (
                    "RX_BLOCK".to_string(),
                    format!("Flash chunk ({size} bytes)"),
                    false,
                    false,
                )
            }
            opcodes::RX_EOF => (
                "RX_EOF".to_string(),
                "End of image stream".to_string(),
                false,
                false,
            ),
            opcodes::REQ_BLOCK => (
                "REQ_BLOCK".to_string(),
                "Request block verify".to_string(),
                false,
                false,
            ),
            opcodes::COMPLETE => (
                "COMPLETE".to_string(),
                "Bootloader exit / jump to app".to_string(),
                true,
                false,
            ),
            opcodes::GET_CANBUS_ID => (
                "GET_CAN_ID".to_string(),
                "Query node assigned CAN ID".to_string(),
                false,
                false,
            ),
            opcodes::ACK => (
                "ACK".to_string(),
                "Command acknowledged OK".to_string(),
                true,
                false,
            ),
            opcodes::NACK => (
                "NACK".to_string(),
                "Negative acknowledge - retry needed".to_string(),
                false,
                true,
            ),
            opcodes::COMMAND_ERROR => (
                "ERROR".to_string(),
                "Command rejected or verify failed".to_string(),
                false,
                true,
            ),
            opcodes::COMMAND_BUSY => (
                "BUSY".to_string(),
                "Flash write in progress".to_string(),
                false,
                false,
            ),
            opcodes::I2C_ENTER_BL => (
                "ENTER_BL".to_string(),
                "I2C bootloader entry signal".to_string(),
                false,
                false,
            ),
            other => (
                format!("OP_{other:02X}"),
                format!("Payload {dlc}B"),
                false,
                false,
            ),
        }
    };

    DecodedFrame {
        timestamp: timestamp.to_string(),
        direction,
        bus_type,
        node_id: node_id.to_string(),
        dlc,
        hex,
        opcode,
        detail,
        is_ack,
        is_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_katapult_header() {
        let raw = [0x01, 0x88, 0x11, 0x00, 0xAA, 0xBB, 0x99, 0x03];
        let frame = decode_packet("12:00:00.100", Direction::Tx, BusType::Can, "0x100", &raw);
        assert_eq!(frame.opcode, "CONNECT");
        assert_eq!(frame.is_ack, false);
        assert_eq!(frame.is_error, false);
    }

    #[test]
    fn test_decode_ack_and_nack() {
        let ack = decode_packet("12:00:00.110", Direction::Rx, BusType::Can, "0x100", &[opcodes::ACK, 0x01]);
        assert_eq!(ack.opcode, "ACK");
        assert!(ack.is_ack);

        let nack = decode_packet("12:00:00.120", Direction::Rx, BusType::Can, "0x100", &[opcodes::NACK]);
        assert_eq!(nack.opcode, "NACK");
        assert!(nack.is_error);
    }
}
