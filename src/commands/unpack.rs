use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use fossil::core::{bundle, container, crc};

use crate::utils::color::Color;
use crate::utils::spinner::Spinner;
use crate::{error, n};

pub fn run(input: &str, output: &str) {
    let sp = Spinner::start("exhuming…");
    let result = unpack(input, output);
    sp.stop();
    match result {
        Ok(r) => {
            n!();
            println!(
                "  {} {} {}",
                r.input.display().to_string().accent(),
                "→".bold(),
                r.output.display().to_string().accent(),
            );
            match r.files {
                Some(n) => {
                    println!(
                        "  {} blocks  → {} bytes ({} files)",
                        r.blocks.to_string().bold(),
                        r.size.to_string().bold(),
                        n.to_string().bold(),
                    );
                }
                None => {
                    println!(
                        "  {} blocks → {} bytes",
                        r.blocks.to_string().bold(),
                        r.size.to_string().bold()
                    );
                }
            }

            n!();
        }
        Err(err) => error!("{}", err),
    }
}

struct UnpackReport {
    input: PathBuf,
    output: PathBuf,
    blocks: usize,
    size: usize,
    files: Option<usize>,
}

fn unpack(input: &str, output: &str) -> io::Result<UnpackReport> {
    let input_path = Path::new(input);
    if !input_path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("input path is not a file: {}", input_path.display()),
        ));
    }

    let data = fs::read(input_path)?;
    let container = container::read(&data)?;
    let bytes = container.decode();

    if crc::crc32(&bytes) != container.crc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "checksum mismatch -- fossil is corrupt",
        ));
    }

    let mut archive_files = None;
    let output_path = if container.ext == "/" {
        let root = Path::new(output);
        let mut written = 0;
        for (rel, contents) in bundle::unpack(&bytes) {
            let dest = root.join(&rel);
            if rel.contains("..") || Path::new(&rel).is_absolute() {
                // skip '..' and absolute paths to avoid malicious fossils
                continue;
            }
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&dest, &contents)?;
            written += 1;
        }
        archive_files = Some(written);
        root.to_path_buf()
    } else {
        let path = resolve_output(output, &container.ext);
        fs::write(&path, &bytes)?;
        path
    };

    Ok(UnpackReport {
        input: input_path.to_path_buf(),
        output: output_path,
        blocks: container.blocks.len(),
        size: bytes.len(),
        files: archive_files,
    })
}

fn resolve_output(output: &str, ext: &str) -> PathBuf {
    let p = Path::new(output);
    if ext.is_empty() || p.extension().is_some() {
        p.to_path_buf()
    } else {
        p.with_extension(ext)
    }
}
