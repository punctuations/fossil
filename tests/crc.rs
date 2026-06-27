use fossil::core::crc::crc32;

#[test]
fn known_check_value() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926); // canonical CRC-32 test vector
}

#[test]
fn empty_is_zero() {
    assert_eq!(crc32(&[]), 0);
}

#[test]
fn detects_single_bit_flip() {
    assert_ne!(crc32(b"hello"), crc32(b"hellp"));
}
