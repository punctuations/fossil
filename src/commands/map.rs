use std::fs;
use std::io;
use std::path::Path;

use fossil::core::block;
use fossil::core::container::{self, Container};
use fossil::core::entropy::{EntropyClass, analyze};

use crate::utils::color::{Color, paint};
use crate::{error, n};

const COLS: usize = 64;
const MIN_SEG: usize = 64;
const MAX_ROWS: usize = 16;
const CELL: &str = "█";

pub fn run(input: &str) {
    if let Err(err) = map(input) {
        error!("{}", err);
    }
}

fn model_color(model: u8) -> &'static str {
    match model {
        block::RLE => "38;5;105",
        block::ENTROPY => "38;5;44",
        block::LZ => "38;5;77",
        block::LZH => "38;5;75",
        block::LZR => "38;5;111",
        block::BWTM => "38;5;177",
        block::RANGE => "38;5;221",
        block::PPM => "38;5;215",
        block::GEN => "38;5;203",
        block::DELTA => "38;5;79",
        block::CSVT => "38;5;43",
        block::WORD => "38;5;169",
        block::SIGNAL => "38;5;209",
        _ => "38;5;144",
    }
}

fn model_map(path: &Path, c: &Container) -> io::Result<()> {
    n!();
    println!(
        "  {}  {}",
        "map".header(),
        path.display().to_string().accent()
    );
    println!(
        "  {}  {} blocks · {} bytes original",
        "data".header(),
        c.blocks.len(),
        c.orig_size,
    );
    n!();

    let cap = COLS * MAX_ROWS;
    let shown = c.blocks.len().min(cap);

    let cols_used = shown.min(COLS);
    let mut ruler = String::new();
    let mut i = 0;
    while i < cols_used {
        if i % 8 == 0 {
            let label = i.to_string();
            ruler.push_str(&label);
            i += label.len();
        } else {
            ruler.push(' ');
            i += 1;
        }
    }
    println!(
        "  {}  {}",
        paint(&format!("{:>9}", "block"), "38;5;244"),
        paint(&ruler, "38;5;244"),
    );

    let painted: Vec<String> = c.blocks[..shown]
        .iter()
        .map(|b| paint(CELL, model_color(b.model)))
        .collect();
    for (row, cells) in painted.chunks(COLS).enumerate() {
        let gutter = paint(&format!("{:>9}", row * COLS), "38;5;240");
        println!("  {}  {}", gutter, cells.concat());
    }
    if c.blocks.len() > shown {
        println!(
            "  {}",
            paint(
                &format!("… {} more blocks", c.blocks.len() - shown),
                "38;5;244"
            )
        );
    }

    n!();
    let mut present = [false; 16];
    for b in &c.blocks {
        if (b.model as usize) < present.len() {
            present[b.model as usize] = true;
        }
    }
    let mut legend = String::from("  ");
    for id in 0u8..16 {
        if present[id as usize] {
            legend.push_str(&paint(CELL, model_color(id)));
            legend.push(' ');
            legend.push_str(block::model_name(id));
            legend.push_str("   ");
        }
    }
    println!("{}", legend);
    n!();

    Ok(())
}

fn color_for(class: EntropyClass) -> &'static str {
    match class {
        EntropyClass::Empty => "38;5;245",
        EntropyClass::VeryLow => "38;5;111",
        EntropyClass::Low => "38;5;80",
        EntropyClass::Medium => "38;5;221",
        EntropyClass::High => "38;5;215",
        EntropyClass::VeryHigh => "38;5;203",
    }
}

fn map(input: &str) -> io::Result<()> {
    let path = Path::new(input);
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("input path is not a file: {}", path.display()),
        ));
    }

    let bytes = fs::read(path)?;

    if bytes.starts_with(b"FOSL") {
        let c = container::read(&bytes).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("corrupt fossil: {}", e))
        })?;
        return model_map(path, &c);
    }

    if bytes.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "file is empty"));
    }

    let max_cells = COLS * MAX_ROWS;
    let seg = ((bytes.len() + max_cells - 1) / max_cells).max(MIN_SEG);

    n!();
    println!(
        "  {}  {}",
        "map".header(),
        path.display().to_string().accent()
    );
    n!();

    let mut painted = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let end = (start + seg).min(bytes.len());
        let a = analyze(&bytes[start..end]);
        painted.push(paint(CELL, color_for(a.class)));
        start = end;
    }

    let cols_used = painted.len().min(COLS);
    let mut ruler = String::new();
    let mut i = 0;
    while i < cols_used {
        if i % 8 == 0 {
            let label = i.to_string();
            ruler.push_str(&label);
            i += label.len();
        } else {
            ruler.push(' ');
            i += 1;
        }
    }
    println!(
        "  {}  {}",
        paint(&format!("{:>9}", "offset"), "38;5;244"),
        paint(&ruler, "38;5;244"),
    );

    for (row, cells) in painted.chunks(COLS).enumerate() {
        let offset = row * COLS * seg;
        let gutter = paint(&format!("{:>9}", offset), "38;5;240");
        println!("  {}  {}", gutter, cells.concat());
    }

    n!();
    println!(
        "  {} {} {} {} {}   {}",
        paint(CELL, color_for(EntropyClass::VeryLow)),
        paint(CELL, color_for(EntropyClass::Low)),
        paint(CELL, color_for(EntropyClass::Medium)),
        paint(CELL, color_for(EntropyClass::High)),
        paint(CELL, color_for(EntropyClass::VeryHigh)),
        "low → high entropy".header(),
    );
    n!();

    Ok(())
}
