use std::fs;
use std::io;
use std::path::Path;

use fossil::core::block::{self, decode_block, model_name};
use fossil::core::container::{self, Container};
use fossil::core::entropy::{EntropyReport, analyze};
use fossil::core::models::{generator, lz};

use crate::utils::color::{Color, paint};
use crate::{error, n};

pub fn run(input: &str, block: Option<usize>) {
    let result = match block {
        Some(n) => detail(input, n),
        None => overview(input),
    };
    if let Err(err) = result {
        error!("{}", err);
    }
}

fn reason(model: u8) -> &'static str {
    match model {
        block::RLE => "adjacent repeated bytes",
        block::ENTROPY => "skewed byte frequencies (canonical Huffman)",
        block::LZ => "repeated substrings",
        block::LZH => "LZ, then Huffman",
        block::BWTM => "Burrows-Wheeler + move-to-front + range coding",
        block::RANGE => "adaptive range coding, no stored table",
        block::PPM => "order-1 context (each byte from the last)",
        block::GEN => "formulas like constant fills and ramps",
        block::DELTA => "smooth, slowly-changing data",
        block::CSVT => "tabular data, columns, grouped",
        block::WORD => "repeated words, dictionary coded",
        _ => "the fallback, stored as-is",
    }
}

fn overview(input: &str) -> io::Result<()> {
    let path = Path::new(input);
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("input path is not a file: {}", path.display()),
        ));
    }

    let data = fs::read(path)?;
    let c = container::read(&data)?;

    let origin = if c.ext.is_empty() {
        "(no ext)".to_string()
    } else {
        format!(".{}", c.ext)
    };
    let packed = data.len();
    let pct = if c.orig_size == 0 {
        0.0
    } else {
        (1.0 - packed as f64 / c.orig_size as f64) * 100.0
    };
    let verdict = if pct >= 0.0 {
        format!("{:.1}% smaller", pct).header()
    } else {
        format!("{:.1}% larger", -pct).coral()
    };

    n!();
    println!(
        "  {}    {}",
        "file".header(),
        path.display().to_string().accent()
    );
    println!(
        "  {}  {} · {} block(s)",
        "origin".header(),
        origin,
        c.blocks.len()
    );
    println!(
        "  {}    {} → {} bytes  {}",
        "size".header(),
        c.orig_size,
        packed.to_string().bold(),
        verdict
    );
    n!();

    println!(
        "  {}",
        format!(
            "{:>3}  {:>9}  {:>9}  {:>8}  {:>7}   {}",
            "#", "original", "packed", "model", "save", "why"
        )
        .header()
        .bold(),
    );

    for (i, b) in c.blocks.iter().enumerate() {
        let save = if b.orig_len == 0 {
            0.0
        } else {
            (1.0 - b.payload.len() as f64 / b.orig_len as f64) * 100.0
        };
        println!(
            "  {:>3}  {:>9}  {:>9}  {}  {:>6.1}%   {}",
            i,
            b.orig_len,
            b.payload.len(),
            format!("{:>8}", model_name(b.model)).accent(),
            save,
            reason(b.model),
        );
    }

    let rebuilt = c.decode();
    n!();
    if rebuilt.len() == c.orig_size {
        println!("  {} reconstructs to {} bytes", "✓".header(), rebuilt.len());
    } else {
        println!(
            "  {} size mismatch: {} vs {}",
            "✗".coral(),
            rebuilt.len(),
            c.orig_size
        );
    }
    n!();

    Ok(())
}

