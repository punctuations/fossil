use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::block::{RAW, decode_block, encode_block};
use super::crc;
use super::image;
use super::varint;

const MAGIC: &[u8; 4] = b"FOSL";
const VERSION: u8 = 2;
pub const BLOCK_SIZE: usize = 4096;
const MODE_BLOCKS: u8 = 0;
const MODE_STORED: u8 = 1;
const FILTER_NONE: u8 = 0;
const FILTER_PNG: u8 = 1;

pub struct Block {
    pub model: u8,
    pub orig_len: usize,
    pub payload: Vec<u8>,
}

pub struct Container {
    pub ext: String,
    pub orig_size: usize,
    pub crc: u32,
    pub filter: u8,
    pub meta: Vec<u8>,
    pub blocks: Vec<Block>,
}

#[derive(Default)]
pub struct Progress {
    pub done: AtomicUsize,
    pub total: AtomicUsize,
}

fn header(mode: u8, filter: u8, ext: &[u8], orig_size: usize, crc: u32, meta: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();

    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(mode);
    out.push(filter);
    out.push(ext.len() as u8);
    out.extend_from_slice(ext);

    varint::write(&mut out, orig_size);
    out.extend_from_slice(&crc.to_le_bytes());

    varint::write(&mut out, meta.len());
    out.extend_from_slice(meta);

    out
}

pub fn write(bytes: &[u8], ext: &str) -> Vec<u8> {
    write_progress(bytes, ext, None, false)
}

pub fn write_progress(bytes: &[u8], ext: &str, progress: Option<&Progress>, fast: bool) -> Vec<u8> {
    write_progress_meta(bytes, ext, &[], progress, fast)
}

pub fn write_progress_meta(
    bytes: &[u8],
    ext: &str,
    meta: &[u8],
    progress: Option<&Progress>,
    fast: bool,
) -> Vec<u8> {
    let filtered = if ext == "/" {
        None
    } else {
        image::detect(bytes).map(|img| image::filter(bytes, &img))
    };

    let block_src: &[u8] = filtered.as_deref().unwrap_or(bytes);

    if let Some(p) = progress {
        let n = if block_src.is_empty() {
            0
        } else {
            block_src.len().div_ceil(BLOCK_SIZE)
        };
        p.total.store(n, Ordering::Relaxed);
    }

    let encoded = encode_blocks(block_src, progress, fast);
    let block_lens: Vec<usize> = block_src.chunks(BLOCK_SIZE).map(|c| c.len()).collect();

    assemble(bytes, ext, filtered.is_some(), meta, &block_lens, &encoded)
}

pub fn assemble(
    orig: &[u8],
    ext: &str,
    filtered: bool,
    meta: &[u8],
    block_lens: &[usize],
    encoded: &[(u8, Vec<u8>)],
) -> Vec<u8> {
    let ext_bytes = ext.as_bytes();
    let crc = crc::crc32(orig);
    let filter = if filtered { FILTER_PNG } else { FILTER_NONE };

    let mut blocked = header(MODE_BLOCKS, filter, ext_bytes, orig.len(), crc, meta);

    varint::write(&mut blocked, encoded.len());

    for (i, (model, payload)) in encoded.iter().enumerate() {
        blocked.push(*model);
        varint::write(&mut blocked, block_lens[i]);
        varint::write(&mut blocked, payload.len());
        blocked.extend_from_slice(payload);
    }

    let mut stored = header(MODE_STORED, FILTER_NONE, ext_bytes, orig.len(), crc, meta);
    stored.extend_from_slice(orig);

    if blocked.len() <= stored.len() {
        blocked
    } else {
        stored
    }
}

fn encode_one(
    input: &[u8],
    start: usize,
    end: usize,
    progress: Option<&Progress>,
    fast: bool,
) -> (u8, Vec<u8>) {
    let out = encode_block(input, start, end, fast);
    if let Some(p) = progress {
        p.done.fetch_add(1, Ordering::Relaxed);
    }
    out
}

