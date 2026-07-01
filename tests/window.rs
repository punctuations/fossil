use fossil::core::container::{self, BLOCK_SIZE};

fn pseudo(n: usize) -> Vec<u8> {
    (0..n as u64)
        .map(|i| {
            let mut x = i.wrapping_mul(0x9E3779B97F4A7C15);
            x ^= x >> 29;
            (x >> 24) as u8
        })
        .collect()
}

#[test]
fn repeat_across_many_blocks_round_trips() {
    let pattern = pseudo(3000);
    let mut data = Vec::new();
    data.extend_from_slice(&pattern);
    data.extend(std::iter::repeat_n(0u8, 5 * BLOCK_SIZE));
    data.extend_from_slice(&pattern);

    let packed = container::write(&data, "bin");
    let c = container::read(&packed).unwrap();
    assert_eq!(c.decode(), data);
}

#[test]
fn far_repeat_becomes_a_reference_not_a_copy() {
    let pattern = pseudo(3000);
    let mut data = Vec::new();
    data.extend_from_slice(&pattern);
    data.extend(std::iter::repeat_n(0u8, 5 * BLOCK_SIZE));
    data.extend_from_slice(&pattern);

    let packed = container::write(&data, "bin");

    assert!(
        packed.len() < pattern.len() + 800,
        "second copy {} blocks away should be a cross-block match, got {} bytes",
        5,
        packed.len()
    );
}
