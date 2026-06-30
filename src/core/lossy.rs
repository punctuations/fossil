pub fn compressed_format(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG") {
        return Some("PNG");
    }
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("JPEG");
    }
    if data.starts_with(b"GIF8") {
        return Some("GIF");
    }
    if data.starts_with(b"PK\x03\x04") {
        return Some("ZIP");
    }
    if data.starts_with(&[0x1F, 0x8B]) {
        return Some("gzip");
    }
    if data.starts_with(b"FOSL") {
        return Some("fossil");
    }
    return None;
}

pub fn raw_image_format(data: &[u8]) -> Option<&'static str> {
    // the raw image formats quantize_content knows how to keep valid
    if data.starts_with(b"P6") || data.starts_with(b"P5") {
        return Some("PPM");
    }
    return None;
}

fn ppm_header_len(data: &[u8]) -> Option<usize> {
    if !(data.starts_with(b"P6") || data.starts_with(b"P5")) {
        return None;
    }

    let mut pos = 2;
    let mut tokens = 0;
    while pos < data.len() && tokens < 3 {
        while pos < data.len() {
            let c = data[pos];
            if c == b'#' {
                while pos < data.len() && data[pos] != b'\n' {
                    pos += 1;
                }
            } else if c.is_ascii_whitespace() {
                pos += 1;
            } else {
                break;
            }
        }
        if pos < data.len() && data[pos].is_ascii_digit() {
            while pos < data.len() && data[pos].is_ascii_digit() {
                pos += 1;
            }
            tokens += 1;
        } else {
            break;
        }
    }

    if tokens < 3 {
        return None;
    }
    if pos < data.len() && data[pos].is_ascii_whitespace() {
        pos += 1;
    }
    return Some(pos);
}

pub fn quantize_content(data: &[u8], bits: u8) -> Vec<u8> {
    if let Some(h) = ppm_header_len(data) {
        let mut out = data[..h].to_vec();
        out.extend(quantize(&data[h..], bits));
        return out;
    }
    return quantize(data, bits);
}

pub fn quantize(data: &[u8], bits: u8) -> Vec<u8> {
    let mask = if bits >= 8 { 0u8 } else { 0xFFu8 << bits };
    return data.iter().map(|&b| b & mask).collect();
}
