use super::models::{bwtm, delta, generator, huffman, lz, lzh, ppm, range, rle, transpose, word};

pub const RAW: u8 = 0;
pub const RLE: u8 = 1;
pub const ENTROPY: u8 = 2;
pub const LZ: u8 = 3;
pub const LZH: u8 = 4;
pub const BWTM: u8 = 5;
pub const RANGE: u8 = 6;
pub const PPM: u8 = 7;
pub const GEN: u8 = 8;
pub const DELTA: u8 = 9;
pub const CSVT: u8 = 10;
pub const WORD: u8 = 11;

pub fn model_name(model: u8) -> &'static str {
    match model {
        RLE => "RLE",
        ENTROPY => "ENTROPY",
        LZ => "LZ",
        LZH => "LZH",
        BWTM => "BWTM",
        RANGE => "RANGE",
        PPM => "PPM",
        GEN => "GEN",
        DELTA => "DELTA",
        CSVT => "CSV",
        WORD => "WORD",
        _ => "RAW",
    }
}

pub fn encode_block(bytes: &[u8]) -> (u8, Vec<u8>) {
    let mut model = RAW;
    let mut best = bytes.to_vec();

    let rle = rle::encode(bytes);
    if rle.len() < best.len() {
        model = RLE;
        best = rle;
    }

    let range = range::encode(bytes);
    if range.len() < best.len() {
        model = RANGE;
        best = range;
    }

    let huff = huffman::encode(bytes);
    if huff.len() < best.len() {
        model = ENTROPY;
        best = huff;
    }

    let lz = lz::encode(bytes);
    if lz.len() < best.len() {
        model = LZ;
        best = lz;
    }

    let lzh = lzh::encode(bytes);
    if lzh.len() < best.len() {
        model = LZH;
        best = lzh;
    }

    let bwtm = bwtm::encode(bytes);
    if bwtm.len() < best.len() {
        model = BWTM;
        best = bwtm;
    }

    let ppm = ppm::encode(bytes);
    if ppm.len() < best.len() {
        model = PPM;
        best = ppm;
    }

    let generator = generator::encode(bytes);
    if generator.len() < best.len() {
        model = GEN;
        best = generator;
    }

    let delta = delta::encode(bytes);
    if delta.len() < best.len() {
        model = DELTA;
        best = delta;
    }

    let transpose = transpose::encode(bytes);
    if transpose.len() < best.len() {
        model = CSVT;
        best = transpose;
    }

    let word = word::encode(bytes);
    if word.len() < best.len() {
        model = WORD;
        best = word;
    }

    return (model, best);
}

pub fn decode_block(model: u8, payload: &[u8], orig_len: usize) -> Vec<u8> {
    match model {
        RLE => rle::decode(payload),
        ENTROPY => huffman::decode(payload, orig_len),
        LZ => lz::decode(payload, orig_len),
        LZH => lzh::decode(payload, orig_len),
        BWTM => bwtm::decode(payload, orig_len),
        RANGE => range::decode(payload, orig_len),
        PPM => ppm::decode(payload, orig_len),
        GEN => generator::decode(payload),
        DELTA => delta::decode(payload, orig_len),
        CSVT => transpose::decode(payload, orig_len),
        WORD => word::decode(payload, orig_len),
        _ => payload.to_vec(),
    }
}
