use fossil::core::models::lzr;

#[test]
fn round_trips_repetitive_text() {
    let data = b"the quick brown fox the quick brown fox ".repeat(200);
    let enc = lzr::encode(&data);
    assert!(enc.len() < data.len());
    assert_eq!(lzr::decode(&enc, data.len()), data);
}

#[test]
fn round_trips_pseudo_random() {
    let data: Vec<u8> = (0u64..5000)
        .map(|i| {
            let mut x = i.wrapping_mul(0x9E3779B97F4A7C15);
            x ^= x >> 29;
            (x >> 24) as u8
        })
        .collect();
    let enc = lzr::encode(&data);
    assert_eq!(lzr::decode(&enc, data.len()), data);
}

#[test]
fn round_trips_empty() {
    let enc = lzr::encode(&[]);
    assert_eq!(lzr::decode(&enc, 0), Vec::<u8>::new());
}

#[test]
fn windowed_matches_into_history() {
    let pattern = b"abcdefghij".repeat(50);
    let history = pattern.clone();

    let mut combined = history.clone();
    combined.extend_from_slice(&pattern);

    let enc = lzr::encode_from(&combined, history.len());
    assert_eq!(lzr::decode_windowed(&enc, pattern.len(), &history), pattern);
}
