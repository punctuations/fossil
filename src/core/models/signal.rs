use crate::core::bits::{BitReader, BitWriter};

const MAX_ORDER: usize = 32;
const COEF_BITS: u32 = 15;
const COEF_MIN: i64 = -(1 << (COEF_BITS - 1));
const COEF_MAX: i64 = (1 << (COEF_BITS - 1)) - 1;
const LPC_ORDERS: [usize; 6] = [2, 4, 8, 12, 16, 32];

struct Config {
    width: u8,
    channels: u8,
    ms: bool,
    be: bool,
}

const CONFIGS: [Config; 13] = [
    Config { width: 1, channels: 1, ms: false, be: false },
    Config { width: 2, channels: 1, ms: false, be: false },
    Config { width: 2, channels: 1, ms: false, be: true },
    Config { width: 2, channels: 2, ms: false, be: false },
    Config { width: 2, channels: 2, ms: true, be: false },
    Config { width: 2, channels: 2, ms: false, be: true },
    Config { width: 2, channels: 2, ms: true, be: true },
    Config { width: 3, channels: 1, ms: false, be: false },
    Config { width: 3, channels: 1, ms: false, be: true },
    Config { width: 3, channels: 2, ms: false, be: false },
    Config { width: 3, channels: 2, ms: true, be: false },
    Config { width: 3, channels: 2, ms: false, be: true },
    Config { width: 3, channels: 2, ms: true, be: true },
];

fn put(w: &mut BitWriter, val: u64, n: u32) {
    for i in (0..n).rev() {
        w.write_bit(((val >> i) & 1) as u8);
    }
}

fn get(r: &mut BitReader, n: u32) -> u64 {
    let mut v = 0u64;
    for _ in 0..n {
        v = (v << 1) | r.read_bit().unwrap_or(0) as u64;
    }
    v
}

fn put_signed(w: &mut BitWriter, v: i64, bits: u32) {
    put(w, (v as u64) & ((1u64 << bits) - 1), bits);
}

fn get_signed(r: &mut BitReader, bits: u32) -> i64 {
    let u = get(r, bits);
    if u & (1u64 << (bits - 1)) != 0 {
        (u as i64) - (1i64 << bits)
    } else {
        u as i64
    }
}

fn zigzag(x: i64) -> u64 {
    ((x << 1) ^ (x >> 63)) as u64
}

fn unzig(u: u64) -> i64 {
    ((u >> 1) as i64) ^ -((u & 1) as i64)
}

fn rice_put(w: &mut BitWriter, u: u64, k: u32) {
    for _ in 0..(u >> k) {
        w.write_bit(1);
    }
    w.write_bit(0);
    if k > 0 {
        put(w, u & ((1 << k) - 1), k);
    }
}

fn rice_get(r: &mut BitReader, k: u32) -> u64 {
    let mut q = 0u64;
    while let Some(1) = r.read_bit() {
        q += 1;
        if q > (1 << 24) {
            break;
        }
    }
    let rem = if k > 0 { get(r, k) } else { 0 };
    (q << k) | rem
}

fn best_k(res: &[i32]) -> (u32, u64) {
    let sum: u64 = res.iter().map(|&r| zigzag(r as i64)).sum();
    let n = res.len().max(1) as u64;
    let mean = sum / n;
    let start = (64 - mean.leading_zeros()).saturating_sub(1);

    let mut best = (0u32, u64::MAX);
    for k in start.saturating_sub(2)..=(start + 2).min(30) {
        let bits: u64 = res
            .iter()
            .map(|&r| (zigzag(r as i64) >> k) + 1 + k as u64)
            .sum();
        if bits < best.1 {
            best = (k, bits);
        }
    }
    best
}

fn partition_p(res: &[i32]) -> u32 {
    let mut best = (0u32, u64::MAX);
    for p in 0..=6u32 {
        let parts = 1usize << p;
        if parts > res.len() {
            break;
        }
        let chunk = res.len().div_ceil(parts).max(1);
        let mut bits = 3u64;
        for c in res.chunks(chunk) {
            bits += 5 + best_k(c).1;
        }
        if bits < best.1 {
            best = (p, bits);
        }
    }
    best.0
}

