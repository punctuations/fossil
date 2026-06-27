use fossil::core::lossy::{compressed_format, quantize, quantize_content};

#[test]
fn clears_low_bits() {
    assert_eq!(quantize(&[0xFF, 0x01, 0xAB], 3), vec![0xF8, 0x00, 0xA8]);
}

#[test]
fn zero_bits_is_identity() {
    assert_eq!(quantize(&[1, 2, 3], 0), vec![1, 2, 3]);
}

#[test]
fn detects_compressed_formats() {
    assert_eq!(compressed_format(b"\x89PNG\r\n\x1a\n"), Some("PNG"));
    assert_eq!(compressed_format(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("JPEG"));
    assert_eq!(compressed_format(b"hello world"), None);
}

#[test]
fn ppm_header_is_preserved() {
    let mut ppm = b"P6\n2 1\n255\n".to_vec();
    ppm.extend_from_slice(&[0xFF, 0x01, 0x7F, 0x10, 0x20, 0x30]);
    let out = quantize_content(&ppm, 4);
    assert_eq!(&out[..11], b"P6\n2 1\n255\n"); // header intact
    assert_eq!(&out[11..], &[0xF0, 0x00, 0x70, 0x10, 0x20, 0x30]); // pixels quantized
}
