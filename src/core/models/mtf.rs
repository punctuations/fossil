pub fn forward(data: &[u8]) -> Vec<u8> {
    let mut table: Vec<u8> = (0..=255).collect();
    let mut out = Vec::with_capacity(data.len());

    for &b in data {
        let pos = table.iter().position(|&x| x == b).unwrap();
        out.push(pos as u8);
        table.remove(pos);
        table.insert(0, b);
    }

    return out;
}

pub fn inverse(data: &[u8]) -> Vec<u8> {
    let mut table: Vec<u8> = (0..=255).collect();
    let mut out = Vec::with_capacity(data.len());

    for &pos in data {
        let b = table[pos as usize];
        out.push(b);
        table.remove(pos as usize);
        table.insert(0, b);
    }

    return out;
}
