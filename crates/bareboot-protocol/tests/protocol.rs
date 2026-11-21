use bareboot_protocol::*;

#[test]
fn crc_matches_c_implementation() {
    assert_eq!(crc16_ccitt(&[0x11, 0x00]), 0x7cf1);
    assert_eq!(crc16_ccitt(&[0x11, 0x02, 0xaa, 0xbb, 0xcc, 0xdd]), 0xeb76);
}

#[test]
fn frame_roundtrip() {
    let payload = [0xde, 0xad, 0xbe, 0xef];
    let mut buf = [0u8; 64];
    let len = encode_frame(commands::RX_BLOCK, &payload, &mut buf).unwrap();
    let parsed = scan(&buf[..len]);
    match parsed {
        FrameParse::Complete { frame_len, command, word_count, payload_offset } => {
            assert_eq!(frame_len, len);
            assert_eq!(command, commands::RX_BLOCK);
            assert_eq!(word_count, 1);
            assert_eq!(&buf[payload_offset..payload_offset + 4], &payload);
        }
        other => panic!("unexpected parse result: {other:?}"),
    }
}

#[test]
fn truncated_frame_needs_more() {
    let payload = [0xde, 0xad, 0xbe, 0xef];
    let mut buf = [0u8; 64];
    let len = encode_frame(commands::RX_BLOCK, &payload, &mut buf).unwrap();
    let parsed = scan(&buf[..len - 1]);
    assert_eq!(parsed, FrameParse::NeedMore);
}

#[test]
fn bad_crc_is_invalid() {
    let payload = [0xde, 0xad, 0xbe, 0xef];
    let mut buf = [0u8; 64];
    let len = encode_frame(commands::RX_BLOCK, &payload, &mut buf).unwrap();
    buf[4] ^= 0xff;
    let parsed = scan(&buf[..len]);
    assert!(matches!(parsed, FrameParse::Invalid { .. }));
}

#[test]
fn test_ccc_and_entry_contracts() {
    assert_eq!(I2C_CMD_ENTER_BOOTLOADER, 0xF0);
    assert_eq!(I3C_BROADCAST_ADDR, 0x7E);
    assert_eq!(CCC_BCAST_ENTDAA, 0x07);
    assert_eq!(CCC_BCAST_RSTACT, 0x2A);
    assert_eq!(CCC_DIRECT_RSTACT, 0x9A);
    assert_eq!(CCC_DIRECT_SETDASA, 0x87);
    assert_eq!(RSTACT_DEF_TGT_RESET, 0x02);
}
