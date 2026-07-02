mod commands;
mod utils;

use std::env;
use std::io::IsTerminal;
use std::path::Path;
use std::process::ExitCode;
use utils::color::{Color, paint};

const FOSSIL_VER: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    dispatch();
    if utils::ui::had_error() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn dispatch() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        help();
        return;
    }

    let command = args[1].as_str();

    match command {
        "inspect" => {
            if args.len() != 3 {
                error!("inspect expects exactly one file");
                info!("fossil inspect <file>", usage = true);
                return;
            }

            commands::inspect::run(&args[2]);
        }

        // ize cause its like `fossil ize <i> <o>` like fossilize -- get it???
        "pack" | "bury" | "cover" | "ize" => {
            let (opts, verify, reveal, fast, pos) = parse_pack_flags(&args[2..]);

            match pos.len() {
                0 => {
                    if !std::io::stdin().is_terminal() {
                        if std::io::stdout().is_terminal() {
                            error!(
                                "refusing to write a .fossil to the terminal, redirect it like `... | fossil pack > out.fossil`"
                            );
                        } else {
                            commands::pack::run("-", "-", opts, verify, fast);
                        }
                    } else {
                        commands::pack::run_clipboard(None, opts, verify, reveal, fast);
                    }
                }
                1 => {
                    let input = pos[0];
                    let output = Path::new(input.trim_end_matches('/'))
                        .file_stem()
                        .and_then(|name| name.to_str())
                        .unwrap_or("output");
                    commands::pack::run(input, output, opts, verify, fast);
                }
                2 => commands::pack::run(pos[0], pos[1], opts, verify, fast),
                _ => {
                    error!("pack expects an input and output file");
                    info!(
                        "fossil pack [--lossy[=bits]] [--best-effort] [--images-only] [--verify] <input> <output>",
                        usage = true
                    );
                }
            }
        }

        "lift" | "c-v" | "c/v" | "cv" => {
            let (opts, verify, reveal, fast, pos) = parse_pack_flags(&args[2..]);
            commands::pack::run_clipboard(
                pos.first().map(|s| s.as_str()),
                opts,
                verify,
                reveal,
                fast,
            );
        }

        // should add another short alias
        "unpack" | "recover" | "exhume" | "uncover" => {
            let mut trust = false;
            let mut pos: Vec<&String> = Vec::new();
            for a in &args[2..] {
                if a == "--trust" {
                    trust = true;
                } else {
                    pos.push(a);
                }
            }

            match pos.len() {
                0 if !std::io::stdin().is_terminal() => {
                    commands::unpack::run("-", "-", trust);
                }
                1 => commands::unpack::run(pos[0], &pos[0].replace(".fossil", ""), trust),
                2 => commands::unpack::run(pos[0], pos[1], trust),
                _ => {
                    error!("unpack expects an input file and output file");
                    info!(
                        "fossil unpack [--trust] <input.fossil> <output>",
                        usage = true
                    );
                }
            }
        }

        "explain" | "why" | "whats" | "describe" => {
            let mut block: Option<usize> = None;
            let mut pos: Vec<&String> = Vec::new();
            let mut i = 2;
            while i < args.len() {
                if args[i] == "--block" {
                    block = args.get(i + 1).and_then(|s| s.parse().ok());
                    i += 2;
                } else if let Some(n) = args[i].strip_prefix("--block=") {
                    block = n.parse().ok();
                    i += 1;
                } else {
                    pos.push(&args[i]);
                    i += 1;
                }
            }
            if pos.len() != 1 {
                error!("explain expects one fossil file");
                info!("fossil explain [--block N] <file.fossil>", usage = true);
                return;
            }
            commands::explain::run(pos[0], block);
        }

        "map" => {
            if args.len() != 3 {
                error!("map expects exactly one file");
                info!("fossil map <file>", usage = true);
                return;
            }

            commands::map::run(&args[2]);
        }

        "verify" | "check" => {
            if args.len() != 3 {
                error!("verify expects exactly one file");
                info!("fossil verify <file.fossil>", usage = true);
                return;
            }

            commands::verify::run(&args[2]);
        }

        "update" | "upgrade" => {
            let mut completions = false;
            let mut man = false;

            if args.len() > 2 {
                for a in args {
                    if a == "--completions" {
                        completions = true;
                    } else if a == "--man" {
                        man = true;
                    }
                }
            }

            commands::update::run(completions, man);
        }

        "help" | "--help" | "-h" | "?" => {
            help();
        }

        "--version" | "version" | "ver" | "--ver" | "-v" | "-V" => {
            let commit = env!("FOSSIL_COMMIT");
            if commit.is_empty() {
                println!("fossil v{}", FOSSIL_VER);
            } else {
                let short = &commit[..commit.len().min(7)];
                println!(
                    "fossil v{} {}",
                    FOSSIL_VER,
                    paint(&format!("· {}", short), "38;5;244")
                );
            }
        }

        unknown => {
            error!("unknown command `{}`", unknown);
            match closest(unknown) {
                Some(guess) => {
                    n!();
                    println!("  did you mean {}?", guess.accent());
                    n!();
                }
                None => {
                    n!();
                    help();
                }
            }
        }
    }
}

const COMMANDS: &[&str] = &[
    "pack", "lift", "unpack", "inspect", "map", "explain", "verify", "update", "help",
];

fn closest(input: &str) -> Option<&'static str> {
    COMMANDS
        .iter()
        .map(|&c| (c, levenshtein(input, c)))
        .min_by_key(|&(_, d)| d)
        .filter(|&(c, d)| d <= c.len() / 2 + 1)
        .map(|(c, _)| c)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

