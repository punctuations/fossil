use super::biglz;
use super::models::{
    bwtm, delta, generator, huffman, lz, lzh, lzr, ppm, range, rle, signal, transpose, word,
};

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
pub const LZR: u8 = 12;
pub const SIGNAL: u8 = 13;

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
        LZR => "LZR",
        SIGNAL => "SIGNAL",
        _ => "RAW",
    }
}

const FAST_CHAIN: usize = 8;

pub fn encode_block(input: &[u8], start: usize, end: usize, fast: bool) -> (u8, Vec<u8>) {
    let bytes = &input[start..end];
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

    let wstart = start.saturating_sub(lz::HISTORY);
    let combined = &input[wstart..end];
    let emit = start - wstart;

    let tokens = if fast {
        biglz::tokens_chain(combined, emit, FAST_CHAIN)
    } else {
        biglz::tokens(combined, emit)
    };
    let lz_stream = biglz::serialize(&tokens);
    if lz_stream.len() < best.len() {
        model = LZ;
        best = lz_stream.clone();
    }

    let lzh_enc = lzh::pack(&lz_stream);
    if lzh_enc.len() < best.len() {
        model = LZH;
        best = lzh_enc;
    }

    let lzr_enc = lzr::encode_tokens(combined, emit, &tokens);
    if lzr_enc.len() < best.len() {
        model = LZR;
        best = lzr_enc;
    }

    if !fast {
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

    if !fast && best.len() * 4 > bytes.len() * 3 {
        let signal = signal::encode(bytes);
        if signal.len() < best.len() {
            model = SIGNAL;
            best = signal;
        }
    }

    return (model, best);
}

pub fn decode_block(model: u8, payload: &[u8], orig_len: usize, history: &[u8]) -> Vec<u8> {
    match model {
        RLE => rle::decode(payload),
        ENTROPY => huffman::decode(payload, orig_len),
        LZ => lz::decode_windowed(payload, orig_len, history),
        LZH => lzh::decode_windowed(payload, orig_len, history),
        LZR => lzr::decode_windowed(payload, orig_len, history),
        BWTM => bwtm::decode(payload, orig_len),
        RANGE => range::decode(payload, orig_len),
        PPM => ppm::decode(payload, orig_len),
        GEN => generator::decode(payload),
        DELTA => delta::decode(payload, orig_len),
        CSVT => transpose::decode(payload, orig_len),
        WORD => word::decode(payload, orig_len),
        SIGNAL => signal::decode(payload, orig_len),
        _ => payload.to_vec(),
    }
}
