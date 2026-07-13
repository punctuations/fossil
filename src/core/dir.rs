use std::io;

use super::crc;
use super::varint;

const MAGIC: &[u8; 4] = b"FDIR";
const MAGIC_V2: &[u8; 4] = b"FDR2";

#[derive(Debug, Clone)]
pub struct Entry {
    pub path: String,
    pub offset: usize,
    pub len: usize,
    pub crc: Option<u32>,
}

pub fn pack(files: &[(String, Vec<u8>)]) -> (Vec<u8>, Vec<u8>) {
    let mut meta = Vec::new();
    let mut payload = Vec::new();

    meta.extend_from_slice(MAGIC_V2);
    varint::write(&mut meta, files.len());

    for (path, bytes) in files {
        let path_bytes = path.as_bytes();

        varint::write(&mut meta, path_bytes.len());
        meta.extend_from_slice(path_bytes);
        varint::write(&mut meta, bytes.len());
        meta.extend_from_slice(&crc::crc32(bytes).to_le_bytes());

        payload.extend_from_slice(bytes);
    }

    (meta, payload)
}

pub fn read(meta: &[u8]) -> io::Result<Vec<Entry>> {
    if meta.len() < MAGIC.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing directory manifest",
        ));
    }

    let has_crc = match &meta[..MAGIC.len()] {
        m if m == MAGIC_V2 => true,
        m if m == MAGIC => false,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "missing directory manifest",
            ));
        }
    };

    let mut pos = MAGIC.len();
    let count = varint::read(meta, &mut pos);

    let mut entries = Vec::with_capacity(count);
    let mut offset = 0usize;

    for _ in 0..count {
        let path_len = varint::read(meta, &mut pos);

        if pos + path_len > meta.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "truncated directory manifest",
            ));
        }

        let path = String::from_utf8_lossy(&meta[pos..pos + path_len]).into_owned();
        pos += path_len;

        let len = varint::read(meta, &mut pos);

        let crc = if has_crc {
            if pos + 4 > meta.len() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "truncated directory manifest",
                ));
            }
            let c = u32::from_le_bytes([meta[pos], meta[pos + 1], meta[pos + 2], meta[pos + 3]]);
            pos += 4;
            Some(c)
        } else {
            None
        };

        entries.push(Entry {
            path,
            offset,
            len,
            crc,
        });
        offset += len;
    }

    Ok(entries)
}
