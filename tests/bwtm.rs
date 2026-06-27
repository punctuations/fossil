use fossil::core::models::bwtm::{decode, encode};

fn roundtrip(data: &[u8]) {
    assert_eq!(decode(&encode(data), data.len()), data);
}

#[test]
fn roundtrips_text() {
    roundtrip(b"the quick brown fox jumps over the lazy dog");
}

#[test]
fn roundtrips_empty() {
    roundtrip(&[]);
}

#[test]
fn roundtrips_repeats() {
    roundtrip(&[0x42u8; 500]);
}

#[test]
fn roundtrips_all_bytes() {
    let data: Vec<u8> = (0..=255).collect();
    roundtrip(&data);
}

#[test]
fn shrinks_repetitive_text() {
    let data = b"abracadabra ".repeat(80);
    assert!(encode(&data).len() < data.len());
}
