use crate::core::varint;

const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
const WINDOW: usize = 4096;

enum Token {
    Lit(u8),
    Match { dist: usize, len: usize },
}

fn longest_match(bytes: &[u8], pos: usize) -> (usize, usize) {
    let start = pos.saturating_sub(WINDOW);
    let max_len = (bytes.len() - pos).min(MAX_MATCH);
    let mut best_len = 0;
    let mut best_dist = 0;

    let mut j = start;
    while j < pos {
        let mut len = 0;
        while len < max_len && bytes[j + len] == bytes[pos + len] {
            len += 1
        }

        if len > best_len {
            best_len = len;
            best_dist = pos - j;
        }
        j += 1;
    }

    return (best_dist, best_len);
}

pub fn encode(bytes: &[u8]) -> Vec<u8> {
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let (dist, len) = longest_match(bytes, i);
        if len >= MIN_MATCH {
            tokens.push(Token::Match { dist, len });
            i += len;
        } else {
            tokens.push(Token::Lit(bytes[i]));
            i += 1;
        }
    }

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

pub fn decode(data: &[u8], count: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(count);
    let mut i = 0;

    while out.len() < count && i < data.len() {
        let flag = data[i];
        i += 1;

        for k in 0..8 {
            if out.len() >= count || i >= data.len() {
                break;
            }

            if flag & (1 << k) == 0 {
                out.push(data[i]);
                i += 1;
            } else {
                let dist = varint::read(data, &mut i);
                let len = varint::read(data, &mut i) + MIN_MATCH;
                if dist == 0 || dist > out.len() {
                    break;
                }
                let start = out.len() - dist;
                for x in 0..len {
                    let b = out[start + x];
                    out.push(b);
                }
            }
        }
    }

    return out;
}

pub fn stats(bytes: &[u8]) -> (usize, usize, usize) {
    let mut literals = 0;
    let mut matches = 0;
    let mut covered = 0;
    let mut i = 0;
    while i < bytes.len() {
        let (_, len) = longest_match(bytes, i);
        if len >= MIN_MATCH {
            matches += 1;
            covered += len;
            i += len;
        } else {
            literals += 1;
            i += 1;
        }
    }
    return (literals, matches, covered);
}
