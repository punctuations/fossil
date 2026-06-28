use super::range::{Decoder, Encoder, Model};

pub fn encode(bytes: &[u8]) -> Vec<u8> {
    let mut models: Vec<Model> = (0..256).map(|_| Model::new()).collect();
    let mut enc = Encoder::new();
    let mut ctx = 0usize;

    for &b in bytes {
        let sym = b as usize;
        let m = &models[ctx];
        enc.encode(m.cum_freq(sym), m.freq(sym), m.total());
        models[ctx].update(sym);
        ctx = sym;
    }

    return enc.finish();
}

pub fn decode(data: &[u8], count: usize) -> Vec<u8> {
    let mut models: Vec<Model> = (0..256).map(|_| Model::new()).collect();
    let mut dec = Decoder::new(data);
    let mut out = Vec::with_capacity(count);
    let mut ctx = 0usize;

    for _ in 0..count {
        let value = dec.decode_freq(models[ctx].total());
        let sym = models[ctx].find(value);
        dec.update(models[ctx].cum_freq(sym), models[ctx].freq(sym));
        models[ctx].update(sym);
        out.push(sym as u8);
        ctx = sym;
    }

    return out;
}