fn parse_pack_flags(
    args: &[String],
) -> (commands::pack::LossyOpts, bool, bool, bool, Vec<&String>) {
    let mut bits: Option<u8> = None;
    let mut verify = false;
    let mut best_effort = false;
    let mut images_only = false;
    let mut reveal = false;
    let mut fast = false;
    let mut pos: Vec<&String> = Vec::new();
    for a in args {
        if let Some(rest) = a.strip_prefix("--lossy") {
            let k = rest
                .strip_prefix('=')
                .and_then(|s| s.parse::<u8>().ok())
                .unwrap_or(3);
            bits = Some(k);
        } else if a == "--verify" {
            verify = true;
        } else if a == "--best-effort" {
            best_effort = true;
        } else if a == "--images-only" {
            images_only = true;
        } else if a == "--reveal" {
            reveal = true;
        } else if a == "--fast" {
            fast = true;
        } else {
            pos.push(a);
        }
    }
    (
        commands::pack::LossyOpts {
            bits,
            best_effort,
            images_only,
        },
        verify,
        reveal,
        fast,
        pos,
    )
}

fn help() {
    let bone = |s: &str| paint(s, "38;5;180");
    let art = r"
⠀⠀⠀⠀⠀⠀⢀⣤⠀⣠⣾⡆⢀⣴⣶⠀⠀⠀⣀⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⢠⡀⣸⣿⡇⠻⠿⠇⠻⠿⠷⢠⣶⣿⡟⠀⣀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⢸⣷⠀⣠⣴⣶⣿⣿⣿⣷⣶⣦⣉⡛⡀⠿⡿⢀⡄⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⡘⢗⣤⣾⣿⢻⢹⡅⡇⡏⢺⣿⣿⣿⠿⢾⢶⢤⣬⣡⣀⣀⠀⣆⢔⣄⡴
⣀⣴⣶⠾⠿⢁⡾⡝⡸⠀⡇⡇⠇⣸⠛⡟⠂⠀⠀⠀⠈⠀⠉⠙⠛⠛⠛⠛⠉⠀
⠈⠉⠁⠀⠀⢸⠇⢱⠀⠀⠁⠁⢠⡧⠀⢷⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠜⠀⠼⠀⠀⠀⠀⠰⠇⠀⣸⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀";
    let info = [
        String::new(),
        format!(
            "{} {}",
            "fossil".accent().bold(),
            format!("v{}", FOSSIL_VER).accent()
        ),
        "Fossilize your data, become a data archeologist".reset(),
        String::new(),
        "usage:".header().bold(),
        "  fossil <command> [args]".to_string(),
        String::new(),
    ];
    let lines: Vec<&str> = art.trim_matches('\n').lines().collect();
    n!();
    for (i, line) in lines.iter().enumerate() {
        let right = info.get(i).cloned().unwrap_or_default();
        println!("{}   {}", bone(line), right);
    }
    n!();
    println!("{}", "commands:".header().bold());
    println!(
        "  {}  per-block analysis (entropy, model, savings)",
        "inspect <file>           ".header()
    );
    println!(
        "  {}  entropy heatmap, or block models for a .fossil",
        "map <file>               ".header()
    );
    println!(
        "  {}  compress a file or directory (no input → the clipboard)",
        "pack <input> [output]    ".header()
    );
    println!(
        "  {}  fossilize the clipboard, copy the .fossil back",
        "lift                     ".header()
    );
    println!(
        "  {}  restore the original (verifies CRC)",
        "unpack <file> [output]   ".header()
    );
    println!(
        "  {}  show the reconstruction recipe",
        "explain <file.fossil>    ".header()
    );
    println!(
        "  {}  check a fossil's CRC without unpacking",
        "verify <file.fossil>     ".header()
    );
    println!(
        "  {}  reinstall the latest fossil from git",
        "update                   ".header()
    );
    println!("  {}  this message", "help                     ".header());
    n!();
    println!("{}", "flags:".header().bold());
    println!(
        "  {}  lossy quantization (drops low bits of each byte)",
        "pack --lossy[=bits]      ".header()
    );
    println!(
        "  {}  pack already-compressed inputs lossless instead of refusing",
        "pack --best-effort       ".header()
    );
    println!(
        "  {}  only apply lossy to raw image formats",
        "pack --images-only       ".header()
    );
    println!(
        "  {}  verify round-trip before writing",
        "pack --verify            ".header()
    );
    println!(
        "  {}  skip the slow models for faster packing",
        "pack --fast              ".header()
    );
    println!(
        "  {}  install and/or update man pages for fossil",
        "update --man             ".header()
    );
    println!(
        "  {}  install and/or update completions for fish, zsh, and bash",
        "update --completions     ".header()
    );
    println!(
        "  {}  deep-dive a single block",
        "explain --block N        ".header()
    );
    println!(
        "  {}  skip the CRC check on unpack",
        "unpack --trust           ".header()
    );
    println!(
        "  {}  reveal the .fossil in the file manager after lift",
        "lift --reveal            ".header()
    );
    n!();
    println!("{}", "examples:".header().bold());
    println!("  fossil pack src/ archive");
    println!("  fossil lift                          (pack whatever you just copied)");
    println!("  fossil unpack archive.fossil out");
    println!("  fossil inspect main.rs");
    println!("  fossil explain archive.fossil --block 3");
    println!("  cat foo.png | fossil pack > foo.fossil");
}
