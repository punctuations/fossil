const TOP: u32 = 1 << 24;
const BOT: u32 = 1 << 16;
const MAX_TOTAL: u32 = BOT - 1;
const INCREMENT: u32 = 24;

struct Model {
    freq: [u32; 256],
    total: u32,
}

impl Model {
    fn new() -> Self {
        Model {
            freq: [1; 256],
            total: 256,
        }
    }

    fn cum_freq(&self, sym: usize) -> u32 {
        self.freq[..sym].iter().sum()
    }

    fn find(&self, value: u32) -> usize {
        let mut c = 0;
        for sym in 0..256 {
            if c + self.freq[sym] > value {
                return sym;
            }
            c += self.freq[sym];
        }
        255
    }

    fn update(&mut self, sym: usize) {
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
    let mut low: u32 = 0;
    let mut range: u32 = 0xFFFF_FFFF;
    let mut out = Vec::new();

    for &b in bytes {
        let sym = b as usize;
        let cum = model.cum_freq(sym);
        let freq = model.freq[sym];

        range /= model.total;
        low = low.wrapping_add(cum * range);
        range *= freq;

        while (low ^ low.wrapping_add(range)) < TOP
            || (range < BOT && {
                range = low.wrapping_neg() & (BOT - 1);
                true
            })
        {
            out.push((low >> 24) as u8);
            low <<= 8;
            range <<= 8;
        }

        model.update(sym);
    }

    for _ in 0..4 {
        out.push((low >> 24) as u8);
        low <<= 8;
    }

    return out;
}

pub fn decode(data: &[u8], count: usize) -> Vec<u8> {
    let mut model = Model::new();
    let mut low: u32 = 0;
    let mut range: u32 = 0xFFFF_FFFF;
    let mut code: u32 = 0;
    let mut pos = 0;

    for _ in 0..4 {
        code = (code << 8) | *data.get(pos).unwrap_or(&0) as u32;
        pos += 1;
    }

    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        range /= model.total;
        let value = (code.wrapping_sub(low) / range).min(model.total - 1);
        let sym = model.find(value);

        let cum = model.cum_freq(sym);
        let freq = model.freq[sym];
        low = low.wrapping_add(cum * range);
        range *= freq;

        while (low ^ low.wrapping_add(range)) < TOP
            || (range < BOT && {
                range = low.wrapping_neg() & (BOT - 1);
                true
            })
        {
            code = (code << 8) | *data.get(pos).unwrap_or(&0) as u32;
            pos += 1;
            low <<= 8;
            range <<= 8;
        }

        out.push(sym as u8);
        model.update(sym);
    }

    return out;
}
