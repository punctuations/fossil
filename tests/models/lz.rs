use fossil::core::models::lz::{decode, encode};

fn roundtrip(data: &[u8]) {
    assert_eq!(decode(&encode(data), data.len()), data);
}

#[test]
fn roundtrips_repeated_substrings() {
    roundtrip(b"the cat sat on the mat, the cat sat on the rug");
}

#[test]
fn roundtrips_runs() {
    roundtrip(&[0x42u8; 1000]);
}

#[test]
fn roundtrips_empty() {
    roundtrip(&[]);
}

#[test]
fn roundtrips_no_matches() {
    let data: Vec<u8> = (0..=255).collect();
    roundtrip(&data);
}

#[test]
fn shrinks_repetitive_text() {
    let data = b"abcabcabc".repeat(200);
    assert!(encode(&data).len() < data.len());
}
