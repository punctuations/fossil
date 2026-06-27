use super::{huffman, lz};
use crate::core::varint;

pub fn encode(bytes: &[u8]) -> Vec<u8> {
    let stream = lz::encode(bytes);
    let mut out = Vec::new();
    varint::write(&mut out, stream.len());
    out.extend_from_slice(&huffman::encode(&stream));
    return out;
}

pub fn decode(data: &[u8], orig_len: usize) -> Vec<u8> {
    let mut pos = 0;
    let stream_len = varint::read(data, &mut pos);
    let stream = huffman::decode(&data[pos..], stream_len);
    return lz::decode(&stream, orig_len);
}
