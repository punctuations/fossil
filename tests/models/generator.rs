use fossil::core::models::generator::{decode, encode};

fn roundtrip(data: &[u8]) {
    assert_eq!(decode(&encode(data)), data);
}

#[test]
fn roundtrips_ramp() {
    roundtrip(&(0..=255).collect::<Vec<u8>>());
}

#[test]
fn roundtrips_const() {
    roundtrip(&[7u8; 100]);
}

#[test]
fn roundtrips_mixed() {
    let mut d: Vec<u8> = (0..50).collect();
    d.extend(b"hello literal text");
    d.extend(vec![9u8; 30]);
    roundtrip(&d);
}

#[test]
fn roundtrips_empty() {
    roundtrip(&[]);
}

#[test]
fn ramp_is_tiny() {
    assert!(encode(&(0..=255).collect::<Vec<u8>>()).len() < 10);
}
