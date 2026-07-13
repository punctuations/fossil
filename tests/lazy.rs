use fossil::core::{container, dir};

fn sample_files() -> Vec<(String, Vec<u8>)> {
    let mut a = Vec::new();
    for i in 0..40000u32 {
        a.extend_from_slice(format!("line {i} the quick brown fox jumps\n").as_bytes());
    }

    let mut b = Vec::new();
    for i in 0..20000u32 {
        b.extend_from_slice(format!("{},{},{}\n", i, i.wrapping_mul(2), i % 7).as_bytes());
    }

    let c: Vec<u8> = (0..50000u32)
        .map(|i| (i.wrapping_mul(2654435761) >> 24) as u8)
        .collect();

    vec![
        ("a.txt".to_string(), a),
        ("b.csv".to_string(), b),
        ("c.bin".to_string(), c),
    ]
}

fn build() -> (Vec<u8>, Vec<(String, Vec<u8>)>) {
    let files = sample_files();
    let (meta, payload) = dir::pack(&files);
    let bytes = container::write_progress_meta(&payload, "/", &meta, None, false);
    (bytes, files)
}

#[test]
fn lazy_take_matches_each_file() {
    let (bytes, files) = build();

    let mut lazy = container::read_lazy(&bytes).unwrap();
    assert!(
        lazy.blocks.len() > 64,
        "test corpus should span several segments, got {} blocks",
        lazy.blocks.len()
    );

    let entries = dir::read(&lazy.meta).unwrap();
    for (i, e) in entries.iter().enumerate() {
        let got = lazy.read_range(e.offset, e.len).unwrap();
        assert_eq!(got, files[i].1, "file {} mismatch", e.path);
    }
}

#[test]
fn lazy_read_range_matches_full_decode() {
    let (bytes, _) = build();
    let full = container::read(&bytes).unwrap().decode();
    let mut lazy = container::read_lazy(&bytes).unwrap();

    let spots = [
        0usize,
        1,
        4095,
        4096,
        4097,
        262143,
        262144,
        262145,
        300000,
        full.len() - 1,
    ];
    let lens = [1usize, 2, 5000, 262146];

    for &off in &spots {
        for &len in &lens {
            if off + len <= full.len() {
                let got = lazy.read_range(off, len).unwrap();
                assert_eq!(got, &full[off..off + len], "range off={off} len={len}");
            }
        }
    }
}

#[test]
fn lazy_take_works_on_stored_directory_fossil() {
    let mut state = 0x9e3779b97f4a7c15u64;
    let mut noise = |n: usize| -> Vec<u8> {
        (0..n)
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (state >> 33) as u8
            })
            .collect()
    };

    let files = vec![
        ("a.bin".to_string(), noise(20000)),
        ("b.bin".to_string(), noise(20000)),
        ("c.bin".to_string(), noise(5)),
    ];

    let (meta, payload) = dir::pack(&files);
    let bytes = container::write_progress_meta(&payload, "/", &meta, None, false);
    assert_eq!(bytes[5], 1, "test corpus should force a stored container");

    let mut lazy = container::read_lazy(&bytes).unwrap();
    let entries = dir::read(&lazy.meta).unwrap();
    for (i, e) in entries.iter().enumerate() {
        let got = lazy.read_range(e.offset, e.len).unwrap();
        assert_eq!(got, files[i].1, "file {} mismatch", e.path);
    }
}

#[test]
fn lazy_read_range_repeated_reads_are_stable() {
    let (bytes, _) = build();
    let full = container::read(&bytes).unwrap().decode();
    let mut lazy = container::read_lazy(&bytes).unwrap();

    for off in [262144usize, 4096, 262144, 300000, 4096] {
        let got = lazy.read_range(off, 8192).unwrap();
        assert_eq!(got, &full[off..off + 8192]);
    }
}
