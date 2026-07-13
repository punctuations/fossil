use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use fossil::core::{container, crc, dir};

use crate::error;
use crate::utils::color::Color;

pub fn run(input: &str, inner_path: Option<&str>, output_path: Option<&str>, trust: bool) {
    match take(input, inner_path, trust) {
        Ok(bytes) => {
            if output_path.is_some() {
                if let Err(err) = fs::write(output_path.unwrap_or(""), bytes) {
                    error!("{}", err);
                }
            } else {
                if let Err(err) = io::stdout().write_all(&bytes) {
                    error!("{}", err);
                }
            }
        }
        Err(err) => error!("{}", err),
    }
}

fn take(input: &str, inner_path: Option<&str>, trust: bool) -> io::Result<Vec<u8>> {
    let from_stdin = input == "-";

    let data = if from_stdin {
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf)?;
        buf
    } else {
        let input_path = Path::new(input);

        if !input_path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("input path is not a file: {}", input_path.display()),
            ));
        }

        fs::read(input_path)?
    };

    let head = container::read_lazy(&data)?;

    if head.ext == "/" {
        let wanted = inner_path.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "directory fossil requires a path: fossil take <archive> <path>",
            )
        })?;

        if head.meta.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "directory fossil has no manifest",
            ));
        }

        let wanted = wanted.trim_start_matches("./").replace('\\', "/");
        let entries = dir::read(&head.meta)?;

        let (offset, len, want_crc) = entries
            .iter()
            .find(|entry| entry.path == wanted)
            .map(|entry| (entry.offset, entry.len, entry.crc))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("file not found in archive: {}", wanted),
                )
            })?;

        let mut lazy = head;
        let bytes = lazy.read_range(offset, len)?;

        if !trust {
            if let Some(want) = want_crc {
                if crc::crc32(&bytes) != want {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "checksum mismatch, file is corrupt (use --trust to skip)",
                    ));
                }
            }
        }

        return Ok(bytes);
    }

    if inner_path.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "inner path only works with directory fossils",
        ));
    }

    let archive = container::read(&data)?;
    let bytes = archive.decode();

    if !trust && crc::crc32(&bytes) != archive.crc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "checksum mismatch, fossil is corrupt (use --trust to skip)",
        ));
    }

    Ok(bytes)
}

pub fn help() -> Vec<String> {
    vec![
        "fossil take".header(),
        "write data from a fossil archive to stdout".bold(),
        "".into(),
        "usage".header(),
        "  fossil take <file.fossil> [options]".into(),
        "  fossil take <dir.fossil> <path> [output] [options]".into(),
        "".into(),
        "arguments".header(),
        "  <file.fossil>     archive to read from".into(),
        "  <path>            file inside a directory fossil".into(),
        "  [output]          output file location".into(),
        "".into(),
        "options".header(),
        "  --trust           skip CRC verification".into(),
        "".into(),
        "examples".header(),
        "  fossil take notes.fossil".into(),
        "  fossil take archive.fossil src/main.rs".into(),
        "  fossil take archive.fossil README.md > README.md".into(),
        "  cat archive.fossil | fossil take - src/main.rs".into(),
    ]
}
