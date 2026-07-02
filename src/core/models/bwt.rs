fn radix_pass(input: &[usize], key: &[usize], count: &mut [usize], out: &mut [usize]) {
    for c in count.iter_mut() {
        *c = 0;
    }
    for &s in input {
        count[key[s]] += 1;
    }
    let mut sum = 0;
    for c in count.iter_mut() {
        let start = sum;
        sum += *c;
        *c = start;
    }
    for &s in input {
        let bucket = key[s];
        out[count[bucket]] = s;
        count[bucket] += 1;
    }
}

pub fn forward(data: &[u8]) -> (Vec<u8>, usize) {
    let n = data.len();
    if n == 0 {
        return (Vec::new(), 0);
    }

    let mut sa: Vec<usize> = (0..n).collect();
    let mut rank: Vec<usize> = data.iter().map(|&b| b as usize).collect();
    let mut tmp = vec![0usize; n];
    let mut rank2 = vec![0usize; n];
    let mut scratch = vec![0usize; n];
    let mut count = vec![0usize; n.max(256)];

    let mut k = 1;
    loop {
        for i in 0..n {
            rank2[i] = rank[(i + k) % n];
        }

        radix_pass(&sa, &rank2, &mut count, &mut scratch);
        radix_pass(&scratch, &rank, &mut count, &mut sa);

        tmp[sa[0]] = 0;
        for w in 1..n {
            let a = sa[w - 1];
            let b = sa[w];
            let same = rank[a] == rank[b] && rank2[a] == rank2[b];
            tmp[b] = tmp[a] + if same { 0 } else { 1 };
        }
        rank.copy_from_slice(&tmp);

        if rank[sa[n - 1]] == n - 1 || k >= n {
            break;
        }
        k *= 2;
    }

    let mut last = Vec::with_capacity(n);
    let mut primary = 0;
    for (i, &r) in sa.iter().enumerate() {
        if r == 0 {
            primary = i;
        }
        last.push(data[(r + n - 1) % n]);
    }

    return (last, primary);
}

pub fn inverse(last: &[u8], primary: usize) -> Vec<u8> {
    let n = last.len();
    if n == 0 {
        return Vec::new();
    }

    let mut counts = [0usize; 256];
    for &b in last {
        counts[b as usize] += 1;
    }

    let mut c = [0usize; 256];
    let mut sum = 0;
    for s in 0..256 {
        c[s] = sum;
        sum += counts[s];
    }

    let mut lf = vec![0usize; n];
    let mut occ = [0usize; 256];
    for i in 0..n {
        let b = last[i] as usize;
        lf[i] = c[b] + occ[b];
        occ[b] += 1;
    }

    let mut out = vec![0u8; n];
    let mut p = primary;
    for j in (0..n).rev() {
        out[j] = last[p];
        p = lf[p];
    }

    return out;
}
