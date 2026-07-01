use fossil::core::container;
use fossil::core::models::signal;

fn s16(v: i32) -> [u8; 2] {
    let u = (v & 0xFFFF) as u16;
    [(u & 0xFF) as u8, (u >> 8) as u8]
}

fn sine_mono(n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let v = (11000.0 * (i as f64 * 0.05).sin() + 4000.0 * (i as f64 * 0.013).sin()) as i32;
        out.extend_from_slice(&s16(v));
    }
    out
}

fn sine_stereo(n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n * 4);
    for i in 0..n {
        let l = (9000.0 * (i as f64 * 0.03).sin()) as i32;
        let r = (9000.0 * (i as f64 * 0.03 + 0.5).sin()) as i32;
        out.extend_from_slice(&s16(l));
        out.extend_from_slice(&s16(r));
    }
    out
}

#[test]
fn round_trips_and_shrinks_mono() {
    let data = sine_mono(4000);
    let enc = signal::encode(&data);
    assert!(enc.len() < data.len());
    assert_eq!(signal::decode(&enc, data.len()), data);
}

#[test]
fn round_trips_and_shrinks_stereo() {
    let data = sine_stereo(4000);
    let enc = signal::encode(&data);
    assert!(enc.len() < data.len());
    assert_eq!(signal::decode(&enc, data.len()), data);
}

#[test]
fn round_trips_odd_length() {
    let mut data = sine_mono(2000);
    data.push(0x7a);
    let enc = signal::encode(&data);
    assert_eq!(signal::decode(&enc, data.len()), data);
}

#[test]
fn round_trips_random() {
    let data: Vec<u8> = (0u64..3000)
        .map(|i| {
            let mut x = i.wrapping_mul(0x9E3779B97F4A7C15);
            x ^= x >> 29;
            (x >> 24) as u8
        })
        .collect();
    let enc = signal::encode(&data);
    assert_eq!(signal::decode(&enc, data.len()), data);
}

#[test]
fn round_trips_empty_and_tiny() {
    for len in 0..6 {
        let data: Vec<u8> = (0..len).map(|i| i as u8).collect();
        let enc = signal::encode(&data);
        assert_eq!(signal::decode(&enc, data.len()), data);
    }
}

#[test]
fn container_round_trips_pcm() {
    let data = sine_mono(20000);
    let packed = container::write(&data, "pcm");
    let c = container::read(&packed).unwrap();
    assert_eq!(c.decode(), data);
    assert!(packed.len() < data.len());
}
