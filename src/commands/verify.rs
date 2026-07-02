use std::fs;
use std::io;
use std::path::Path;

use fossil::core::{ container, crc };

use crate::utils::color::Color;
use crate::{ error, n };

pub fn run(input: &str) {
    match check(input) {
        Ok((true, blocks, size)) => {
            n!();
            println!("  {} {} intact", "✓".header(), input.accent());
            println!("  {} block(s) · {} bytes · crc ok", blocks, size);
            n!();
        }
        Ok((false, _, _)) => {
            n!();
            println!("  {} {} is corrupt (crc mismatch)", "✗".coral(), input.accent());
            n!();
            std::process::exit(1);
        }
        Err(err) => {
            error!("{}", err);
            std::process::exit(1);
        }
    }
}

fn check(input: &str) -> io::Result<(bool, usize, usize)> {
    let path = Path::new(input);
    if !path.is_file() {
        return Err(
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("input path is not a file: {}", path.display())
            )
        );
    }

    let data = fs::read(path)?;
    let container = container::read(&data)?;
    let bytes = container.decode();
    let ok = crc::crc32(&bytes) == container.crc;
    Ok((ok, container.blocks.len(), bytes.len()))
}

pub fn help() -> Vec<String> {
    vec![
        "fossil verify".header(),
        "check a .fossil archive without unpacking it".bold(),
        "".into(),
        "usage".header(),
        "  fossil verify <file.fossil>".into(),
        "".into(),
        "arguments".header(),
        "  <file.fossil>     archive to verify".into(),
        "".into(),
        "checks".header(),
        "  header            archive format and version".into(),
        "  blocks            stored block data".into(),
        "  crc               original-data checksum".into(),
        "".into(),
        "examples".header(),
        "  fossil verify archive.fossil".into(),
        "  fossil verify backup.fossil".into()
    ]
}
