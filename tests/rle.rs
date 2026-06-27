use fossil::core::models::rle::{decode, encode};

#[test]
fn roundtrips_repetitive_data() {
    let data = b"AAAAABBBCCCCCCCC";
    assert_eq!(decode(&encode(data)), data);
}

#[test]
fn roundtrips_empty() {
    assert_eq!(encode(&[]), Vec::<u8>::new());
    assert_eq!(decode(&[]), Vec::<u8>::new());
}

#[test]
fn roundtrips_no_repeats() {
    let data: Vec<u8> = (0..=255).collect();
    assert_eq!(decode(&encode(&data)), data);
}

#[test]
fn one_run_is_five_bytes() {
    assert_eq!(encode(&[0x42; 1000]).len(), 5);
}
