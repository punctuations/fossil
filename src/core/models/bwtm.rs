use super::range::{Decoder, Encoder, Model};
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

fn encode_run(enc: &mut Encoder, model: &mut Model, mut v: usize) {
    loop {
        let mut byte = v & 0x7f;
        if v >= 0x80 {
            byte |= 0x80;
        }
        enc.encode(model.cum_freq(byte), model.freq(byte), model.total());
        model.update(byte);
        if v < 0x80 {
            break;
        }
        v >>= 7;
    }
}

const MAX_RUN_CHUNKS: usize = usize::BITS.div_ceil(7) as usize;

fn decode_run(dec: &mut Decoder, model: &mut Model) -> usize {
    let mut v = 0usize;
    let mut shift = 0;
    for _ in 0..MAX_RUN_CHUNKS {
        let value = dec.decode_freq(model.total());
        let byte = model.find(value);
        dec.update(model.cum_freq(byte), model.freq(byte));
        model.update(byte);
        v |= (byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    v
}

pub fn encode2(bytes: &[u8]) -> Vec<u8> {
    let (last, primary) = bwt::forward(bytes);
    let moved = mtf::forward(&last);

    let mut enc = Encoder::new();
    let mut sym_model = Model::new();
    let mut run_model = Model::new();

    let mut i = 0;
    while i < moved.len() {
        let sym = moved[i] as usize;
        enc.encode(
            sym_model.cum_freq(sym),
            sym_model.freq(sym),
            sym_model.total(),
        );
        sym_model.update(sym);

        if sym == 0 {
            let mut run = 1;
            while i + run < moved.len() && moved[i + run] == 0 {
                run += 1;
            }
            encode_run(&mut enc, &mut run_model, run - 1);
            i += run;
        } else {
            i += 1;
        }
    }

    let mut out = Vec::new();
    varint::write(&mut out, primary);
    out.extend_from_slice(&enc.finish());
    return out;
}

pub fn decode2(data: &[u8], orig_len: usize) -> Vec<u8> {
    let mut pos = 0;
    let primary = varint::read(data, &mut pos);

    let mut dec = Decoder::new(&data[pos..]);
    let mut sym_model = Model::new();
    let mut run_model = Model::new();

    let mut moved = Vec::with_capacity(orig_len);
    while moved.len() < orig_len {
        let value = dec.decode_freq(sym_model.total());
        let sym = sym_model.find(value);
        dec.update(sym_model.cum_freq(sym), sym_model.freq(sym));
        sym_model.update(sym);

        if sym == 0 {
            let run = decode_run(&mut dec, &mut run_model) + 1;
            for _ in 0..run.min(orig_len - moved.len()) {
                moved.push(0);
            }
        } else {
            moved.push(sym as u8);
        }
    }

    let last = mtf::inverse(&moved);
    return bwt::inverse(&last, primary);
}

pub fn decode(data: &[u8], orig_len: usize) -> Vec<u8> {
    let mut pos = 0;
    let primary = varint::read(data, &mut pos);
    let moved = range::decode(&data[pos..], orig_len);
    let last = mtf::inverse(&moved);
    return bwt::inverse(&last, primary);
}