fn fixed_coefs(order: usize) -> Vec<i64> {
    match order {
        1 => vec![1],
        2 => vec![2, -1],
        3 => vec![3, -3, 1],
        4 => vec![4, -6, 4, -1],
        _ => vec![],
    }
}

fn autocorr(x: &[i32], max_lag: usize) -> Vec<f64> {
    let n = x.len();
    let mut xw = vec![0.0f64; n];
    if n > 1 {
        let c = (n - 1) as f64 / 2.0;
        for (i, w) in xw.iter_mut().enumerate() {
            let t = (i as f64 - c) / c;
            *w = x[i] as f64 * (1.0 - t * t);
        }
    } else if n == 1 {
        xw[0] = x[0] as f64;
    }

    let mut r = vec![0.0; max_lag + 1];
    for (lag, slot) in r.iter_mut().enumerate() {
        let mut s = 0.0;
        for i in lag..n {
            s += xw[i] * xw[i - lag];
        }
        *slot = s;
    }
    r
}

fn levinson(r: &[f64], order: usize) -> Option<Vec<f64>> {
    if r[0] == 0.0 {
        return None;
    }
    let mut a = vec![0.0f64; order + 1];
    let mut err = r[0];
    for m in 1..=order {
        let mut acc = r[m];
        for j in 1..m {
            acc -= a[j] * r[m - j];
        }
        let k = acc / err;
        let prev = a.clone();
        a[m] = k;
        for j in 1..m {
            a[j] = prev[j] - k * prev[m - j];
        }
        err *= 1.0 - k * k;
        if !err.is_finite() || err <= 0.0 {
            break;
        }
    }
    Some(a[1..=order].to_vec())
}

fn quantize(coefs: &[f64]) -> Option<(u32, Vec<i64>)> {
    let cmax = coefs.iter().fold(0.0f64, |m, &c| m.max(c.abs()));
    if cmax <= 0.0 || !cmax.is_finite() {
        return None;
    }
    let mut shift = (COEF_BITS as i32 - 1) - (cmax.log2().floor() as i32) - 1;
    shift = shift.clamp(0, 15);

    let scale = (1i64 << shift) as f64;
    let mut q = Vec::with_capacity(coefs.len());
    let mut err = 0.0;
    for &c in coefs {
        let v = c * scale + err;
        let qi = v.round();
        err = v - qi;
        q.push((qi as i64).clamp(COEF_MIN, COEF_MAX));
    }
    Some((shift as u32, q))
}

fn residual(x: &[i32], q: &[i64], shift: u32) -> Vec<i32> {
    let order = q.len();
    let mut res = Vec::with_capacity(x.len().saturating_sub(order));
    for i in order..x.len() {
        let mut acc = 0i64;
        for (j, &c) in q.iter().enumerate() {
            acc += c * x[i - 1 - j] as i64;
        }
        res.push((x[i] as i64 - (acc >> shift)) as i32);
    }
    res
}

fn restore(warmup: &[i32], res: &[i32], q: &[i64], shift: u32, n: usize) -> Vec<i32> {
    let order = q.len();
    let mut x = Vec::with_capacity(n);
    x.extend_from_slice(warmup);
    for i in order..n {
        let mut acc = 0i64;
        for (j, &c) in q.iter().enumerate() {
            acc += c * x[i - 1 - j] as i64;
        }
        x.push((res[i - order] as i64 + (acc >> shift)) as i32);
    }
    x
}

