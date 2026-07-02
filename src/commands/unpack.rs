use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use fossil::core::{container, crc, dir};

use crate::utils::color::Color;
use crate::utils::spinner::Spinner;
use crate::{error, n};

pub fn run(input: &str, output: &str, trust: bool) {
    let sp = Spinner::start("exhuming…");
    let result = unpack(input, output, trust);
    sp.stop();
    match result {
        Ok(r) if output == "-" => {
            let _ = r;
        }
        Ok(r) => {
            n!();
            println!(
                "  {} {} {}",
                r.input.display().to_string().accent(),
                "→".bold(),
                r.output.display().to_string().accent()
            );
            match r.files {
                Some(n) => {
                    println!(
                        "  {} blocks  → {} bytes ({} files)",
                        r.blocks.to_string().bold(),
                        r.size.to_string().bold(),
                        n.to_string().bold()
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

fn unpack(input: &str, output: &str, trust: bool) -> io::Result<UnpackReport> {
    let from_stdin = input == "-";
    let to_stdout = output == "-";

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

    let container = container::read(&data)?;
    let bytes = container.decode();

    // could still pass on corrupted dir manifest
    if !trust && crc::crc32(&bytes) != container.crc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "checksum mismatch, fossil is corrupt (use --trust to skip)",
        ));
    }

    let mut archive_files = None;
    let output_path = if container.ext == "/" {
        if to_stdout {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "can't write a directory archive to stdout",
            ));
        }

        let root = Path::new(output);
        let entries = dir::read(&container.meta)?;

        let mut written = 0;

        for entry in entries {
            let rel_path = Path::new(&entry.path);

            let unsafe_path = rel_path.components().any(|part| {
                matches!(
                    part,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            });

            if unsafe_path {
                continue;
            }

            let start = entry.offset;
            let end = start.checked_add(entry.len).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "directory entry size overflow")
            })?;

            let contents = bytes.get(start..end).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("directory entry points outside payload: {}", entry.path),
                )
            })?;

            let dest = root.join(rel_path);

            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::write(&dest, contents)?;
            written += 1;
        }

        archive_files = Some(written);
        root.to_path_buf()
    } else if to_stdout {
        io::stdout().write_all(&bytes)?;
        PathBuf::from("stdout")
    } else {
        let path = resolve_output(output, &container.ext);
        fs::write(&path, &bytes)?;
        path
    };

    Ok(UnpackReport {
        input: if from_stdin {
            PathBuf::from("stdin")
        } else {
            Path::new(input).to_path_buf()
        },
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

pub fn help() -> Vec<String> {
    vec![
        "fossil unpack".header(),
        "restore the original data from a .fossil archive".bold(),
        "".into(),
        "usage".header(),
        "  fossil unpack <file.fossil> [output] [options]".into(),
        "".into(),
        "arguments".header(),
        "  <file.fossil>     archive to restore".into(),
        "  [output]          output file or directory".into(),
        "".into(),
        "options".header(),
        "  --trust           skip CRC verification before unpacking".into(),
        "".into(),
        "examples".header(),
        "  fossil unpack archive.fossil".into(),
        "  fossil unpack archive.fossil out/".into(),
        "  fossil unpack archive.fossil --trust".into(),
    ]
}
