use std::fs;
use std::io;
use std::path::Path;

use fossil::core::block::{encode_block, model_name};
use fossil::core::container::BLOCK_SIZE;
use fossil::core::entropy::analyze;

use crate::utils::color::Color;
use crate::{error, n};

pub fn run(input: &str) {
    if let Err(err) = inspect(input) {
        error!("{}", err);
    }
}

pub fn inspect(input: &str) -> io::Result<()> {
    let path = Path::new(input);
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("input path is not a file: {}", path.display()),
        ));
    }

    let bytes = fs::read(path)?;
    let whole = analyze(&bytes);
    let n_blocks = bytes.chunks(BLOCK_SIZE).count();

    n!();
    println!(
        "  {}  {}",
        "file".header(),
        path.display().to_string().accent()
    );
    println!(
        "  {}  {} bytes · {:.2} bits/byte ({}) · {} block(s)",
        "data".header(),
        whole.len,
        whole.entropy_bpb,
        whole.class.label(),
        n_blocks,
    );
    n!();

    println!(
        "  {}",
        format!(
            "{:>3}  {:>9}  {:>6}  {:>7}  {:>9}  {:>8}  {:>6}",
            "#", "offset", "size", "entropy", "class", "model", "save"
        )
        .header()
        .bold(),
    );

    for (i, chunk) in bytes.chunks(BLOCK_SIZE).enumerate() {
        let start = i * BLOCK_SIZE;
        let end = start + chunk.len();
        let a = analyze(chunk);
        let (model, payload) = encode_block(&bytes, start, end);
        let save = (1.0 - payload.len() as f64 / chunk.len() as f64) * 100.0;

        println!(
            "  {:>3}  {:>9}  {:>6}  {:>7.2}  {:>9}  {}  {:>5.1}%",
            i,
            i * BLOCK_SIZE,
            chunk.len(),
            a.entropy_bpb,
            a.class.label(),
            format!("{:>8}", model_name(model)).accent(),
            save,
        );
    }

    Ok(())
}
