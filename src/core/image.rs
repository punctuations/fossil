pub struct Image {
    pub header: usize,
    pub bpp: usize,
    pub stride: usize,
}

pub fn detect(data: &[u8]) -> Option<Image> {
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
    let pixels = stride.checked_mul(h)?;
    if pos.checked_add(pixels)? != data.len() {
        return None;
    }

    Some(Image {
        header: pos,
        bpp,
        stride,
    })
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

pub fn filter(data: &[u8], img: &Image) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[..img.header]);

    let px = &data[img.header..];
    for i in 0..px.len() {
        let left = if i >= img.bpp { px[i - img.bpp] } else { 0 };
        let up = if i >= img.stride { px[i - img.stride] } else { 0 };
        let upleft = if i >= img.stride + img.bpp {
            px[i - img.stride - img.bpp]
        } else {
            0
        };
        out.push(px[i].wrapping_sub(paeth(left, up, upleft)));
    }
    out
}

pub fn unfilter(data: &[u8]) -> Vec<u8> {
    let img = match detect(data) {
        Some(i) => i,
        None => return data.to_vec(),
    };

    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[..img.header]);

    let res = &data[img.header..];
    let mut px = vec![0u8; res.len()];
    for i in 0..res.len() {
        let left = if i >= img.bpp { px[i - img.bpp] } else { 0 };
        let up = if i >= img.stride { px[i - img.stride] } else { 0 };
        let upleft = if i >= img.stride + img.bpp {
            px[i - img.stride - img.bpp]
        } else {
            0
        };
        px[i] = res[i].wrapping_add(paeth(left, up, upleft));
    }
    out.extend_from_slice(&px);
    out
}
