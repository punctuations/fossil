use std::collections::BTreeMap;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::block::{RAW, SEGMENT_BLOCKS, decode_block, encode_block_seg};
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

    let seg = if ext == "/" {
        SEGMENT_BLOCKS * BLOCK_SIZE
    } else {
        0
    };

    let encoded = encode_blocks(block_src, seg, progress, fast);
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
    if let Some(last) = block_lens.last() {
        varint::write(&mut blocked, *last);
    }

    for (model, payload) in encoded.iter() {
        blocked.push(*model);
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
    seg: usize,
    progress: Option<&Progress>,
    fast: bool,
) -> (u8, Vec<u8>) {
    let out = encode_block_seg(input, start, end, seg, fast);
    if let Some(p) = progress {
        p.done.fetch_add(1, Ordering::Relaxed);
    }
    out
}

fn encode_blocks(
    input: &[u8],
    seg: usize,
    progress: Option<&Progress>,
    fast: bool,
) -> Vec<(u8, Vec<u8>)> {
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
                encode_one(input, start, end, seg, progress, fast)
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
                        local.push((k, encode_one(input, start, end, seg, progress, fast)));
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
            let last_len = if version >= 2 && n_blocks > 0 {
                c.varint()?
            } else {
                0
            };
            let mut blocks = Vec::with_capacity(n_blocks);
            for i in 0..n_blocks {
                let model = c.u8()?;
                let orig_len = if version >= 2 {
                    if i == n_blocks - 1 { last_len } else { BLOCK_SIZE }
                } else {
                    c.varint()?
                };
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
        let seg = if self.ext == "/" {
            SEGMENT_BLOCKS * BLOCK_SIZE
        } else {
            0
        };

        let mut out = Vec::with_capacity(self.orig_size);
        for (i, b) in self.blocks.iter().enumerate() {
            let floor = if seg == 0 {
                0
            } else {
                ((i / SEGMENT_BLOCKS) * seg).min(out.len())
            };
            let decoded = decode_block(b.model, &b.payload, b.orig_len, &out[floor..]);
            out.extend_from_slice(&decoded);
        }

        if self.filter == FILTER_PNG {
            out = image::unfilter(&out);
        }

        return out;
    }
}

pub struct BlockRef {
    pub model: u8,
    pub orig_len: usize,
    pub payload_offset: usize,
    pub payload_len: usize,
}

pub struct LazyContainer<'a> {
    pub ext: String,
    pub orig_size: usize,
    pub crc: u32,
    pub filter: u8,
    pub meta: Vec<u8>,
    pub blocks: Vec<BlockRef>,
    data: &'a [u8],
    seg_blocks: usize,
    cache: BTreeMap<usize, Vec<u8>>,
}

pub fn read_lazy(data: &[u8]) -> io::Result<LazyContainer<'_>> {
    let mut c = Cursor { data, pos: 0 };

    if c.take(4)? != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a fossil file (bad magic)",
        ));
    }
    let version = c.u8()?;
    if version > VERSION {
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

    let mut blocks = Vec::new();

    match mode {
        MODE_STORED => {
            let payload_offset = c.pos;
            c.take(orig_size)?;
            blocks.reserve(orig_size.div_ceil(BLOCK_SIZE));
            let mut off = 0;
            while off < orig_size {
                let len = (orig_size - off).min(BLOCK_SIZE);
                blocks.push(BlockRef {
                    model: RAW,
                    orig_len: len,
                    payload_offset: payload_offset + off,
                    payload_len: len,
                });
                off += len;
            }
        }
        MODE_BLOCKS => {
            let n_blocks = c.varint()?;
            let last_len = if version >= 2 && n_blocks > 0 {
                c.varint()?
            } else {
                0
            };
            blocks.reserve(n_blocks);
            for i in 0..n_blocks {
                let model = c.u8()?;
                let orig_len = if version >= 2 {
                    if i == n_blocks - 1 { last_len } else { BLOCK_SIZE }
                } else {
                    c.varint()?
                };
                let payload_len = c.varint()?;
                let payload_offset = c.pos;
                c.take(payload_len)?;
                blocks.push(BlockRef {
                    model,
                    orig_len,
                    payload_offset,
                    payload_len,
                });
            }
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown container mode {}", other),
            ));
        }
    }

    let seg_blocks = if ext == "/" { SEGMENT_BLOCKS } else { 0 };

    Ok(LazyContainer {
        ext,
        orig_size,
        crc,
        filter,
        meta,
        blocks,
        data,
        seg_blocks,
        cache: BTreeMap::new(),
    })
}

impl<'a> LazyContainer<'a> {
    pub fn read_range(&mut self, offset: usize, len: usize) -> io::Result<Vec<u8>> {
        if offset.saturating_add(len) > self.orig_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "read range beyond archive",
            ));
        }
        decode_range(
            self.data,
            &self.blocks,
            self.seg_blocks,
            &mut self.cache,
            offset,
            len,
        )
    }

    pub fn into_parts(self) -> LazyParts {
        LazyParts {
            ext: self.ext,
            orig_size: self.orig_size,
            crc: self.crc,
            filter: self.filter,
            meta: self.meta,
            blocks: self.blocks,
            seg_blocks: self.seg_blocks,
        }
    }
}

pub struct LazyParts {
    pub ext: String,
    pub orig_size: usize,
    pub crc: u32,
    pub filter: u8,
    pub meta: Vec<u8>,
    pub blocks: Vec<BlockRef>,
    pub seg_blocks: usize,
}

pub fn decode_range(
    data: &[u8],
    blocks: &[BlockRef],
    seg_blocks: usize,
    cache: &mut BTreeMap<usize, Vec<u8>>,
    offset: usize,
    len: usize,
) -> io::Result<Vec<u8>> {
    if len == 0 {
        return Ok(Vec::new());
    }

    let end = offset
        .checked_add(len)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "read range overflow"))?;

    let seg_start = |idx: usize| {
        if seg_blocks == 0 {
            0
        } else {
            (idx / seg_blocks) * seg_blocks
        }
    };

    let first = offset / BLOCK_SIZE;
    let last = (end - 1) / BLOCK_SIZE;

    if last >= blocks.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "read range beyond archive",
        ));
    }

    let from = seg_start(first);
    let base = from * BLOCK_SIZE;

    let mut buf: Vec<u8> = Vec::new();

    for j in from..=last {
        let floor = (seg_start(j) * BLOCK_SIZE - base).min(buf.len());

        let decoded = if cache.contains_key(&j) {
            cache[&j].clone()
        } else {
            let b = &blocks[j];
            let payload = &data[b.payload_offset..b.payload_offset + b.payload_len];
            let d = decode_block(b.model, payload, b.orig_len, &buf[floor..]);
            cache.insert(j, d.clone());
            d
        };

        buf.extend_from_slice(&decoded);
    }

    if end - base > buf.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "read range beyond archive",
        ));
    }

    Ok(buf[offset - base..end - base].to_vec())
}
