pub struct Image {
    pub header: usize,
    pub bpp: usize,
    pub stride: usize,
    pub rows: usize,
}

fn ppm_geometry(data: &[u8]) -> Option<Image> {
    let channels = if data.starts_with(b"P6") {
        3
    } else if data.starts_with(b"P5") {
        1
    } else {
        return None;
    };

    let mut pos = 2;
    let mut nums = [0usize; 3];
    for slot in nums.iter_mut() {
        while pos < data.len() {
            let c = data[pos];
            if c == b'#' {
                while pos < data.len() && data[pos] != b'\n' {
                    pos += 1;
                }
            } else if c.is_ascii_whitespace() {
                pos += 1;
            } else {
                break;
            }
        }
        let start = pos;
        while pos < data.len() && data[pos].is_ascii_digit() {
            pos += 1;
        }
        if pos == start {
            return None;
        }
        *slot = std::str::from_utf8(&data[start..pos]).ok()?.parse().ok()?;
    }
    if pos < data.len() && data[pos].is_ascii_whitespace() {
        pos += 1;
    }

    let (w, h, maxval) = (nums[0], nums[1], nums[2]);
    if w == 0 || h == 0 || maxval == 0 {
        return None;
    }

    let sample = if maxval < 256 { 1 } else { 2 };
    let bpp = channels * sample;
    let stride = w.checked_mul(bpp)?;

    Some(Image {
        header: pos,
        bpp,
        stride,
        rows: h,
    })
}

fn le_u32(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

fn bmp_geometry(data: &[u8]) -> Option<Image> {
    if !data.starts_with(b"BM") || data.len() < 54 {
        return None;
    }

    let offset = le_u32(data, 10) as usize;
    let bi_size = le_u32(data, 14) as usize;
    if bi_size < 40 {
        return None;
    }

    let width = le_u32(data, 18) as i32;
    let height = le_u32(data, 22) as i32;
    let bit_count = u16::from_le_bytes([data[28], data[29]]);
    let compression = le_u32(data, 30);

    if compression != 0 || width <= 0 || height == 0 {
        return None;
    }
    if bit_count != 24 && bit_count != 32 {
        return None;
    }

    let w = width as usize;
    let rows = height.unsigned_abs() as usize;
    let bpp = (bit_count / 8) as usize;
    let stride = ((w.checked_mul(bpp)? + 3) / 4) * 4;

    if offset > data.len() {
        return None;
    }

    Some(Image {
        header: offset,
        bpp,
        stride,
        rows,
    })
}

fn geometry(data: &[u8]) -> Option<Image> {
    if data.starts_with(b"P6") || data.starts_with(b"P5") {
        ppm_geometry(data)
    } else if data.starts_with(b"BM") {
        bmp_geometry(data)
    } else {
        None
    }
}

pub fn detect(data: &[u8]) -> Option<Image> {
    let img = geometry(data)?;
    if img.stride == 0 || img.rows == 0 {
        return None;
    }
    let pixels = img.rows.checked_mul(img.stride)?;
    if img.header.checked_add(pixels)? != data.len() {
        return None;
    }
    Some(img)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (ai, bi, ci) = (a as i32, b as i32, c as i32);
    let p = ai + bi - ci;
    let pa = (p - ai).abs();
    let pb = (p - bi).abs();
    let pc = (p - ci).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn predict(ftype: u8, a: u8, b: u8, c: u8) -> u8 {
    match ftype {
        1 => a,
        2 => b,
        3 => ((a as u16 + b as u16) / 2) as u8,
        4 => paeth(a, b, c),
        _ => 0,
    }
}

pub fn filter(data: &[u8], img: &Image) -> Vec<u8> {
    let stride = img.stride;
    let bpp = img.bpp;
    let px = &data[img.header..];

    let mut out = Vec::with_capacity(data.len() + img.rows);
    out.extend_from_slice(&data[..img.header]);

    let mut best = vec![0u8; stride];
    let mut cand = vec![0u8; stride];

    for r in 0..img.rows {
        let cur = &px[r * stride..r * stride + stride];
        let prev: Option<&[u8]> = if r > 0 {
            Some(&px[(r - 1) * stride..(r - 1) * stride + stride])
        } else {
            None
        };

        let mut best_type = 0u8;
        let mut best_score = u64::MAX;
        for ftype in 0u8..5 {
            let mut score = 0u64;
            for x in 0..stride {
                let a = if x >= bpp { cur[x - bpp] } else { 0 };
                let b = prev.map_or(0, |p| p[x]);
                let c = if x >= bpp { prev.map_or(0, |p| p[x - bpp]) } else { 0 };
                let res = cur[x].wrapping_sub(predict(ftype, a, b, c));
                cand[x] = res;
                score += (res as i8).unsigned_abs() as u64;
            }
            if score < best_score {
                best_score = score;
                best_type = ftype;
                best.copy_from_slice(&cand);
            }
        }

        out.push(best_type);
        out.extend_from_slice(&best);
    }

    out
}

pub fn unfilter(data: &[u8]) -> Vec<u8> {
    let img = match geometry(data) {
        Some(i) => i,
        None => return data.to_vec(),
    };

    let stride = img.stride;
    let bpp = img.bpp;
    let row_span = 1 + stride;
    let filtered = &data[img.header..];

    let mut out = Vec::with_capacity(img.header + img.rows * stride);
    out.extend_from_slice(&data[..img.header]);

    let mut px = vec![0u8; img.rows * stride];
    for r in 0..img.rows {
        let base = r * row_span;
        if base + row_span > filtered.len() {
            break;
        }
        let ftype = filtered[base];
        let frow = &filtered[base + 1..base + 1 + stride];
        let cur = r * stride;
        for x in 0..stride {
            let a = if x >= bpp { px[cur + x - bpp] } else { 0 };
            let b = if r > 0 { px[cur - stride + x] } else { 0 };
            let c = if r > 0 && x >= bpp { px[cur - stride + x - bpp] } else { 0 };
            px[cur + x] = frow[x].wrapping_add(predict(ftype, a, b, c));
        }
    }

    out.extend_from_slice(&px);
    out
}
