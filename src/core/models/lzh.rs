use super::{huffman, lz};
use crate::core::{biglz, varint};

pub fn encode(bytes: &[u8]) -> Vec<u8> {
    encode_from(bytes, 0)
}

pub fn encode_from(bytes: &[u8], emit_start: usize) -> Vec<u8> {
    let stream = biglz::encode_from(bytes, emit_start);
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

pub fn decode_windowed(data: &[u8], orig_len: usize, history: &[u8]) -> Vec<u8> {
    let mut pos = 0;
    let stream_len = varint::read(data, &mut pos);
    let stream = huffman::decode(&data[pos..], stream_len);
    return lz::decode_windowed(&stream, orig_len, history);
}
