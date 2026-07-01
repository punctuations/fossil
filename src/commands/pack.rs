use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use fossil::core::{biglz, bundle, container, lossy};

use crate::utils::clipboard;
use crate::utils::color::{Color, link, paint};
use crate::utils::spinner::Spinner;
use crate::{error, n};

pub struct LossyOpts {
    pub bits: Option<u8>,
    pub best_effort: bool,
    pub images_only: bool,
}

enum Lossy {
    Quantize,
    Skip,
    Refuse,
}

fn lossy_decision(data: &[u8], opts: &LossyOpts) -> Lossy {
    if opts.images_only {
        if lossy::raw_image_format(data).is_some() {
            Lossy::Quantize
        } else {
            Lossy::Skip
        }
    } else if lossy::compressed_format(data).is_some() {
        if opts.best_effort {
            Lossy::Skip
        } else {
            Lossy::Refuse
        }
    } else {
        Lossy::Quantize
    }
}

pub fn run(input: &str, output: &str, lossy: LossyOpts, verify: bool) {
    let progress = Arc::new(container::Progress::default());
    let sp = {
        let p = progress.clone();
        Spinner::progress(move || {
            let total = p.total.load(Ordering::Relaxed);
            if total == 0 {
                "fossilizing…".to_string()
            } else {
                let done = p.done.load(Ordering::Relaxed).min(total);
                format!("fossilizing… ({}/{})", done, total)
            }
        })
    };
    let result = pack(input, output, &lossy, verify, Some(progress.as_ref()));
    sp.stop();
    match result {
        Ok(r) => {
            if output != "-" {
                print_report(&r);
                n!();
            }
        }
        Err(err) => error!("{}", err),
    }
}

pub fn run_clipboard(output: Option<&str>, lossy: LossyOpts, verify: bool, reveal: bool) {
    let sp = Spinner::start("lifting…");
    let result = pack_clipboard(output, &lossy, verify);
    sp.stop();
    match result {
        Ok(r) => {
            print_report(&r);
            let sp2 = Spinner::dim("copying to clipboard…");
            let copied = clipboard::copy(&r.output);
            if reveal {
                let _ = clipboard::reveal(&r.output);
            }
            sp2.stop();
            match copied {
                Ok(()) => println!("  {}", paint("copied to clipboard", "38;5;244")),
                Err(e) => println!(
                    "  {}",
                    paint(
                        &format!("packed, but clipboard copy failed: {}", e),
                        "38;5;173"
                    )
                ),
            }
            n!();
        }
        Err(err) => error!("{}", err),
    }
}

fn print_report(r: &PackReport) {
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
    if r.packed_size > r.raw_size {
        let header = r.packed_size.saturating_sub(r.payload_bytes);
        if r.payload_bytes == r.raw_size {
            // raw, not compressed
            println!(
                "  {}",
                paint(
                    &format!("{} raw bytes + {} bytes of header", r.payload_bytes, header),
                    "38;5;244"
                )
            );
        } else {
            // compressed
            println!(
                "  {}",
                paint(
                    &format!(
                        "{} compressed bytes + {} bytes of header",
                        r.payload_bytes, header
                    ),
                    "38;5;244"
                )
            );
        }
    }
    if let Some(k) = r.lossy {
        println!(
            "  {}",
            paint(
                &format!("lossy · dropped {} low bit(s)/byte", k),
                "38;5;244"
            )
        );
    }
}

fn pack_clipboard(
    output: Option<&str>,
    lossy: &LossyOpts,
    verify: bool,
) -> io::Result<PackReport> {
    let (bytes, ext) = clipboard::paste()?;
    let in_ext = if ext.is_empty() { "bin" } else { ext.as_str() };
    let tmp_in = std::env::temp_dir().join(format!("fossil-clipboard-input.{}", in_ext));
    fs::write(&tmp_in, &bytes)?;

    let out: String = match output {
        Some(o) => o.to_string(),
        None => std::env::temp_dir()
            .join("clipboard")
            .to_string_lossy()
            .into_owned(),
    };

    let input = tmp_in.to_string_lossy().into_owned();
    let result = pack(&input, &out, lossy, verify, None);
    let _ = fs::remove_file(&tmp_in);

    let mut report = result?;
    report.input = std::path::PathBuf::from("clipboard");
    Ok(report)
}

