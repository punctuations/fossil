use fossil::core::bundle::{pack, unpack};

#[test]
fn roundtrips_multiple_files() {
    let files = vec![
        ("a.txt".to_string(), b"hello".to_vec()),
        ("nested/b.bin".to_string(), vec![0u8, 1, 2, 3, 255]),
        ("empty".to_string(), vec![]),
    ];
    assert_eq!(unpack(&pack(&files)), files);
}

#[test]
fn roundtrips_empty_bundle() {
    let files: Vec<(String, Vec<u8>)> = vec![];
    assert_eq!(unpack(&pack(&files)), files);
}
