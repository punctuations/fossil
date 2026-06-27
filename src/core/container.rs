use std::io;

use super::block::{RAW, decode_block, encode_block};
use super::crc;
use super::varint;

const MAGIC: &[u8; 4] = b"FOSL";
const VERSION: u8 = 1;
pub const BLOCK_SIZE: usize = 4096;
const MODE_BLOCKS: u8 = 0;
const MODE_STORED: u8 = 1;

pub struct Block {
    pub model: u8,
    pub orig_len: usize,
    pub payload: Vec<u8>,
}

pub struct Container {
    pub ext: String,
    pub orig_size: usize,
    pub crc: u32,
    pub blocks: Vec<Block>,
}

fn header(mode: u8, ext: &[u8], orig_size: usize, crc: u32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(mode);
    out.push(ext.len() as u8);
    out.extend_from_slice(ext);
    varint::write(&mut out, orig_size);
    out.extend_from_slice(&crc.to_le_bytes());
    return out;
}

pub fn write(bytes: &[u8], ext: &str) -> Vec<u8> {
    let ext_bytes = ext.as_bytes();
    let crc = crc::crc32(bytes);
    let blocks: Vec<&[u8]> = bytes.chunks(BLOCK_SIZE).collect();

    let mut blocked = header(MODE_BLOCKS, ext_bytes, bytes.len(), crc);
    varint::write(&mut blocked, blocks.len());

    for block in &blocks {
        let (model, payload) = encode_block(block);
        blocked.push(model);
        varint::write(&mut blocked, block.len());
        varint::write(&mut blocked, payload.len());
        blocked.extend_from_slice(&payload);
    }

    let mut stored = header(MODE_STORED, ext_bytes, bytes.len(), crc);
    stored.extend_from_slice(bytes);

    if blocked.len() <= stored.len() {
        return blocked;
    }
    return stored;
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
    if version != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported version {}", version),
        ));
    }

    let mode = c.u8()?;
    let ext_len = c.u8()? as usize;
    let ext = String::from_utf8_lossy(c.take(ext_len)?).into_owned();
    let orig_size = c.varint()?;
    let crc = c.u32le()?;

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
        blocks,
    });
}

impl Container {
    pub fn decode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.orig_size);
        for b in &self.blocks {
            out.extend_from_slice(&decode_block(b.model, &b.payload, b.orig_len));
        }

        return out;
    }
}
