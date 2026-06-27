use fossil::core::block::{ENTROPY, RAW, RLE, decode_block, encode_block};

#[test]
fn incompressible_block_stays_raw() {
    let data: Vec<u8> = (0..=255).collect();
    let (model, payload) = encode_block(&data);
    assert_eq!(model, RAW);
    assert_eq!(decode_block(model, &payload, data.len()), data);
}

#[test]
fn repetitive_block_picks_rle() {
    let data = [0x42u8; 1000];
    let (model, payload) = encode_block(&data);
    assert_eq!(model, RLE);
    assert_eq!(decode_block(model, &payload, data.len()), data);
}

#[test]
fn noisy_skew_roundtrips_and_shrinks() {
    let data = b"aab".repeat(1000);
    let (model, payload) = encode_block(&data);
    assert!(payload.len() < data.len());
    assert_eq!(decode_block(model, &payload, data.len()), data);
}

#[test]
fn repeated_substrings_compress_and_roundtrip() {
    let data = b"abcabcabc".repeat(500);
    let (model, payload) = encode_block(&data);
    assert!(payload.len() < data.len());
    assert_eq!(decode_block(model, &payload, data.len()), data);
}
