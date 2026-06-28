const TOP: u32 = 1 << 24;
const BOT: u32 = 1 << 16;
const MAX_TOTAL: u32 = BOT - 1;
const INCREMENT: u32 = 24;

pub struct Encoder {
    low: u32,
    range: u32,
    pub out: Vec<u8>,
}

impl Encoder {
    pub fn new() -> Self {
        Encoder {
            low: 0,
            range: 0xFFFF_FFFF,
            out: Vec::new(),
        }
    }

    pub fn encode(&mut self, cum: u32, freq: u32, total: u32) {
        self.range /= total;
        self.low = self.low.wrapping_add(cum * self.range);
        self.range *= freq;
        while (self.low ^ self.low.wrapping_add(self.range)) < TOP
            || (self.range < BOT && {
                self.range = self.low.wrapping_neg() & (BOT - 1);
                true
            })
        {
            self.out.push((self.low >> 24) as u8);
            self.low <<= 8;
            self.range <<= 8;
        }
    }

    pub fn finish(mut self) -> Vec<u8> {
        for _ in 0..4 {
            self.out.push((self.low >> 24) as u8);
            self.low <<= 8;
        }
        self.out
    }
}

pub struct Decoder<'a> {
    low: u32,
    range: u32,
    code: u32,
    data: &'a [u8],
    pos: usize,
}

impl<'a> Decoder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        let mut d = Decoder {
            low: 0,
            range: 0xFFFF_FFFF,
            code: 0,
            data,
            pos: 0,
        };
        for _ in 0..4 {
            let b = d.next();
            d.code = (d.code << 8) | b;
        }
        d
    }

    fn next(&mut self) -> u32 {
        let b = *self.data.get(self.pos).unwrap_or(&0) as u32;
        self.pos += 1;
        b
    }

    pub fn decode_freq(&mut self, total: u32) -> u32 {
        self.range /= total;
        (self.code.wrapping_sub(self.low) / self.range).min(total - 1)
    }

    pub fn update(&mut self, cum: u32, freq: u32) {
        self.low = self.low.wrapping_add(cum * self.range);
        self.range *= freq;
        while (self.low ^ self.low.wrapping_add(self.range)) < TOP
            || (self.range < BOT && {
                self.range = self.low.wrapping_neg() & (BOT - 1);
                true
            })
        {
            let b = self.next();
            self.code = (self.code << 8) | b;
            self.low <<= 8;
            self.range <<= 8;
        }
    }
}

pub struct Model {
    freq: [u32; 256],
    total: u32,
}

impl Model {
    pub fn new() -> Self {
        Model {
            freq: [1; 256],
            total: 256,
        }
    }

    pub fn cum_freq(&self, sym: usize) -> u32 {
        self.freq[..sym].iter().sum()
    }

    pub fn find(&self, value: u32) -> usize {
        let mut c = 0;
        for sym in 0..256 {
            if c + self.freq[sym] > value {
                return sym;
            }
            c += self.freq[sym];
        }
        255
    }

    pub fn total(&self) -> u32 {
        self.total
    }

    pub fn freq(&self, sym: usize) -> u32 {
        self.freq[sym]
    }

    pub fn update(&mut self, sym: usize) {
        self.freq[sym] += INCREMENT;
        self.total += INCREMENT;
        if self.total >= MAX_TOTAL {
            self.total = 0;
            for f in self.freq.iter_mut() {
                *f = (*f >> 1) | 1;
                self.total += *f;
            }
        }
    }
}

pub fn encode(bytes: &[u8]) -> Vec<u8> {
    let mut model = Model::new();
    let mut enc = Encoder::new();
    for &b in bytes {
        let sym = b as usize;
        enc.encode(model.cum_freq(sym), model.freq(sym), model.total());
        model.update(sym);
    }
    return enc.finish();
}

pub fn decode(data: &[u8], count: usize) -> Vec<u8> {
    let mut model = Model::new();
    let mut dec = Decoder::new(data);
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let value = dec.decode_freq(model.total());
        let sym = model.find(value);
        dec.update(model.cum_freq(sym), model.freq(sym));
        out.push(sym as u8);
        model.update(sym);
    }
    return out;
}
