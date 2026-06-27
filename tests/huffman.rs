use fossil::core::models::huffman::{canonical_codes, code_lengths, decode, encode};

#[test]
fn empty_has_no_lengths() {
    assert_eq!(code_lengths(&[]), [0u8; 256]);
}

#[test]
fn single_symbol_gets_length_one() {
    let lens = code_lengths(&[0x41; 50]);
    assert_eq!(lens[0x41], 1);
    assert_eq!(lens.iter().filter(|&&l| l > 0).count(), 1);
}

#[test]
fn satisfies_kraft_equality() {
    let lens = code_lengths(b"this is an example of a huffman tree");
    let sum: f64 = lens
        .iter()
        .filter(|&&l| l > 0)
        .map(|&l| 2f64.powi(-(l as i32)))
        .sum();
    assert!((sum - 1.0).abs() < 1e-9);
}

#[test]
fn frequent_symbol_not_longer_than_rare() {
    let mut data = vec![b'a'; 100];
    data.extend(std::iter::repeat(b'b').take(10));
    data.push(b'z');
    let lens = code_lengths(&data);
    assert!(lens[b'a' as usize] <= lens[b'z' as usize]);
}

#[test]
fn known_canonical_assignment() {
    let mut lengths = [0u8; 256];
    lengths[0] = 2;
    lengths[1] = 1;
    lengths[2] = 3;
    lengths[3] = 3;

    let codes = canonical_codes(&lengths);
    assert_eq!(codes[1], 0b0);
    assert_eq!(codes[0], 0b10);
    assert_eq!(codes[2], 0b110);
    assert_eq!(codes[3], 0b111);
}

#[test]
fn codes_fit_within_their_lengths() {
    let lengths = fossil::core::models::huffman::code_lengths(
        b"this is an example of a huffman
  tree",
    );
    let codes = canonical_codes(&lengths);
    for s in 0..256 {
        if lengths[s] > 0 {
            assert!(codes[s] < (1u32 << lengths[s]));
        }
    }
}

fn roundtrip(data: &[u8]) {
    assert_eq!(decode(&encode(data), data.len()), data);
}

#[test]
fn roundtrips_text() {
    roundtrip(b"the quick brown fox jumps over the lazy dog");
}

#[test]
fn roundtrips_single_symbol() {
    roundtrip(&[b'a'; 300]);
}

#[test]
fn roundtrips_all_byte_values() {
    let data: Vec<u8> = (0..=255).collect();
    roundtrip(&data);
}

#[test]
fn roundtrips_empty() {
    assert_eq!(decode(&encode(&[]), 0), Vec::<u8>::new());
}

#[test]
fn shrinks_skewed_data() {
    let mut data = vec![b'a'; 10000];
    data.extend(std::iter::repeat(b'b').take(200));
    assert!(encode(&data).len() < data.len());
}

#[test]
fn small_alphabet_has_tiny_table() {
    let enc = encode(b"aaaaabbbbbcccccddddd");
    assert!(enc.len() < 30);
}
