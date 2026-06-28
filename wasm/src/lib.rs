use fossil::core::{block, container};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn pack_summary(data: &[u8]) -> String {
    let packed = container::write(data, "");
    let blocks = match container::read(&packed) {
        Ok(c) => c.blocks,
        Err(_) => Vec::new(),
    };

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

    return format!(
        "{{\"orig\":{},\"packed\":{},\"blocks\":{}}}",
        data.len(),
        packed.len(),
        items
    );
}
