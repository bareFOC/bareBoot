use bareboot_protocol::*;

#[test]
fn test_fast_hash64_katapult_parity() {
    let uid = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44];
    let hash = fasthash64(&uid, KATAPULT_UUID_SEED);
    assert_ne!(hash, 0);

    let uuid = katapult_can_uuid(&uid);
    let hash_bytes = hash.to_le_bytes();
    assert_eq!(&uuid[..], &hash_bytes[..6]);
}

#[test]
fn test_fallback_address_contract() {
    let uid1 = [0x00112233, 0x44556677, 0x8899AABB];
    let addr1 = fallback_address(&uid1);
    assert!((0x10..=0x4F).contains(&addr1));

    let uid2 = [0, 0, 0];
    assert_eq!(fallback_address(&uid2), 0x10);

    let uid3 = [0xFFFFFFFF, 0, 0];
    assert_eq!(fallback_address(&uid3), 0x10 + 0x3F);
}

#[test]
fn test_soft_reset_mailbox_validation_and_rejection() {
    let mb = SoftResetMailbox::new(REQUEST_CANBOOT, 0x42, SoftResetMailbox::BUS_TYPE_I3C);
    assert_eq!(mb.dynamic_node_id, 0x42);
    assert_eq!(mb.bus_type, SoftResetMailbox::BUS_TYPE_I3C);
    assert!(mb.is_valid());

    let bytes = mb.to_bytes();
    let decoded = SoftResetMailbox::from_bytes(&bytes);
    assert_eq!(decoded, mb);
    assert!(decoded.is_valid());

    // Corrupt dynamic node ID
    let mut corrupted = bytes;
    corrupted[8] ^= 0x55;
    let bad_mb = SoftResetMailbox::from_bytes(&corrupted);
    assert!(!bad_mb.is_valid());

    // Corrupt CRC
    let mut corrupted_crc = bytes;
    corrupted_crc[31] ^= 0x80;
    let bad_crc_mb = SoftResetMailbox::from_bytes(&corrupted_crc);
    assert!(!bad_crc_mb.is_valid());
}

#[test]
fn test_request_app_mailbox() {
    let mb = SoftResetMailbox::new(REQUEST_APP, 0x30, SoftResetMailbox::BUS_TYPE_I2C);
    assert_eq!(mb.magic, REQUEST_APP);
    assert_eq!(mb.dynamic_node_id, 0x30);
    assert_eq!(mb.bus_type, SoftResetMailbox::BUS_TYPE_I2C);
    assert!(mb.is_valid());
}
