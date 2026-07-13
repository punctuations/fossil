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

use fossil::core::biglz;

fn roundtrip2(data: &[u8]) {
    let tokens = biglz::tokens(data, 0);
    let enc = lzr::encode_tokens2(data, 0, &tokens);
    assert_eq!(lzr::decode_windowed2(&enc, data.len(), &[]), data);
}

#[test]
fn lzr2_round_trips_repetitive_text() {
    let data = b"the quick brown fox the quick brown fox ".repeat(200);
    roundtrip2(&data);
}

#[test]
fn lzr2_round_trips_strided_records() {
    let mut data = Vec::new();
    for i in 0u32..500 {
        data.extend_from_slice(&i.to_le_bytes());
        data.extend_from_slice(b"ROW-PADDING-");
    }
    roundtrip2(&data);
}

#[test]
fn lzr2_round_trips_pseudo_random() {
    let data: Vec<u8> = (0u64..5000)
        .map(|i| {
            let mut x = i.wrapping_mul(0x9E3779B97F4A7C15);
            x ^= x >> 29;
            (x >> 24) as u8
        })
        .collect();
    roundtrip2(&data);
}

#[test]
fn lzr2_round_trips_empty() {
    roundtrip2(&[]);
}

#[test]
fn lzr2_windowed_matches_into_history() {
    let pattern = b"abcdefghij".repeat(50);
    let history = pattern.clone();

    let mut combined = history.clone();
    combined.extend_from_slice(&pattern);

    let tokens = biglz::tokens(&combined, history.len());
    let enc = lzr::encode_tokens2(&combined, history.len(), &tokens);
    assert_eq!(lzr::decode_windowed2(&enc, pattern.len(), &history), pattern);
}

#[test]
fn lzr2_beats_lzr_on_strided_data() {
    let mut data = Vec::new();
    for i in 0u32..800 {
        data.extend_from_slice(b"field,");
        data.extend_from_slice(&(i % 10).to_le_bytes());
    }
    let tokens = biglz::tokens(&data, 0);
    let old = lzr::encode_tokens(&data, 0, &tokens);
    let new = lzr::encode_tokens2(&data, 0, &tokens);
    assert!(new.len() <= old.len(), "lzr2 {} > lzr {}", new.len(), old.len());
}
