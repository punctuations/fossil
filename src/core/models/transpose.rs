use super::range;
use crate::core::varint;

fn transpose(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.is_empty() {
        return None;
    }
    let trailing_nl = *bytes.last().unwrap() == b'\n';
    let body = if trailing_nl {
        &bytes[..bytes.len() - 1]
    } else {
        bytes
    };

    let rows: Vec<&[u8]> = body.split(|&b| b == b'\n').collect();
    if rows.len() < 2 {
        return None;
    }

    let grid: Vec<Vec<&[u8]>> = rows
        .iter()
        .map(|r| r.split(|&b| b == b',').collect())
        .collect();
    let ncols = grid[0].len();
    if ncols < 2 || grid.iter().any(|r| r.len() != ncols) {
        return None;
    }

    let mut out = Vec::new();
    varint::write(&mut out, rows.len());
    varint::write(&mut out, ncols);
    out.push(trailing_nl as u8);
    for col in 0..ncols {
        for row in &grid {
            out.extend_from_slice(row[col]);
            out.push(b'\n');
        }
    }
    return Some(out);
}

fn untranspose(data: &[u8]) -> Vec<u8> {
    let mut pos = 0;
    let nrows = varint::read(data, &mut pos);
    let ncols = varint::read(data, &mut pos);
    let trailing_nl = data.get(pos).copied().unwrap_or(0) != 0;
    pos += 1;

    let mut fields: Vec<&[u8]> = Vec::with_capacity(nrows * ncols);
    let mut i = pos;
    while fields.len() < nrows * ncols && i <= data.len() {
        let start = i;
        while i < data.len() && data[i] != b'\n' {
            i += 1;
        }
        fields.push(&data[start..i.min(data.len())]);
        i += 1;
    }

    let mut out = Vec::new();
    for row in 0..nrows {
        for col in 0..ncols {
            if col > 0 {
                out.push(b',');
            }
            if let Some(f) = fields.get(col * nrows + row) {
                out.extend_from_slice(f);
            }
        }
        if row + 1 < nrows || trailing_nl {
            out.push(b'\n');
        }
    }
    return out;
}

pub fn encode(bytes: &[u8]) -> Vec<u8> {
    match transpose(bytes) {
        Some(t) => {
            let mut out = vec![1u8];
            varint::write(&mut out, t.len());
            out.extend_from_slice(&range::encode(&t));
            out
        }
        None => {
            let mut out = vec![0u8];
            out.extend_from_slice(bytes);
            out
        }
    }
}

pub fn decode(data: &[u8], _orig_len: usize) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }
    if data[0] == 0 {
        return data[1..].to_vec();
    }
    let mut pos = 1;
    let t_len = varint::read(data, &mut pos);
    let t = range::decode(&data[pos..], t_len);
    return untranspose(&t);
}
