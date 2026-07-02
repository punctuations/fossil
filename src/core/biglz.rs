use std::cell::RefCell;

use super::models::lz;
use super::varint;

pub const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 1 << 16;
const HASH_SIZE: usize = 1 << 16;
const MAX_CHAIN: usize = 128;

thread_local! {
    static SCRATCH: RefCell<(Vec<usize>, Vec<usize>)> = RefCell::new((Vec::new(), Vec::new()));
}

fn hash3(b: &[u8], i: usize) -> usize {
    let v = (b[i] as usize) | (b[i + 1] as usize) << 8 | (b[i + 2] as usize) << 16;
    (v.wrapping_mul(2654435761) >> 13) & (HASH_SIZE - 1)
}

pub enum Token {
    Lit(u8),
    Match { dist: usize, len: usize },
}

fn find_match(
    bytes: &[u8],
    i: usize,
    head: &[usize],
    prev: &[usize],
    max_chain: usize,
) -> (usize, usize) {
    let n = bytes.len();
    let mut best_len = 0;
    let mut best_dist = 0;

    if i + MIN_MATCH <= n {
        let h = hash3(bytes, i);
        let mut cand = head[h];
        let mut chain = 0;
        let max_len = (n - i).min(MAX_MATCH);
        while cand != usize::MAX && chain < max_chain {
            if bytes[cand + best_len.min(max_len - 1)] == bytes[i + best_len.min(max_len - 1)] {
                let mut l = 0;
                while l < max_len && bytes[cand + l] == bytes[i + l] {
                    l += 1
                }

                if l > best_len {
                    best_len = l;
                    best_dist = i - cand;
                }
            }
            cand = prev[cand];
            chain += 1;
        }
    }

    return (best_dist, best_len);
}

fn insert(bytes: &[u8], p: usize, head: &mut [usize], prev: &mut [usize]) {
    if p + MIN_MATCH <= bytes.len() {
        let h = hash3(bytes, p);
        prev[p] = head[h];
        head[h] = p;
    }
}

pub fn encode(bytes: &[u8]) -> Vec<u8> {
    encode_from(bytes, 0)
}

pub fn encode_from(bytes: &[u8], emit_start: usize) -> Vec<u8> {
    serialize(&tokens(bytes, emit_start))
}

pub fn serialize(tokens: &[Token]) -> Vec<u8> {
    let mut out = Vec::new();
    for group in tokens.chunks(8) {
        let mut flag = 0u8;
        for (k, t) in group.iter().enumerate() {
            if matches!(t, Token::Match { .. }) {
                flag |= 1 << k;
            }
        }

        out.push(flag);
        for t in group {
            match t {
                Token::Lit(b) => out.push(*b),
                Token::Match { dist, len } => {
                    varint::write(&mut out, *dist);
                    varint::write(&mut out, *len - MIN_MATCH);
                }
            }
        }
    }

    return out;
}

pub fn tokens(bytes: &[u8], emit_start: usize) -> Vec<Token> {
    tokens_chain(bytes, emit_start, MAX_CHAIN)
}

pub fn tokens_chain(bytes: &[u8], emit_start: usize, max_chain: usize) -> Vec<Token> {
    let n = bytes.len();

    SCRATCH.with(|cell| {
        let mut guard = cell.borrow_mut();
        let (head, prev) = &mut *guard;
        head.resize(HASH_SIZE, usize::MAX);
        head.fill(usize::MAX);
        prev.resize(n.max(1), 0);

        tokens_inner(bytes, emit_start, head, prev, max_chain)
    })
}

fn tokens_inner(
    bytes: &[u8],
    emit_start: usize,
    head: &mut [usize],
    prev: &mut [usize],
    max_chain: usize,
) -> Vec<Token> {
    let n = bytes.len();
    let mut tokens = Vec::new();

    let mut i = 0;
    while i < emit_start {
        insert(bytes, i, head, prev);
        i += 1;
    }

    while i < n {
        let (dist, len) = find_match(bytes, i, head, prev, max_chain);
        insert(bytes, i, head, prev);

        if len < MIN_MATCH {
            tokens.push(Token::Lit(bytes[i]));
            i += 1;
            continue;
        }

        let next_len = if i + 1 < n {
            find_match(bytes, i + 1, head, prev, max_chain).1
        } else {
            0
        };

        if next_len > len {
            tokens.push(Token::Lit(bytes[i]));
            i += 1;
        } else {
            tokens.push(Token::Match { dist, len });
            let end = i + len;
            let mut j = i + 1;
            while j < end {
                insert(bytes, j, head, prev);
                j += 1;
            }
            i = end;
        }
    }

    return tokens;
}

pub fn decode(data: &[u8], orig_len: usize) -> Vec<u8> {
    return lz::decode(data, orig_len);
}
