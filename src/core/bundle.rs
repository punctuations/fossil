use super::varint;

pub fn pack(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    varint::write(&mut out, files.len());

    for (path, data) in files {
        let p = path.as_bytes();
        varint::write(&mut out, p.len());
        out.extend_from_slice(p);
        varint::write(&mut out, data.len());
    }

    for (_, data) in files {
        out.extend_from_slice(data);
    }

    return out;
}

pub fn unpack(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut pos = 0;
    let n = varint::read(bytes, &mut pos);

    let mut meta = Vec::with_capacity(n);
    for _ in 0..n {
        let plen = varint::read(bytes, &mut pos);
        let end = (pos + plen).min(bytes.len());
        let path = String::from_utf8_lossy(&bytes[pos..end]).into_owned();
        pos = end;
        let dlen = varint::read(bytes, &mut pos);
        meta.push((path, dlen));
    }

    let mut files = Vec::with_capacity(n);
    for (path, dlen) in meta {
        let end = (pos + dlen).min(bytes.len());
        files.push((path, bytes[pos..end].to_vec()));
        pos = end;
    }

    return files;
}