fn encode_blocks(input: &[u8], progress: Option<&Progress>, fast: bool) -> Vec<(u8, Vec<u8>)> {
    let n = input.len();
    let n_blocks = if n == 0 { 0 } else { n.div_ceil(BLOCK_SIZE) };

    let threads = std::thread::available_parallelism()
        .map(|x| x.get())
        .unwrap_or(1);

    if n_blocks <= 1 || threads <= 1 {
        return (0..n_blocks)
            .map(|k| {
                let start = k * BLOCK_SIZE;
                let end = (start + BLOCK_SIZE).min(n);
                encode_one(input, start, end, progress, fast)
            })
            .collect();
    }

    let next = AtomicUsize::new(0);

    return std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut local: Vec<(usize, (u8, Vec<u8>))> = Vec::new();
                    loop {
                        let k = next.fetch_add(1, Ordering::Relaxed);
                        if k >= n_blocks {
                            break;
                        }
                        let start = k * BLOCK_SIZE;
                        let end = (start + BLOCK_SIZE).min(n);
                        local.push((k, encode_one(input, start, end, progress, fast)));
                    }
                    local
                })
            })
            .collect();

        let mut all: Vec<(usize, (u8, Vec<u8>))> = Vec::with_capacity(n_blocks);
        for h in handles {
            all.extend(h.join().unwrap());
        }
        all.sort_by_key(|(k, _)| *k);
        all.into_iter().map(|(_, r)| r).collect()
    });
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
        if self.pos + n > self.data.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "truncated fossil",
            ));
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u32le(&mut self) -> io::Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn varint(&mut self) -> io::Result<usize> {
        let mut result: usize = 0;
        let mut shift = 0;
        loop {
            let byte = self.u8()?;
            result |= ((byte & 0x7f) as usize) << shift;
            if byte & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
        }
    }
}

pub fn read(data: &[u8]) -> io::Result<Container> {
    let mut c = Cursor { data, pos: 0 };

    if c.take(4)? != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a fossil file (bad magic)",
        ));
    }
    let version = c.u8()?;
    if version > VERSION {
        // backwards compat, if file version is newer than fossil version unsupported.
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported version {}", version),
        ));
    }

    let mode = c.u8()?;
    let filter = c.u8()?;
    let ext_len = c.u8()? as usize;
    let ext = String::from_utf8_lossy(c.take(ext_len)?).into_owned();
    let orig_size = c.varint()?;
    let crc = c.u32le()?;

    let meta = if version >= 2 {
        let meta_len = c.varint()?;
        c.take(meta_len)?.to_vec()
    } else {
        Vec::new()
    };

    let blocks = match mode {
        MODE_STORED => {
            let raw = c.take(orig_size)?.to_vec();
            vec![Block {
                model: RAW,
                orig_len: orig_size,
                payload: raw,
            }]
        }
        MODE_BLOCKS => {
            let n_blocks = c.varint()?;
            let mut blocks = Vec::with_capacity(n_blocks);
            for _ in 0..n_blocks {
                let model = c.u8()?;
                let orig_len = c.varint()?;
                let pay_len = c.varint()?;
                let payload = c.take(pay_len)?.to_vec();
                blocks.push(Block {
                    model,
                    orig_len,
                    payload,
                });
            }
            blocks
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown container mode {}", other),
            ));
        }
    };

    return Ok(Container {
        ext,
        orig_size,
        crc,
        filter,
        meta,
        blocks,
    });
}

impl Container {
    pub fn decode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.orig_size);
        for b in &self.blocks {
            let decoded = decode_block(b.model, &b.payload, b.orig_len, &out);
            out.extend_from_slice(&decoded);
        }

        if self.filter == FILTER_PNG {
            out = image::unfilter(&out);
        }

        return out;
    }
}
