use std::fs;
use std::io;
use std::path::Path;

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