fn encode_channel(x: &[i32], depth: u32, signed: bool, w: &mut BitWriter) {
    let n = x.len();
    let maxo = MAX_ORDER.min(n.saturating_sub(1));

    let r = autocorr(x, maxo);

    let mut best: Option<(bool, Vec<i64>, u32, Vec<i32>, u32, u64)> = None;
    let consider = |is_lpc: bool, q: Vec<i64>, shift: u32, best: &mut Option<(bool, Vec<i64>, u32, Vec<i32>, u32, u64)>| {
        if q.len() >= n {
            return;
        }
        let res = residual(x, &q, shift);
        let (k, res_bits) = best_k(&res);
        let header = if is_lpc {
            1 + 6 + 4 + q.len() as u64 * COEF_BITS as u64
        } else {
            1 + 3
        };
        let total = header + q.len() as u64 * depth as u64 + 5 + res_bits;
        if best.as_ref().map_or(true, |b| total < b.5) {
            *best = Some((is_lpc, q, shift, res, k, total));
        }
    };

    for order in 0..=4usize {
        consider(false, fixed_coefs(order), 0, &mut best);
    }
    for &order in LPC_ORDERS.iter() {
        if order <= maxo {
            if let Some(coefs) = levinson(&r, order) {
                if let Some((shift, q)) = quantize(&coefs) {
                    consider(true, q, shift, &mut best);
                }
            }
        }
    }

    let (is_lpc, q, shift, res, _, _) = best.unwrap();
    let order = q.len();

    w.write_bit(is_lpc as u8);
    if is_lpc {
        put(w, order as u64, 6);
        put(w, shift as u64, 4);
        for &c in &q {
            put_signed(w, c, COEF_BITS);
        }
    } else {
        put(w, order as u64, 3);
    }
    for &s in &x[..order] {
        if signed {
            put_signed(w, s as i64, depth);
        } else {
            put(w, s as u64, depth);
        }
    }

    let p = partition_p(&res);
    put(w, p as u64, 3);
    let chunk = res.len().div_ceil(1usize << p).max(1);
    for c in res.chunks(chunk) {
        let k = best_k(c).0;
        put(w, k as u64, 5);
        for &e in c {
            rice_put(w, zigzag(e as i64), k);
        }
    }
}

fn decode_channel(r: &mut BitReader, n: usize, depth: u32, signed: bool) -> Vec<i32> {
    let is_lpc = r.read_bit().unwrap_or(0) == 1;
    let (q, shift) = if is_lpc {
        let order = get(r, 6) as usize;
        let shift = get(r, 4) as u32;
        let mut q = Vec::with_capacity(order);
        for _ in 0..order {
            q.push(get_signed(r, COEF_BITS));
        }
        (q, shift)
    } else {
        let order = get(r, 3) as usize;
        (fixed_coefs(order), 0)
    };
    let order = q.len();

    let mut warmup = Vec::with_capacity(order);
    for _ in 0..order {
        warmup.push(if signed {
            get_signed(r, depth) as i32
        } else {
            get(r, depth) as i32
        });
    }

    let res_count = n - order;
    let p = get(r, 3) as u32;
    let chunk = res_count.div_ceil(1usize << p).max(1);
    let mut res = Vec::with_capacity(res_count);
    let mut remaining = res_count;
    while remaining > 0 {
        let this = chunk.min(remaining);
        let k = get(r, 5) as u32;
        for _ in 0..this {
            res.push(unzig(rice_get(r, k)) as i32);
        }
        remaining -= this;
    }

    restore(&warmup, &res, &q, shift, n)
}

fn read_sample(bytes: &[u8], off: usize, width: u8, be: bool) -> i32 {
    match width {
        1 => bytes[off] as i32,
        2 => {
            let (a, b) = (bytes[off] as i32, bytes[off + 1] as i32);
            let u = if be { (a << 8) | b } else { a | (b << 8) };
            if u >= 1 << 15 { u - (1 << 16) } else { u }
        }
        _ => {
            let (a, b, c) = (
                bytes[off] as i32,
                bytes[off + 1] as i32,
                bytes[off + 2] as i32,
            );
            let u = if be { (a << 16) | (b << 8) | c } else { a | (b << 8) | (c << 16) };
            if u >= 1 << 23 { u - (1 << 24) } else { u }
        }
    }
}

