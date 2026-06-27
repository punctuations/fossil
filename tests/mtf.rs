use fossil::core::models::mtf::{forward, inverse};

fn roundtrip(data: &[u8]) {
    assert_eq!(inverse(&forward(data)), data);
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
fn runs_collapse_to_zeros() {
    let out = forward(&[0x42u8; 100]);
    assert_eq!(out[0], 66); // 0x42 starts at index 66
    assert!(out[1..].iter().all(|&x| x == 0)); // rest are repeats -> 0
}
