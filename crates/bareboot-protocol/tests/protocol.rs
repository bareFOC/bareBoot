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

#[test]
fn test_constants_and_crc32() {
    assert_eq!(REQUEST_CANBOOT, 0x5984_E3FA_6CA1_589B);
    assert_eq!(REQUEST_APP, 0x4649_5348_4150_5031);
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(crc32(b""), 0x0000_0000);
}

#[test]
fn test_fasthash64_and_uuid() {
    let uid = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44];
    let hash = fasthash64(&uid, KATAPULT_UUID_SEED);
    assert_ne!(hash, 0);

    let uuid = katapult_can_uuid(&uid);
    let hash_bytes = hash.to_le_bytes();
    assert_eq!(&uuid[..], &hash_bytes[..6]);
}

#[test]
fn test_fallback_address() {
    let uid1 = [0x00112233, 0x44556677, 0x8899AABB];
    let addr1 = fallback_address(&uid1);
    assert!((0x10..=0x4F).contains(&addr1));

    let uid2 = [0xFFFFFFFF, 0x00000000, 0x00000000];
    let addr2 = fallback_address(&uid2);
    assert_eq!(addr2, 0x10 + (0x3F & 0xFF));

    let uid_zero = [0, 0, 0];
    assert_eq!(fallback_address(&uid_zero), 0x10);
}

#[test]
fn test_soft_reset_mailbox_lifecycle() {
    assert_eq!(core::mem::size_of::<SoftResetMailbox>(), 32);

    let mb = SoftResetMailbox::new(REQUEST_CANBOOT, 0x42, SoftResetMailbox::BUS_TYPE_I3C);
    assert_eq!(mb.dynamic_node_id, 0x42);
    assert_eq!(mb.bus_type, SoftResetMailbox::BUS_TYPE_I3C);
    assert!(mb.is_valid());

    let bytes = mb.to_bytes();
    assert_eq!(bytes.len(), 32);

    let decoded = SoftResetMailbox::from_bytes(&bytes);
    assert_eq!(decoded, mb);
    assert!(decoded.is_valid());

    // Corrupted payload fails CRC validation
    let mut corrupted = bytes;
    corrupted[8] ^= 0xFF; // corrupt dynamic_node_id
    let bad_mb = SoftResetMailbox::from_bytes(&corrupted);
    assert!(!bad_mb.is_valid());

    // Invalid magic fails validation even with matching CRC
    let mut bad_magic_mb = mb;
    bad_magic_mb.magic = 0x1234_5678_9ABC_DEF0;
    bad_magic_mb.crc32 = bad_magic_mb.compute_crc32();
    assert!(!bad_magic_mb.is_valid());
}
