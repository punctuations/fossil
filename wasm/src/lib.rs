use fossil::core::{block, container, image};
use wasm_bindgen::prelude::*;

fn summarize(orig_len: usize, packed_len: usize, blocks: &[container::Block]) -> String {
    let mut items = String::from("[");
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 {
            items.push(',');
        }
        items.push_str(&format!(
            "{{\"model\":\"{}\",\"orig\":{},\"packed\":{}}}",
            block::model_name(b.model),
            b.orig_len,
            b.payload.len()
        ));
    }
    items.push(']');

    format!(
        "{{\"orig\":{},\"packed\":{},\"blocks\":{}}}",
        orig_len, packed_len, items
    )
}

#[wasm_bindgen]
pub fn pack_summary(data: &[u8]) -> String {
    let packed = container::write(data, "");
    let blocks = container::read(&packed).map(|c| c.blocks).unwrap_or_default();
    summarize(data.len(), packed.len(), &blocks)
}

#[wasm_bindgen]
pub struct Packer {
    orig: Vec<u8>,
    src: Vec<u8>,
    filtered: bool,
    encoded: Vec<(u8, Vec<u8>)>,
    pos: usize,
    n_blocks: usize,
}

#[wasm_bindgen]
impl Packer {
    #[wasm_bindgen(constructor)]
    pub fn new(data: &[u8]) -> Packer {
        let (filtered, src) = match image::detect(data) {
            Some(img) => (true, image::filter(data, &img)),
            None => (false, data.to_vec()),
        };
        let n_blocks = if src.is_empty() {
            0
        } else {
            src.len().div_ceil(container::BLOCK_SIZE)
        };
        Packer {
            orig: data.to_vec(),
            src,
            filtered,
            encoded: Vec::new(),
            pos: 0,
            n_blocks,
        }
    }

    pub fn total(&self) -> usize {
        self.n_blocks
    }

    pub fn step(&mut self, batch: usize) -> usize {
        let bs = container::BLOCK_SIZE;
        let end_block = (self.pos + batch.max(1)).min(self.n_blocks);
        for k in self.pos..end_block {
            let start = k * bs;
            let end = (start + bs).min(self.src.len());
            self.encoded.push(block::encode_block(&self.src, start, end, false));
        }
        self.pos = end_block;
        self.pos
    }

    pub fn finish(&self) -> String {
        let bs = container::BLOCK_SIZE;
        let block_lens: Vec<usize> = (0..self.n_blocks)
            .map(|k| ((k + 1) * bs).min(self.src.len()) - k * bs)
            .collect();
        let packed = container::assemble(&self.orig, "", self.filtered, &block_lens, &self.encoded);
        let blocks = container::read(&packed).map(|c| c.blocks).unwrap_or_default();
        summarize(self.orig.len(), packed.len(), &blocks)
    }
}
