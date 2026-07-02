use std::sync::atomic::Ordering;

use fossil::core::container::{self, Progress};

#[test]
fn progress_counts_every_block() {
    let data = vec![7u8; 4096 * 5 + 100];
    let p = Progress::default();
    let _ = container::write_progress(&data, "bin", Some(&p), false);

    let total = p.total.load(Ordering::Relaxed);
    let done = p.done.load(Ordering::Relaxed);

    assert_eq!(total, 6);
    assert_eq!(done, total);
}

#[test]
fn write_matches_write_progress() {
    let data = vec![42u8; 20000];
    assert_eq!(
        container::write(&data, "bin"),
        container::write_progress(&data, "bin", None, false)
    );
}
