use fossil::core::container::{read, write};

fn roundtrip(data: &[u8], ext: &str) {
    let packed = write(data, ext);
    let c = read(&packed).unwrap();
    assert_eq!(c.ext, ext);
    assert_eq!(c.orig_size, data.len());
    assert_eq!(c.decode(), data);
}

#[test]
fn roundtrips_text() {
    roundtrip(b"the quick brown fox jumps over the lazy dog", "txt");
}

#[test]
fn roundtrips_zeros() {
    roundtrip(&[0u8; 100_000], "bin");
}

#[test]
fn roundtrips_empty() {
    roundtrip(&[], "");
}

#[test]
fn roundtrips_multiblock_mixed() {
    let mut data = vec![0x42u8; 5000];
    data.extend((0..5000).map(|i| (i % 256) as u8));
    data.extend(b"aab".repeat(2000));
    roundtrip(&data, "dat");
}

#[test]
fn rejects_bad_magic() {
    assert!(read(b"XXXXrest of junk").is_err());
}

#[test]
fn incompressible_data_uses_stored_form() {
    let data: Vec<u8> = (0..20000).map(|i| (i % 256) as u8).collect();
    let packed = write(&data, "bin");
    assert!(packed.len() <= data.len() + 32);
    assert_eq!(read(&packed).unwrap().decode(), data);
}

#[test]
fn tiny_file_roundtrips() {
    roundtrip(b"hi", "txt");
}