fn write_sample(out: &mut Vec<u8>, s: i32, width: u8, be: bool) {
    match width {
        1 => out.push((s & 0xFF) as u8),
        2 => {
            let v = (s & 0xFFFF) as u32;
            let bytes = [(v & 0xFF) as u8, ((v >> 8) & 0xFF) as u8];
            if be {
                out.extend_from_slice(&[bytes[1], bytes[0]]);
            } else {
                out.extend_from_slice(&bytes);
            }
        }
        _ => {
            let v = (s & 0xFFFFFF) as u32;
            let bytes = [(v & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, ((v >> 16) & 0xFF) as u8];
            if be {
                out.extend_from_slice(&[bytes[2], bytes[1], bytes[0]]);
            } else {
                out.extend_from_slice(&bytes);
            }
        }
    }
}

fn depth_of(cfg: &Config, c: usize) -> (u32, bool) {
    if cfg.width == 1 {
        (8, false)
    } else {
        let base = cfg.width as u32 * 8;
        if cfg.ms && c == 1 { (base + 1, true) } else { (base, true) }
    }
}

fn encode_with(bytes: &[u8], cfg: &Config) -> Vec<u8> {
    let frame = cfg.channels as usize * cfg.width as usize;
    let n = bytes.len() / frame;
    let chans = cfg.channels as usize;

    let mut ch = vec![Vec::with_capacity(n); chans];
    for f in 0..n {
        for (c, col) in ch.iter_mut().enumerate() {
            col.push(read_sample(bytes, f * frame + c * cfg.width as usize, cfg.width, cfg.be));
        }
    }
    if cfg.ms {
        for f in 0..n {
            let (l, rr) = (ch[0][f], ch[1][f]);
            ch[0][f] = (l + rr) >> 1;
            ch[1][f] = l - rr;
        }
    }

    let mut w = BitWriter::new();
    for (c, col) in ch.iter().enumerate() {
        let (depth, signed) = depth_of(cfg, c);
        encode_channel(col, depth, signed, &mut w);
    }
    let stream = w.finish();

    let tail = &bytes[n * frame..];
    let mut out = Vec::with_capacity(1 + tail.len() + stream.len());
    out.extend_from_slice(tail);
    out.extend_from_slice(&stream);
    out
}

pub fn encode(bytes: &[u8]) -> Vec<u8> {
    let mut best: Option<Vec<u8>> = None;
    let mut best_cfg = 0u8;

    for (ci, cfg) in CONFIGS.iter().enumerate() {
        let frame = cfg.channels as usize * cfg.width as usize;
        if bytes.len() < frame * 2 {
            continue;
        }
        let body = encode_with(bytes, cfg);
        if best.as_ref().map_or(true, |b| body.len() < b.len()) {
            best = Some(body);
            best_cfg = ci as u8;
        }
    }

    let body = match best {
        Some(b) => b,
        None => {
            let mut out = Vec::with_capacity(1 + bytes.len());
            out.push(0xFE);
            out.extend_from_slice(bytes);
            return out;
        }
    };
    let mut out = Vec::with_capacity(1 + body.len());
    out.push(best_cfg);
    out.extend_from_slice(&body);
    out
}

pub fn decode(data: &[u8], orig_len: usize) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }
    if data[0] == 0xFE {
        return data[1..].to_vec();
    }
    if data[0] as usize >= CONFIGS.len() {
        return Vec::new();
    }
    let cfg = &CONFIGS[data[0] as usize];

    let frame = cfg.channels as usize * cfg.width as usize;
    let n = orig_len / frame;
    let tail_len = orig_len - n * frame;

    let tail = &data[1..1 + tail_len];
    let mut r = BitReader::new(&data[1 + tail_len..]);

    let chans = cfg.channels as usize;
    let mut ch = Vec::with_capacity(chans);
    for c in 0..chans {
        let (depth, signed) = depth_of(cfg, c);
        ch.push(decode_channel(&mut r, n, depth, signed));
    }
    if cfg.ms {
        for f in 0..n {
            let (mid, side) = (ch[0][f], ch[1][f]);
            let sum = (mid << 1) | (side & 1);
            ch[0][f] = (sum + side) >> 1;
            ch[1][f] = (sum - side) >> 1;
        }
    }

    let mut out = Vec::with_capacity(orig_len);
    for f in 0..n {
        for col in ch.iter() {
            write_sample(&mut out, col[f], cfg.width, cfg.be);
        }
    }
    out.extend_from_slice(tail);
    out
}
