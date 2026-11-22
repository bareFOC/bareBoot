pub use bareboot_engine::protocol::*;

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
