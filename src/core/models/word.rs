use std::collections::HashMap;

use super::range;
use crate::core::varint;

fn tokenize(bytes: &[u8]) -> Vec<u8> {
    let mut dict: Vec<&[u8]> = Vec::new();
    let mut index: HashMap<&[u8], usize> = HashMap::new();
    let mut items: Vec<(bool, usize)> = Vec::new();

    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_alphabetic() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                i += 1;
            }
            let w = &bytes[start..i];
            let id = match index.get(w) {
                Some(&id) => id,
                None => {
                    let id = dict.len();
                    dict.push(w);
                    index.insert(w, id);
                    id
                }
            };
            items.push((true, id));
        } else {
            items.push((false, bytes[i] as usize));
            i += 1;
        }
    }

    let mut out = Vec::new();
    varint::write(&mut out, dict.len());
    for w in &dict {
        varint::write(&mut out, w.len());
        out.extend_from_slice(w);
    }
    varint::write(&mut out, items.len());
    for group in items.chunks(8) {
        let mut flag = 0u8;
        for (k, (is_word, _)) in group.iter().enumerate() {
            if *is_word {
                flag |= 1 << k;
            }
        }
        out.push(flag);
        for (is_word, v) in group {
            if *is_word {
                varint::write(&mut out, *v);
            } else {
                out.push(*v as u8);
            }
        }
    }
    return out;
}

fn untokenize(data: &[u8]) -> Vec<u8> {
    let mut pos = 0;
    let n_words = varint::read(data, &mut pos);
    let mut dict: Vec<Vec<u8>> = Vec::with_capacity(n_words);
    for _ in 0..n_words {
        let len = varint::read(data, &mut pos);
        let end = (pos + len).min(data.len());
        dict.push(data[pos..end].to_vec());
        pos = end;
    }

    let n_items = varint::read(data, &mut pos);
    let mut out = Vec::new();
    let mut flag = 0u8;
    let mut bit = 8;
    for _ in 0..n_items {
        if bit == 8 {
            flag = data.get(pos).copied().unwrap_or(0);
            pos += 1;
            bit = 0;
        }
        let is_word = (flag >> bit) & 1 == 1;
        bit += 1;
        if is_word {
            let id = varint::read(data, &mut pos);
            if let Some(w) = dict.get(id) {
                out.extend_from_slice(w);
            }
        } else if pos < data.len() {
            out.push(data[pos]);
            pos += 1;
        }
    }
    return out;
}

pub fn encode(bytes: &[u8]) -> Vec<u8> {
    let t = tokenize(bytes);
    let mut out = Vec::new();
    varint::write(&mut out, t.len());
    out.extend_from_slice(&range::encode(&t));
    return out;
}

pub fn decode(data: &[u8], _orig_len: usize) -> Vec<u8> {
    let mut pos = 0;
    let t_len = varint::read(data, &mut pos);
    let t = range::decode(&data[pos..], t_len);
    return untokenize(&t);
}
