use fossil::core::models::word::{decode, encode};

fn roundtrip(d: &[u8]) {
    assert_eq!(decode(&encode(d), d.len()), d);
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
    let d: Vec<u8> = (0..=255).collect();
    roundtrip(&d);
}

#[test]
fn shrinks_repeated_words() {
    let d = b"alpha beta gamma ".repeat(200);
    assert!(encode(&d).len() < d.len());
}
