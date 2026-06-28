use fossil::core::models::ppm::{decode, encode};

fn roundtrip(data: &[u8]) {
    assert_eq!(decode(&encode(data), data.len()), data);
}

#[test]
fn roundtrips_text() {
    roundtrip(b"the quick brown fox jumps over the lazy dog the quick brown fox");
}

#[test]
fn roundtrips_empty() {
    roundtrip(&[]);
}

#[test]
fn roundtrips_all_byte_values() {
    let data: Vec<u8> = (0..=255).collect();
    roundtrip(&data);
}

#[test]
fn beats_order0_on_text() {
    let data = b"she sells sea shells by the sea shore ".repeat(40);
    let order1 = encode(&data).len();
    let order0 = fossil::core::models::range::encode(&data).len();
    assert!(order1 <= order0);
}
