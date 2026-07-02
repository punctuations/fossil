use std::io;

use super::varint;

const MAGIC: &[u8; 4] = b"FDIR";

#[derive(Debug, Clone)]
pub struct Entry {
    pub path: String,
    pub offset: usize,
    pub len: usize,
}

pub fn pack(files: &[(String, Vec<u8>)]) -> (Vec<u8>, Vec<u8>) {
    let mut meta = Vec::new();
    let mut payload = Vec::new();

    meta.extend_from_slice(MAGIC);
    varint::write(&mut meta, files.len());

    for (path, bytes) in files {
        let path_bytes = path.as_bytes();

        varint::write(&mut meta, path_bytes.len());
        meta.extend_from_slice(path_bytes);
        varint::write(&mut meta, bytes.len());

        payload.extend_from_slice(bytes);
    }

    (meta, payload)
}

pub fn read(meta: &[u8]) -> io::Result<Vec<Entry>> {
    if meta.len() < MAGIC.len() || &meta[..MAGIC.len()] != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing directory manifest",
        ));
    }

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

        entries.push(Entry { path, offset, len });
        offset += len;
    }

    Ok(entries)
}
