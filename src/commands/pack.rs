use std::fs;
use std::io;
use std::path::Path;

use fossil::core::{bundle, container, lossy};

use crate::utils::color::{Color, paint};
use crate::utils::spinner::Spinner;
use crate::{error, n};

pub fn run(input: &str, output: &str, lossy_bits: Option<u8>, verify: bool) {
    let sp = Spinner::start("fossilizing…");
    let result = pack(input, output, lossy_bits, verify);
    sp.stop();
    match result {
        Ok(r) => {
            let delta = if r.raw_size == 0 {
                String::new()
            } else {
                let pct = (1.0 - r.packed_size as f64 / r.raw_size as f64) * 100.0;
                if pct >= 0.0 {
                    format!("  {:.1}% smaller", pct).header()
                } else {
                    format!("  {:.1}% larger", -pct).coral()
                }
            };

            n!();
            println!(
                "  {} {} {}",
                r.input.display().to_string().accent(),
                "→".bold(),
                r.output.display().to_string().accent(),
            );
            println!(
                "  {} → {} bytes{}",
                r.raw_size,
                r.packed_size.to_string().bold(),
                delta,
            );
            if let Some(k) = r.lossy {
                println!(
                    "  {}",
                    paint(
                        &format!("lossy · dropped {} low bit(s)/byte", k),
                        "38;5;244"
                    )
                );
            }
            n!();
        }
        Err(err) => error!("{}", err),
    }
}

struct PackReport {
    input: std::path::PathBuf,
    output: std::path::PathBuf,
    raw_size: u64,
    packed_size: u64,
    lossy: Option<u8>,
}

fn collect_files(dir: &Path, base: &Path, out: &mut Vec<(String, Vec<u8>)>) -> io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, base, out)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let data = fs::read(&path)?;
            out.push((rel, data));
        }
    }

    Ok(())
}

fn pack(input: &str, output: &str, lossy_bits: Option<u8>, verify: bool) -> io::Result<PackReport> {
    let input_path = Path::new(input);
    let output_str = if output.ends_with(".fossil") {
        output.to_string()
    } else {
        format!("{}{}", output, ".fossil")
    };
    let output_path = Path::new(&output_str);

    let (bytes, ext, raw) = if input_path.is_dir() {
        let mut files = Vec::new();
        collect_files(input_path, input_path, &mut files)?;
        if let Some(k) = lossy_bits {
            for (_, data) in files.iter_mut() {
                if lossy::compressed_format(data).is_none() {
                    *data = lossy::quantize(data, k);
                }
            }
        }
        let raw: u64 = files.iter().map(|(_, d)| d.len() as u64).sum();
        (bundle::pack(&files), "/".to_string(), raw)
    } else if input_path.is_file() {
        let mut bytes = fs::read(input_path)?;
        if let Some(k) = lossy_bits {
            if let Some(fmt) = lossy::compressed_format(&bytes) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    // will fuck up existing crc check with lossy, refuse
                    format!("--lossy can't be applied to {} (already compressed)", fmt),
                ));
            }
            bytes = lossy::quantize_content(&bytes, k);
        }
        let ext = input_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
        let raw = bytes.len() as u64;
        (bytes, ext, raw)
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("input path does not exist: {}", input_path.display()),
        ));
    };

    let fossil = container::write(&bytes, &ext);
    if verify {
        let check = container::read(&fossil)?;
        if check.decode() != bytes {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "verify failed: round-trip did not match input",
            ));
        }
    }
    fs::write(output_path, &fossil)?;

    Ok(PackReport {
        input: input_path.to_path_buf(),
        output: output_path.to_path_buf(),
        raw_size: raw,
        packed_size: fossil.len() as u64,
        lossy: lossy_bits,
    })
}
