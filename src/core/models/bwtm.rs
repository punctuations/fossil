use super::{bwt, mtf, range};
use crate::core::varint;

pub fn encode(bytes: &[u8]) -> Vec<u8> {
    let (last, primary) = bwt::forward(bytes);
    let moved = mtf::forward(&last);

    let mut out = Vec::new();
    varint::write(&mut out, primary);
    out.extend_from_slice(&range::encode(&moved));
    return out;
}

pub fn decode(data: &[u8], orig_len: usize) -> Vec<u8> {
    let mut pos = 0;
    let primary = varint::read(data, &mut pos);
    let moved = range::decode(&data[pos..], orig_len);
    let last = mtf::inverse(&moved);
    return bwt::inverse(&last, primary);
}
