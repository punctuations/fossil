use fossil::core::models::delta::{decode, encode};

fn roundtrip(data: &[u8]) {
    assert_eq!(decode(&encode(data), data.len()), data);
}

#[test]
fn roundtrips_text() {
    roundtrip(b"the quick brown fox");
}

#[test]
fn roundtrips_empty() {
    roundtrip(&[]);
}

#[test]
fn roundtrips_all_bytes() {
    roundtrip(&(0..=255).collect::<Vec<u8>>());
}

#[test]
fn shrinks_smooth_signal() {
    let data: Vec<u8> = (0..3000).map(|i| (i / 10) as u8).collect();
    assert!(encode(&data).len() < data.len());
}
