use crate::core::varint;

const MIN_RUN: usize = 4;

enum Seg {
    Literal(Vec<u8>),
    Const { byte: u8, len: usize },
    Ramp { start: u8, step: u8, len: usize },
}

pub fn encode(bytes: &[u8]) -> Vec<u8> {
    let n = bytes.len();
    let mut segs: Vec<Seg> = Vec::new();
    let mut lit: Vec<u8> = Vec::new();
    let mut i = 0;

    while i < n {
        if i + 1 < n {
            let step = bytes[i + 1].wrapping_sub(bytes[i]);
            let mut run = 2;
            while i + run < n && bytes[i + run].wrapping_sub(bytes[i + run - 1]) == step {
                run += 1;
            }
            if run >= MIN_RUN {
                if !lit.is_empty() {
                    segs.push(Seg::Literal(std::mem::take(&mut lit)))
                }
                if step == 0 {
                    segs.push(Seg::Const {
                        byte: bytes[i],
                        len: run,
                    });
                } else {
                    segs.push(Seg::Ramp {
                        start: bytes[i],
                        step,
                        len: run,
                    });
                }
                i += run;
                continue;
            }
        }
        lit.push(bytes[i]);
        i += 1;
    }
    if !lit.is_empty() {
        segs.push(Seg::Literal(lit));
    }

    let mut out = Vec::new();
    varint::write(&mut out, segs.len());
    for seg in &segs {
        match seg {
            Seg::Literal(b) => {
                out.push(0);
                varint::write(&mut out, b.len());
                out.extend_from_slice(b);
            }
            Seg::Const { byte, len } => {
                out.push(1);
                varint::write(&mut out, *len);
                out.push(*byte);
            }
            Seg::Ramp { start, step, len } => {
                out.push(2);
                varint::write(&mut out, *len);
                out.push(*start);
                out.push(*step);
            }
        }
    }

    return out;
}

pub fn describe(data: &[u8]) -> Vec<String> {
    let mut pos = 0;
    let n_segs = varint::read(data, &mut pos);
    let mut out = Vec::new();

    for _ in 0..n_segs {
        if pos >= data.len() {
            break;
        }
        let kind = data[pos];
        pos += 1;
        let len = varint::read(data, &mut pos);
        match kind {
            0 => {
                pos = (pos + len).min(data.len());
                out.push(format!("literal      × {}", len));
            }
            1 => {
                if pos < data.len() {
                    let byte = data[pos];
                    pos += 1;
                    out.push(format!("const 0x{:02X}   × {}", byte, len));
                }
            }
            2 => {
                if pos + 1 < data.len() {
                    let (start, step) = (data[pos], data[pos + 1]);
                    pos += 2;
                    out.push(format!("ramp 0x{:02X} {:+} × {}", start, step as i8, len));
                }
            }
            _ => break,
        }
    }

    return out;
}

pub fn decode(data: &[u8]) -> Vec<u8> {
    let mut pos = 0;
    let n_segs = varint::read(data, &mut pos);
    let mut out = Vec::new();

    for _ in 0..n_segs {
        if pos >= data.len() {
            break;
        }

        let kind = data[pos];
        pos += 1;
        let len = varint::read(data, &mut pos);
        match kind {
            0 => {
                let end = (pos + len).min(data.len());
                out.extend_from_slice(&data[pos..end]);
                pos = end;
            }
            1 => {
                if pos < data.len() {
                    let byte = data[pos];
                    pos += 1;
                    out.extend(std::iter::repeat(byte).take(len));
                }
            }
            2 => {
                if pos + 1 < data.len() {
                    let (start, step) = (data[pos], data[pos + 1]);
                    pos += 2;
                    let mut v = start;
                    for _ in 0..len {
                        out.push(v);
                        v = v.wrapping_add(step);
                    }
                }
            }
            _ => break,
        }
    }

    return out;
}