struct PackReport {
    input: std::path::PathBuf,
    output: std::path::PathBuf,
    raw_size: u64,
    packed_size: u64,
    payload_bytes: u64,
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

fn pack(
    input: &str,
    output: &str,
    opts: &LossyOpts,
    verify: bool,
    progress: Option<&container::Progress>,
) -> io::Result<PackReport> {
    let input_path = Path::new(input);
    let to_stdout = output == "-";
    let output_str = if to_stdout || output.ends_with(".fossil") {
        output.to_string()
    } else {
        format!("{}{}", output, ".fossil")
    };
    let output_path = Path::new(&output_str);

    let (bytes, ext, raw, applied) = if input == "-" {
        let mut bytes = Vec::new();
        io::stdin().read_to_end(&mut bytes)?;
        let mut applied = None;
        if let Some(k) = opts.bits {
            match lossy_decision(&bytes, opts) {
                Lossy::Quantize => {
                    bytes = lossy::quantize_content(&bytes, k);
                    applied = Some(k);
                }
                Lossy::Skip => {}
                Lossy::Refuse => {
                    let fmt = lossy::compressed_format(&bytes).unwrap_or("this");
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "{} is already compressed. --best-effort packs it lossless ({})",
                            fmt,
                            link("why?", "https://fossilize.vercel.app/examples")
                        ),
                    ));
                }
            }
        }
        let raw = bytes.len() as u64;
        (bytes, String::new(), raw, applied)
    } else if input_path.is_dir() {
        let mut files = Vec::new();
        collect_files(input_path, input_path, &mut files)?;
        let mut applied = None;
        if let Some(k) = opts.bits {
            for (rel, data) in files.iter_mut() {
                match lossy_decision(data, opts) {
                    Lossy::Quantize => {
                        *data = lossy::quantize_content(data, k);
                        applied = Some(k);
                    }
                    Lossy::Skip => {}
                    Lossy::Refuse => {
                        let fmt = lossy::compressed_format(data).unwrap_or("compressed");
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!(
                                "{} is already compressed ({}). use --best-effort or --images-only ({})",
                                rel,
                                fmt,
                                link("why?", "https://fossilize.vercel.app/examples")
                            ),
                        ));
                    }
                }
            }
        }
        let raw: u64 = files.iter().map(|(_, d)| d.len() as u64).sum();
        let bundle = bundle::pack(&files);
        let mut wrapped = Vec::new();
        fossil::core::varint::write(&mut wrapped, bundle.len());
        wrapped.extend_from_slice(&biglz::encode(&bundle));
        (wrapped, "/".to_string(), raw, applied)
    } else if input_path.is_file() {
        let mut bytes = fs::read(input_path)?;
        let mut applied = None;
        if let Some(k) = opts.bits {
            match lossy_decision(&bytes, opts) {
                Lossy::Quantize => {
                    bytes = lossy::quantize_content(&bytes, k);
                    applied = Some(k);
                }
                Lossy::Skip => {}
                Lossy::Refuse => {
                    // will fuck up existing crc check with lossy, refuse
                    let fmt = lossy::compressed_format(&bytes).unwrap_or("this");
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "{} is already compressed. --best-effort packs it lossless ({})",
                            fmt,
                            link("why?", "https://fossilize.vercel.app/examples")
                        ),
                    ));
                }
            }
        }
        let ext = input_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
        let raw = bytes.len() as u64;
        (bytes, ext, raw, applied)
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("input path does not exist: {}", input_path.display()),
        ));
    };

    let fossil = container::write_progress(&bytes, &ext, progress);
    if verify {
        let check = container::read(&fossil)?;
        if check.decode() != bytes {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "verify failed: round-trip did not match input",
            ));
        }
    }
    if to_stdout {
        io::stdout().write_all(&fossil)?;
    } else {
        fs::write(output_path, &fossil)?;
    }

    let payload_bytes: u64 = container::read(&fossil)
        .map(|c| c.blocks.iter().map(|b| b.payload.len() as u64).sum())
        .unwrap_or(0);

    Ok(PackReport {
        input: if input == "-" {
            std::path::PathBuf::from("stdin")
        } else {
            input_path.to_path_buf()
        },
        output: if to_stdout {
            std::path::PathBuf::from("stdout")
        } else {
            output_path.to_path_buf()
        },
        raw_size: raw,
        packed_size: fossil.len() as u64,
        payload_bytes,
        lossy: applied,
    })
}
