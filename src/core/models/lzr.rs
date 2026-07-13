use super::lz;
use super::range::{Decoder, Encoder, Model};
use crate::core::biglz::{self, Token};

struct Bit {
    c0: u32,
    c1: u32,
}

impl Bit {
    fn new() -> Self {
        Bit { c0: 1, c1: 1 }
    }

    fn total(&self) -> u32 {
        self.c0 + self.c1
    }

    fn bump(&mut self, bit: u8) {
        if bit == 0 {
            self.c0 += 24;
        } else {
            self.c1 += 24;
        }
        if self.total() >= 60000 {
            self.c0 = (self.c0 >> 1) | 1;
            self.c1 = (self.c1 >> 1) | 1;
        }
    }

    fn encode(&mut self, enc: &mut Encoder, bit: u8) {
        if bit == 0 {
            enc.encode(0, self.c0, self.total());
        } else {
            enc.encode(self.c0, self.c1, self.total());
        }
        self.bump(bit);
    }

    fn decode(&mut self, dec: &mut Decoder) -> u8 {
        let total = self.total();
        let v = dec.decode_freq(total);
        let bit = if v < self.c0 { 0 } else { 1 };
        if bit == 0 {
            dec.update(0, self.c0);
        } else {
            dec.update(self.c0, self.c1);
        }
        self.bump(bit);
        bit
    }
}

