use fossil::core::models::bwt::{forward, inverse};

fn roundtrip(data: &[u8]) {
    let (last, primary) = forward(data);
    assert_eq!(inverse(&last, primary), data);
}

#[test]
fn roundtrips_banana() {
    roundtrip(b"banana");
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
fn clusters_similar_context() {
    // BWT of repetitive text should produce long same-byte runs
    let (last, _) = forward(&b"abracadabra".repeat(20));
    let runs = last.windows(2).filter(|w| w[0] == w[1]).count();
    assert!(runs > 100);
}
