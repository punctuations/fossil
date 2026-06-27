use fossil::core::bits::{BitReader, BitWriter};

#[test]
fn bits_roundtrip_in_order() {
    let pattern = [1u8, 0, 1, 1, 0, 0, 0, 1, 1, 1, 0];

    let mut w = BitWriter::new();
    for &b in &pattern {
        w.write_bit(b);
    }
    let bytes = w.finish();

    let mut r = BitReader::new(&bytes);
    let mut got = Vec::new();
    for _ in 0..pattern.len() {
        got.push(r.read_bit().unwrap());
    }
    assert_eq!(got, pattern);
}

#[test]
fn reader_runs_out() {
    let mut r = BitReader::new(&[0xFF]);
    for _ in 0..8 {
        assert!(r.read_bit().is_some());
    }
    assert!(r.read_bit().is_none());
}
