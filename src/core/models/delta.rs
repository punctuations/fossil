use super::range;

fn forward(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut prev = 0u8;
    for &b in bytes {
        out.push(b.wrapping_sub(prev));
        prev = b;
    }

    return out;
}

fn inverse(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut prev = 0u8;
    for &d in data {
        let b = d.wrapping_add(prev);
        out.push(b);
        prev = b;
    }

    return out;
}

pub fn encode(bytes: &[u8]) -> Vec<u8> {
    return range::encode(&forward(bytes));
}

pub fn decode(data: &[u8], orig_len: usize) -> Vec<u8> {
    return inverse(&range::decode(data, orig_len));
}
