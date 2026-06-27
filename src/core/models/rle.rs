pub fn encode(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        let mut run: u32 = 1;

        while i + (run as usize) < bytes.len() && bytes[i + run as usize] == b {
            run += 1
        }

        out.extend_from_slice(&run.to_le_bytes());
        out.push(b);
        i += run as usize;
    }

    return out;
}

pub fn decode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;

    while i + 5 <= data.len() {
        let run = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        let b = data[i + 4];
        out.extend(std::iter::repeat(b).take(run as usize));
        i += 5;
    }

    return out;
}
