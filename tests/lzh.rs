use fossil::core::models::lzh::{decode, encode};

fn roundtrip(data: &[u8]) {
    assert_eq!(decode(&encode(data), data.len()), data);
}

#[test]
fn roundtrips_text() {
    roundtrip(b"the cat sat on the mat, the cat sat on the rug");
}

#[test]
fn roundtrips_empty() {
    roundtrip(&[]);
}

#[test]
fn roundtrips_binary() {
    let data: Vec<u8> = (0..6000).map(|i| (i % 256) as u8).collect();
    roundtrip(&data);
}

#[test]
fn shrinks_mixed_text() {
    let data = b"lorem ipsum dolor sit amet ".repeat(80);
    assert!(encode(&data).len() < data.len());
}