fn encode_value(enc: &mut Encoder, model: &mut Model, mut v: usize) {
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

fn decode_value(dec: &mut Decoder, model: &mut Model) -> usize {
    let mut v = 0usize;
    let mut shift = 0;
    loop {
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

pub fn encode(bytes: &[u8]) -> Vec<u8> {
    encode_from(bytes, 0)
}

pub fn encode_from(bytes: &[u8], emit_start: usize) -> Vec<u8> {
    encode_tokens(bytes, emit_start, &biglz::tokens(bytes, emit_start))
}

pub fn encode_tokens(bytes: &[u8], emit_start: usize, tokens: &[Token]) -> Vec<u8> {
    let mut enc = Encoder::new();
    let mut flag = [Bit::new(), Bit::new()];
    let mut lits: Vec<Model> = (0..256).map(|_| Model::new()).collect();
    let mut len_model = Model::new();
    let mut dist_model = Model::new();

    let mut pos = emit_start;
    let mut prev_match = 0usize;

    for t in tokens {
        match t {
            Token::Lit(b) => {
                flag[prev_match].encode(&mut enc, 0);
                let ctx = if pos > 0 { bytes[pos - 1] as usize } else { 0 };
                let sym = *b as usize;
                let m = &lits[ctx];
                enc.encode(m.cum_freq(sym), m.freq(sym), m.total());
                lits[ctx].update(sym);
                pos += 1;
                prev_match = 0;
            }
            Token::Match { dist, len } => {
                flag[prev_match].encode(&mut enc, 1);
                encode_value(&mut enc, &mut len_model, len - biglz::MIN_MATCH);
                encode_value(&mut enc, &mut dist_model, *dist);
                pos += len;
                prev_match = 1;
            }
        }
    }

    enc.finish()
}

fn encode_value_ctx(enc: &mut Encoder, models: &mut [Model], mut v: usize) {
    let mut idx = 0;
    loop {
        let mut byte = v & 0x7f;
        if v >= 0x80 {
            byte |= 0x80;
        }
        let m = &mut models[idx.min(models.len() - 1)];
        enc.encode(m.cum_freq(byte), m.freq(byte), m.total());
        m.update(byte);
        if v < 0x80 {
            break;
        }
        v >>= 7;
        idx += 1;
    }
}

fn decode_value_ctx(dec: &mut Decoder, models: &mut [Model]) -> usize {
    let mut v = 0usize;
    let mut shift = 0;
    let mut idx = 0;
    loop {
        let m = &mut models[idx.min(models.len() - 1)];
        let value = dec.decode_freq(m.total());
        let byte = m.find(value);
        dec.update(m.cum_freq(byte), m.freq(byte));
        m.update(byte);
        v |= (byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        idx += 1;
    }
    v
}

pub fn encode_tokens2(bytes: &[u8], emit_start: usize, tokens: &[Token]) -> Vec<u8> {
    let mut enc = Encoder::new();
    let mut flag = [Bit::new(), Bit::new()];
    let mut rep = Bit::new();
    let mut lits: Vec<Model> = (0..256).map(|_| Model::new()).collect();
    let mut len_models: Vec<Model> = (0..2).map(|_| Model::new()).collect();
    let mut dist_models: Vec<Model> = (0..3).map(|_| Model::new()).collect();

    let mut pos = emit_start;
    let mut prev_match = 0usize;
    let mut last_dist = 0usize;

    for t in tokens {
        match t {
            Token::Lit(b) => {
                flag[prev_match].encode(&mut enc, 0);
                let ctx = if pos > 0 { bytes[pos - 1] as usize } else { 0 };
                let sym = *b as usize;
                let m = &lits[ctx];
                enc.encode(m.cum_freq(sym), m.freq(sym), m.total());
                lits[ctx].update(sym);
                pos += 1;
                prev_match = 0;
            }
            Token::Match { dist, len } => {
                flag[prev_match].encode(&mut enc, 1);
                if last_dist != 0 {
                    if *dist == last_dist {
                        rep.encode(&mut enc, 1);
                        encode_value_ctx(&mut enc, &mut len_models, len - biglz::MIN_MATCH);
                        pos += len;
                        prev_match = 1;
                        continue;
                    }
                    rep.encode(&mut enc, 0);
                }
                encode_value_ctx(&mut enc, &mut len_models, len - biglz::MIN_MATCH);
                encode_value_ctx(&mut enc, &mut dist_models, *dist);
                last_dist = *dist;
                pos += len;
                prev_match = 1;
            }
        }
    }

    enc.finish()
}

pub fn decode_windowed2(data: &[u8], count: usize, history: &[u8]) -> Vec<u8> {
    let seed = history.len().min(lz::HISTORY);
    let mut out = Vec::with_capacity(seed + count);
    out.extend_from_slice(&history[history.len() - seed..]);

    let target = seed + count;
    let mut dec = Decoder::new(data);
    let mut flag = [Bit::new(), Bit::new()];
    let mut rep = Bit::new();
    let mut lits: Vec<Model> = (0..256).map(|_| Model::new()).collect();
    let mut len_models: Vec<Model> = (0..2).map(|_| Model::new()).collect();
    let mut dist_models: Vec<Model> = (0..3).map(|_| Model::new()).collect();
    let mut prev_match = 0usize;
    let mut last_dist = 0usize;

    while out.len() < target {
        let is_match = flag[prev_match].decode(&mut dec);
        if is_match == 0 {
            let ctx = if out.is_empty() {
                0
            } else {
                out[out.len() - 1] as usize
            };
            let m = &lits[ctx];
            let value = dec.decode_freq(m.total());
            let sym = m.find(value);
            dec.update(m.cum_freq(sym), m.freq(sym));
            lits[ctx].update(sym);
            out.push(sym as u8);
            prev_match = 0;
        } else {
            let dist = if last_dist != 0 && rep.decode(&mut dec) == 1 {
                last_dist
            } else {
                let len = decode_value_ctx(&mut dec, &mut len_models) + biglz::MIN_MATCH;
                let dist = decode_value_ctx(&mut dec, &mut dist_models);
                last_dist = dist;
                if dist == 0 || dist > out.len() {
                    break;
                }
                let start = out.len() - dist;
                for x in 0..len {
                    let b = out[start + x];
                    out.push(b);
                }
                prev_match = 1;
                continue;
            };

            let len = decode_value_ctx(&mut dec, &mut len_models) + biglz::MIN_MATCH;
            if dist == 0 || dist > out.len() {
                break;
            }
            let start = out.len() - dist;
            for x in 0..len {
                let b = out[start + x];
                out.push(b);
            }
            prev_match = 1;
        }
    }

    out.split_off(seed)
}

pub fn decode(data: &[u8], count: usize) -> Vec<u8> {
    decode_windowed(data, count, &[])
}

pub fn decode_windowed(data: &[u8], count: usize, history: &[u8]) -> Vec<u8> {
    let seed = history.len().min(lz::HISTORY);
    let mut out = Vec::with_capacity(seed + count);
    out.extend_from_slice(&history[history.len() - seed..]);

    let target = seed + count;
    let mut dec = Decoder::new(data);
    let mut flag = [Bit::new(), Bit::new()];
    let mut lits: Vec<Model> = (0..256).map(|_| Model::new()).collect();
    let mut len_model = Model::new();
    let mut dist_model = Model::new();
    let mut prev_match = 0usize;

    while out.len() < target {
        let is_match = flag[prev_match].decode(&mut dec);
        if is_match == 0 {
            let ctx = if out.is_empty() {
                0
            } else {
                out[out.len() - 1] as usize
            };
            let m = &lits[ctx];
            let value = dec.decode_freq(m.total());
            let sym = m.find(value);
            dec.update(m.cum_freq(sym), m.freq(sym));
            lits[ctx].update(sym);
            out.push(sym as u8);
            prev_match = 0;
        } else {
            let len = decode_value(&mut dec, &mut len_model) + biglz::MIN_MATCH;
            let dist = decode_value(&mut dec, &mut dist_model);
            if dist == 0 || dist > out.len() {
                break;
            }
            let start = out.len() - dist;
            for x in 0..len {
                let b = out[start + x];
                out.push(b);
            }
            prev_match = 1;
        }
    }

    out.split_off(seed)
}
