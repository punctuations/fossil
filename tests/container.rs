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

#[test]
fn reads_legacy_v1_block_framing() {
    let orig = b"abcdefghij";
    let crc = fossil::core::crc::crc32(orig);

    let mut v1 = Vec::new();
    v1.extend_from_slice(b"FOSL");
    v1.push(1);
    v1.push(0);
    v1.push(0);
    v1.push(3);
    v1.extend_from_slice(b"txt");
    v1.push(10);
    v1.extend_from_slice(&crc.to_le_bytes());
    v1.push(2);
    v1.push(0);
    v1.push(6);
    v1.push(6);
    v1.extend_from_slice(b"abcdef");
    v1.push(0);
    v1.push(4);
    v1.push(4);
    v1.extend_from_slice(b"ghij");

    let c = fossil::core::container::read(&v1).unwrap();
    assert_eq!(c.ext, "txt");
    assert_eq!(c.blocks.len(), 2);
    assert_eq!(c.blocks[0].orig_len, 6);
    assert_eq!(c.decode(), orig);

    let mut lazy = fossil::core::container::read_lazy(&v1).unwrap();
    assert_eq!(lazy.blocks[1].orig_len, 4);
    assert_eq!(lazy.read_range(2, 4).unwrap(), b"cdef");
}

#[test]
fn v2_framing_round_trips_multi_block() {
    let data: Vec<u8> = (0..20000u32).map(|i| (i % 251) as u8).collect();
    let packed = fossil::core::container::write(&data, "bin");
    assert_eq!(packed[4], 2);

    let c = fossil::core::container::read(&packed).unwrap();
    assert_eq!(c.decode(), data);

    let mut lazy = fossil::core::container::read_lazy(&packed).unwrap();
    assert_eq!(lazy.read_range(8000, 5000).unwrap(), &data[8000..13000]);
}
