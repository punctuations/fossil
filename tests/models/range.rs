use fossil::core::models::range::{decode, encode};

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
fn roundtrips_single_symbol() {
    roundtrip(&[7u8; 500]);
}

#[test]
fn roundtrips_all_byte_values() {
    let data: Vec<u8> = (0..=255).collect();
    roundtrip(&data);
}

#[test]
fn shrinks_skewed_data() {
    let mut data = vec![0u8; 5000];
    for i in 0..100 {
        data[i * 50] = 1;
    }
    assert!(encode(&data).len() < data.len());
}
