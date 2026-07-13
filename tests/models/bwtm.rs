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

use fossil::core::models::bwtm;

fn roundtrip2(data: &[u8]) {
    assert_eq!(bwtm::decode2(&bwtm::encode2(data), data.len()), data);
}

#[test]
fn bwtm2_roundtrips_text() {
    roundtrip2(b"the quick brown fox jumps over the lazy dog");
}

#[test]
fn bwtm2_roundtrips_empty() {
    roundtrip2(&[]);
}

#[test]
fn bwtm2_roundtrips_repeats() {
    roundtrip2(&[0x42u8; 500]);
}

#[test]
fn bwtm2_roundtrips_all_bytes() {
    let data: Vec<u8> = (0..=255).collect();
    roundtrip2(&data);
}

#[test]
fn bwtm2_beats_bwtm_on_repetitive_text() {
    let data = b"abracadabra ".repeat(300);
    let old = bwtm::encode(&data);
    let new = bwtm::encode2(&data);
    assert!(new.len() < old.len(), "bwtm2 {} >= bwtm {}", new.len(), old.len());
}