fn detail(input: &str, n: usize) -> io::Result<()> {
    let path = Path::new(input);
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("input path is not a file: {}", path.display()),
        ));
    }

    let data = fs::read(path)?;
    let c: Container = container::read(&data)?;

    if n >= c.blocks.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("block {} out of range (file has {})", n, c.blocks.len()),
        ));
    }

    let offset: usize = c.blocks[..n].iter().map(|b| b.orig_len).sum();
    let mut history = Vec::with_capacity(offset);
    for pb in &c.blocks[..n] {
        let d = decode_block(pb.model, &pb.payload, pb.orig_len, &history);
        history.extend_from_slice(&d);
    }
    let b = &c.blocks[n];
    let bytes = decode_block(b.model, &b.payload, b.orig_len, &history);
    let a = analyze(&bytes);
    let (runs, longest) = run_stats(&bytes);
    let (lits, matches, covered) = lz::stats(&bytes);

    n!();
    println!(
        "  {}  block {} of {}",
        "explain".header(),
        n,
        c.blocks.len() - 1
    );
    println!(
        "  {}    bytes {}..{}",
        "range".header(),
        offset,
        offset + b.orig_len
    );
    println!(
        "  {}     {} → {} bytes ({:.1}% saved)  via {}",
        "size".header(),
        b.orig_len,
        b.payload.len(),
        (1.0 - b.payload.len() as f64 / b.orig_len.max(1) as f64) * 100.0,
        model_name(b.model).accent(),
    );
    n!();

    println!("  {}", "structure".header().bold());
    println!(
        "    entropy   {:.2} bits/byte ({})",
        a.entropy_bpb,
        a.class.label()
    );
    println!("    distinct  {} / 256 byte values", a.unique_bytes);
    println!("    runs      {} (longest {})", runs, longest);
    println!(
        "    repeats   {} matches cover {:.0}% ({} literals)",
        matches,
        covered as f64 / b.orig_len.max(1) as f64 * 100.0,
        lits,
    );
    println!(
        "    {}",
        model_insight(b.model, &a, runs, covered, b.orig_len)
    );
    n!();

    if b.model == block::GEN {
        println!("  {}", "recipe".header().bold());
        for seg in generator::describe(&b.payload) {
            println!("    {}", seg);
        }
        n!();
    }

    println!("  {}", "pattern  (byte value: dark→light)".header().bold());
    for chunk in bytes.chunks(64).take(32) {
        let mut line = String::from("    ");
        for &byte in chunk {
            let shade = 232 + (byte as usize * 23 / 255);
            line.push_str(&paint("█", &format!("38;5;{}", shade)));
        }
        println!("{}", line);
    }
    if bytes.len() > 64 * 32 {
        println!("    … ({} more bytes)", bytes.len() - 64 * 32);
    }
    n!();

    Ok(())
}

fn run_stats(data: &[u8]) -> (usize, usize) {
    if data.is_empty() {
        return (0, 0);
    }
    let mut count = 1;
    let mut longest = 1;
    let mut cur = 1;
    for w in data.windows(2) {
        if w[0] == w[1] {
            cur += 1;
        } else {
            count += 1;
            longest = longest.max(cur);
            cur = 1;
        }
    }
    return (count, longest.max(cur));
}

fn model_insight(model: u8, a: &EntropyReport, runs: usize, covered: usize, len: usize) -> String {
    let pct = covered as f64 / len.max(1) as f64 * 100.0;
    match model {
        block::RLE => format!("→ RLE: {} adjacent runs collapse cheaply", runs),
        block::ENTROPY => format!(
            "→ Huffman: only {} symbols, {:.1} bits/byte to exploit",
            a.unique_bytes, a.entropy_bpb
        ),
        block::LZ => format!("→ LZ: {:.0}% of bytes are repeated substrings", pct),
        block::LZH => format!("→ LZ+Huffman: {:.0}% repeats, then entropy-coded", pct),
        block::BWTM => "→ BWT regrouped similar contexts into runs, then entropy-coded".to_string(),
        block::RANGE => format!(
            "→ range: adaptive coding near the {:.1} bits/byte entropy floor (no table)",
            a.entropy_bpb
        ),
        block::PPM => format!(
            "→ PPM: each byte predicted from the previous one (order-1 context beats the {:.1} bits/byte order-0 floor)",
            a.entropy_bpb
        ),
        block::GEN => {
            "→ generator: formulaic region stored as constant fills & arithmetic ramps — a recipe, not bytes".to_string()
        }
        block::DELTA => format!(
              "→ delta: consecutive bytes change little, so differences entropy-code near {:.1} bits/byte",
              a.entropy_bpb
        ),
        block::CSVT => {
            "→ CSV: a rectangular table, transposed so each column's values sit together".to_string()
        }
        block::WORD => {
            "→ word: text with repeating words, replaced by short dictionary references".to_string()
        }
        _ => "→ no exploitable structure — stored raw".to_string(),
    }
}
