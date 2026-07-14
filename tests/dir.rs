use fossil::core::{
    container::{read, write_progress_meta},
    crc, dir,
};

fn sample_files() -> Vec<(String, Vec<u8>)> {
    vec![
        ("core/main.rs".to_string(), b"fn main() {}\n".to_vec()),
        ("core/lib.rs".to_string(), b"pub fn lib() {}\n".to_vec()),
        ("examples/demo.txt".to_string(), b"demo\n".to_vec()),
        ("README.md".to_string(), b"# fossil\n".to_vec()),
    ]
}

#[test]
fn manifest_roundtrips_paths_lengths_and_offsets() {
    let files = sample_files();

    let (meta, payload) = dir::pack(&files);
    let entries = dir::read(&meta).unwrap();

    assert_eq!(entries.len(), files.len());

    let mut offset = 0;

    for (entry, (path, contents)) in entries.iter().zip(files.iter()) {
        assert_eq!(&entry.path, path);
        assert_eq!(entry.offset, offset);
        assert_eq!(entry.len, contents.len());

        let start = entry.offset;
        let end = start + entry.len;

        assert_eq!(&payload[start..end], contents);

        offset += contents.len();
    }

    assert_eq!(payload.len(), offset);
}

#[test]
fn directory_payload_is_plain_concatenation() {
    let files = sample_files();

    let (_meta, payload) = dir::pack(&files);

    let expected = files
        .iter()
        .flat_map(|(_, contents)| contents.iter().copied())
        .collect::<Vec<u8>>();

    assert_eq!(payload, expected);
}

#[test]
fn container_preserves_directory_metadata() {
    let files = sample_files();

    let (meta, payload) = dir::pack(&files);
    let packed = write_progress_meta(&payload, "/", &meta, None, true);

    let c = read(&packed).unwrap();

    assert_eq!(c.ext, "/");
    assert_eq!(c.orig_size, payload.len());
    assert_eq!(c.meta, meta);
    assert_eq!(c.decode(), payload);
}

#[test]
fn directory_container_entries_slice_decoded_payload() {
    let files = sample_files();

    let (meta, payload) = dir::pack(&files);
    let packed = write_progress_meta(&payload, "/", &meta, None, true);

    let c = read(&packed).unwrap();
    let decoded = c.decode();
    let entries = dir::read(&c.meta).unwrap();

    for (entry, (path, contents)) in entries.iter().zip(files.iter()) {
        assert_eq!(&entry.path, path);

        let start = entry.offset;
        let end = start + entry.len;

        assert_eq!(&decoded[start..end], contents);
    }
}

#[test]
fn empty_directory_manifest_roundtrips() {
    let files: Vec<(String, Vec<u8>)> = Vec::new();

    let (meta, payload) = dir::pack(&files);
    let entries = dir::read(&meta).unwrap();

    assert!(entries.is_empty());
    assert!(payload.is_empty());

    let packed = write_progress_meta(&payload, "/", &meta, None, true);
    let c = read(&packed).unwrap();

    assert_eq!(c.ext, "/");
    assert_eq!(c.meta, meta);
    assert_eq!(c.decode(), payload);
}

#[test]
fn rejects_bad_manifest_magic() {
    assert!(dir::read(b"XXXXrest of junk").is_err());
}

#[test]
fn manifest_stores_per_file_crc() {
    let files = sample_files();

    let (meta, _payload) = dir::pack(&files);
    let entries = dir::read(&meta).unwrap();

    for (entry, (_, contents)) in entries.iter().zip(files.iter()) {
        assert_eq!(entry.crc, Some(crc::crc32(contents)));
    }
}

#[test]
fn legacy_fdir_manifest_still_reads() {
    let mut meta = Vec::new();
    meta.extend_from_slice(b"FDIR");
    meta.push(2);
    meta.push(1);
    meta.extend_from_slice(b"a");
    meta.push(3);
    meta.push(2);
    meta.extend_from_slice(b"bb");
    meta.push(5);

    let entries = dir::read(&meta).unwrap();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].path, "a");
    assert_eq!(entries[0].offset, 0);
    assert_eq!(entries[0].len, 3);
    assert_eq!(entries[0].crc, None);

    assert_eq!(entries[1].path, "bb");
    assert_eq!(entries[1].offset, 3);
    assert_eq!(entries[1].len, 5);
    assert_eq!(entries[1].crc, None);
}

#[test]
fn handles_empty_files() {
    let files = vec![
        ("empty.txt".to_string(), Vec::new()),
        ("after.txt".to_string(), b"after\n".to_vec()),
    ];

    let (meta, payload) = dir::pack(&files);
    let entries = dir::read(&meta).unwrap();

    assert_eq!(entries.len(), 2);

    assert_eq!(entries[0].path, "empty.txt");
    assert_eq!(entries[0].offset, 0);
    assert_eq!(entries[0].len, 0);

    assert_eq!(entries[1].path, "after.txt");
    assert_eq!(entries[1].offset, 0);
    assert_eq!(entries[1].len, b"after\n".len());

    assert_eq!(payload, b"after\n");
}
