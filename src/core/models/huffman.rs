use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::collections::HashMap;

use crate::core::bits::{BitReader, BitWriter};
use crate::core::entropy::histogram;
use crate::core::varint;

enum Node {
    Leaf(u8),
    Internal(usize, usize),
}

pub fn code_lengths(bytes: &[u8]) -> [u8; 256] {
    let mut lengths = [0u8; 256];
    let hist = histogram(bytes);

    let present: Vec<usize> = (0..256).filter(|&s| hist[s] > 0).collect();
    if present.is_empty() {
        return lengths;
    }
    if present.len() == 1 {
        lengths[present[0]] = 1;
        return lengths;
    }

    let mut nodes: Vec<Node> = Vec::new();
    let mut heap: BinaryHeap<Reverse<(usize, usize)>> = BinaryHeap::new();

    for &s in &present {
        let idx = nodes.len();
        nodes.push(Node::Leaf(s as u8));
        heap.push(Reverse((hist[s], idx)));
    }

    while heap.len() > 1 {
        let Reverse((f1, a)) = heap.pop().unwrap();
        let Reverse((f2, b)) = heap.pop().unwrap();
        let idx = nodes.len();

        nodes.push(Node::Internal(a, b));
        heap.push(Reverse((f1 + f2, idx)));
    }

    let Reverse((_, root)) = heap.pop().unwrap();
    assign_depths(&nodes, root, 0, &mut lengths);
    return lengths;
}

fn assign_depths(nodes: &[Node], idx: usize, depth: u8, lengths: &mut [u8; 256]) {
    match nodes[idx] {
        Node::Leaf(sym) => lengths[sym as usize] = depth,
        Node::Internal(a, b) => {
            assign_depths(nodes, a, depth + 1, lengths);
            assign_depths(nodes, b, depth + 1, lengths);
        }
    }
}

pub fn canonical_codes(lengths: &[u8; 256]) -> [u32; 256] {
    let mut codes = [0u32; 256];

    let max_len = *lengths.iter().max().unwrap() as usize;
    if max_len == 0 {
        return codes;
    }

    let mut bl_count = vec![0u32; max_len + 1];
    for &l in lengths.iter() {
        if l > 0 {
            bl_count[l as usize] += 1;
        }
    }

    let mut next_code = vec![0u32; max_len + 1];
    let mut code = 0u32;
    for bits in 1..=max_len {
        code = (code + bl_count[bits - 1]) << 1;
        next_code[bits] = code;
    }

    for sym in 0..256 {
        let l = lengths[sym] as usize;
        if l != 0 {
            codes[sym] = next_code[l];
            next_code[l] += 1;
        }
    }

    return codes;
}

fn write_table(out: &mut Vec<u8>, lengths: &[u8; 256]) {
    let present: Vec<usize> = (0..256).filter(|&s| lengths[s] > 0).collect();

    let mut sparse = vec![1u8];
    varint::write(&mut sparse, present.len());
    for &s in &present {
        sparse.push(s as u8);
        sparse.push(lengths[s]);
    }

    let max_len = *lengths.iter().max().unwrap();
    if max_len <= 15 && sparse.len() >= 130 {
        out.push(2);
        for pair in lengths.chunks(2) {
            out.push(pair[0] | (pair[1] << 4));
        }
        return;
    }

    if sparse.len() < 257 {
        out.extend_from_slice(&sparse);
    } else {
        out.push(0);
        out.extend_from_slice(lengths);
    }
}

fn read_table(data: &[u8], pos: &mut usize) -> [u8; 256] {
    let mut lengths = [0u8; 256];
    if *pos >= data.len() {
        return lengths;
    }
    let mode = data[*pos];
    *pos += 1;

    if mode == 0 {
        let end = (*pos + 256).min(data.len());
        lengths[..end - *pos].copy_from_slice(&data[*pos..end]);
        *pos = end;
    } else if mode == 2 {
        let end = (*pos + 128).min(data.len());
        for (i, &b) in data[*pos..end].iter().enumerate() {
            lengths[i * 2] = b & 0x0f;
            lengths[i * 2 + 1] = b >> 4;
        }
        *pos = end;
    } else {
        let n = varint::read(data, pos);
        for _ in 0..n {
            if *pos + 2 > data.len() {
                break;
            }
            let s = data[*pos] as usize;
            lengths[s] = data[*pos + 1];
            *pos += 2
        }
    }

    return lengths;
}

pub fn encode(bytes: &[u8]) -> Vec<u8> {
    let lengths = code_lengths(bytes);
    let codes = canonical_codes(&lengths);

    let mut out = Vec::new();
    write_table(&mut out, &lengths);

    let mut w = BitWriter::new();
    for &b in bytes.iter() {
        let sym = b as usize;
        let code = codes[sym];
        for i in (0..lengths[sym]).rev() {
            w.write_bit(((code >> i) & 1) as u8);
        }
    }

    out.extend_from_slice(&w.finish());

    return out;
}

pub fn decode(data: &[u8], count: usize) -> Vec<u8> {
    if count == 0 || data.is_empty() {
        return Vec::new();
    }

    let mut pos = 0;
    let lengths = read_table(data, &mut pos);
    let codes = canonical_codes(&lengths);

    let mut table: HashMap<(u8, u32), u8> = HashMap::new();
    for sym in 0..256 {
        if lengths[sym] > 0 {
            table.insert((lengths[sym], codes[sym]), sym as u8);
        }
    }

    let mut out = Vec::with_capacity(count);
    let mut r = BitReader::new(&data[pos..]);
    let mut cur_code: u32 = 0;
    let mut cur_len: u8 = 0;

    while out.len() < count {
        match r.read_bit() {
            Some(bit) => {
                cur_code = (cur_code << 1) | bit as u32;
                cur_len += 1;
                if let Some(&sym) = table.get(&(cur_len, cur_code)) {
                    out.push(sym);
                    cur_code = 0;
                    cur_len = 0;
                }
            }
            None => break,
        }
    }

    return out;
}
