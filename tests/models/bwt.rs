use fossil::core::models::bwt::{forward, inverse};

fn roundtrip(data: &[u8]) {
    let (last, primary) = forward(data);
    assert_eq!(inverse(&last, primary), data);
}

// brute-force reference: sort all cyclic rotations directly
fn reference(data: &[u8]) -> (Vec<u8>, usize) {
    let n = data.len();
    if n == 0 {
        return (Vec::new(), 0);
    }
    let mut rot: Vec<usize> = (0..n).collect();
    rot.sort_by(|&a, &b| {
        for i in 0..n {
            let (x, y) = (data[(a + i) % n], data[(b + i) % n]);
            if x != y {
                return x.cmp(&y);
            }
        }
        a.cmp(&b)
    });
    let last: Vec<u8> = rot.iter().map(|&r| data[(r + n - 1) % n]).collect();
    let primary = rot.iter().position(|&r| r == 0).unwrap();
    (last, primary)
}

fn check(data: &[u8]) {
    let (last, primary) = forward(data);
    assert_eq!(
        inverse(&last, primary),
        data,
        "round-trip failed, len {}",
        data.len()
    );
    assert_eq!(
        (last, primary),
        reference(data),
        "suffix array differs from reference, len {}",
        data.len()
    );
}

fn lcg(seed: &mut u64) -> u8 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*seed >> 33) as u8
}

#[test]
fn roundtrips_banana() {
    roundtrip(b"banana");
}

#[test]
fn roundtrips_text() {
    roundtrip(b"the quick brown fox jumps over the lazy dog");
}

#[test]
fn roundtrips_empty() {
    roundtrip(&[]);
}

#[test]
fn roundtrips_repeats() {
    roundtrip(&[0x42u8; 500]);
}

#[test]
fn roundtrips_all_bytes() {
    let data: Vec<u8> = (0..=255).collect();
    roundtrip(&data);
}

#[test]
fn clusters_similar_context() {
    // BWT of repetitive text should produce long same-byte runs
    let (last, _) = forward(&b"abracadabra".repeat(20));
    let runs = last.windows(2).filter(|w| w[0] == w[1]).count();
    assert!(runs > 100);
}

#[test]
fn small_and_boundary_sizes() {
    // exercises n < 256 (where initial ranks exceed n), n around 256, and up
    for n in 0..300 {
        check(&vec![0x41u8; n]); // fully periodic
        let ramp: Vec<u8> = (0..n).map(|i| i as u8).collect();
        check(&ramp);
    }
}

#[test]
fn periodic_patterns() {
    for period in ["ab", "abc", "abcd", "aab", "banana", "mississippi"] {
        for reps in [1usize, 2, 3, 7, 40] {
            check(&period.as_bytes().repeat(reps));
        }
    }
}

#[test]
fn random_many_sizes() {
    let mut seed = 0x1234_5678_9abc_def0u64;
    for &n in &[1usize, 2, 3, 4, 8, 100, 255, 256, 257, 511, 1000, 4095, 4096, 4097] {
        for _ in 0..8 {
            let data: Vec<u8> = (0..n).map(|_| lcg(&mut seed)).collect();
            check(&data);
        }
    }
}

#[test]
fn low_alphabet_random() {
    // few distinct symbols -> lots of equal-rank ties, the fragile case
    let mut seed = 99u64;
    for _ in 0..40 {
        let data: Vec<u8> = (0..2000).map(|_| lcg(&mut seed) % 3).collect();
        check(&data);
    }
}
