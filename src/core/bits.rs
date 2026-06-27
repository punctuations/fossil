pub struct BitWriter {
    bytes: Vec<u8>,
    cur: u8,
    nbits: u8,
}

impl BitWriter {
    pub fn new() -> Self {
        BitWriter {
            bytes: Vec::new(),
            cur: 0,
            nbits: 0,
        }
    }

    pub fn write_bit(&mut self, bit: u8) {
        self.cur = (self.cur << 1) | (bit & 1);
        self.nbits += 1;
        if self.nbits == 8 {
            self.bytes.push(self.cur);
            self.cur = 0;
            self.nbits = 0;
        }
    }

    pub fn finish(mut self) -> Vec<u8> {
        if self.nbits > 0 {
            self.cur <<= 8 - self.nbits;
            self.bytes.push(self.cur);
        }
        return self.bytes;
    }
}

pub struct BitReader<'a> {
    bytes: &'a [u8],
    pos: usize,
    nbits: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        BitReader {
            bytes,
            pos: 0,
            nbits: 0,
        }
    }

    pub fn read_bit(&mut self) -> Option<u8> {
        if self.pos >= self.bytes.len() {
            return None;
        }

        let bit = (self.bytes[self.pos] >> (7 - self.nbits)) & 1;
        self.nbits += 1;
        if self.nbits == 8 {
            self.nbits = 0;
            self.pos += 1;
        }

        Some(bit)
    }
}
