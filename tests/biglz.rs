use fossil::core::biglz::{decode, encode};

fn roundtrip(data: &[u8]) {
    assert_eq!(decode(&encode(data), data.len()), data);
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
fn roundtrips_runs() {
    roundtrip(&[0x42u8; 5000]);
}

#[test]
fn roundtrips_random() {
    let data: Vec<u8> = (0u32..3000)
        .map(|i| (i.wrapping_mul(2654435761) >> 16) as u8)
        .collect();
    roundtrip(&data);
}

#[test]
fn dedupes_distant_duplicate() {
    let chunk: Vec<u8> = (0..2000).map(|x| (x * 7) as u8).collect();
    let mut data = chunk.clone();
    data.extend(vec![0u8; 8000]); // gap far bigger than the 4096 block window
    data.extend(&chunk); // identical chunk, 10 KB later
    let enc = encode(&data);
    assert!(enc.len() < data.len() / 2); // the 2nd copy collapses to ~one match
    assert_eq!(decode(&enc, data.len()), data);
}
