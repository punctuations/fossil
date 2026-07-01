use std::io::{self, BufRead, IsTerminal, Write};
use std::process::{Command, Stdio};

use crate::utils::color::{Color, paint};
use crate::{error, n};

const REPO: &str = "https://github.com/punctuations/fossil";

pub fn run() {
    if Command::new("cargo").arg("--version").output().is_err() {
        error!("update needs cargo — install Rust from https://rustup.rs and try again");
        return;
    }

    let current = env!("FOSSIL_COMMIT");
    let latest = remote_head();

    if let Some(latest) = latest.as_deref() {
        if !current.is_empty() && latest == current {
            n!();
            println!("  {} already up to date ({})", "✓".header(), short(current));
            n!();
            return;
        }
    }

    #[cfg(windows)]
    let moved = move_self_aside();

    n!();
    let result = install();

    #[cfg(windows)]
    if !matches!(result, Ok(true)) {
        if let Some((exe, old)) = &moved {
            let _ = std::fs::rename(old, exe);
        }
    }

    match result {
        Ok(true) => {
            let to = latest.as_deref().map(short).unwrap_or("");
            if to.is_empty() {
                println!("  {} fossil updated", "✓".header());
            } else {
                println!("  {} fossil updated to {}", "✓".header(), to);
            }
        }
        Ok(false) => error!("update failed"),
        Err(e) => error!("couldn't run cargo: {}", e),
    }
    n!();
}

#[cfg(windows)]
fn move_self_aside() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let exe = std::env::current_exe().ok()?;
    let old = exe.with_extension("old");
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&exe, &old).ok()?;
    Some((exe, old))
}

fn short(hash: &str) -> &str {
    &hash[..hash.len().min(7)]
}

fn remote_head() -> Option<String> {
    let out = Command::new("git")
        .args(["ls-remote", REPO, "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.split_whitespace().next().map(|h| h.to_string())
}

fn install() -> io::Result<bool> {
    let mut child = Command::new("cargo")
        .args(["install", "--git", REPO, "--force"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;

    let tty = io::stderr().is_terminal();
    let mut log: Vec<String> = Vec::new();
    let mut steps = 0usize;

    if let Some(out) = child.stderr.take() {
        for line in io::BufReader::new(out).lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            let head = line.trim_start();
            if head.starts_with("Compiling")
                || head.starts_with("Downloading")
                || head.starts_with("Downloaded")
                || head.starts_with("Updating")
                || head.starts_with("Installing")
            {
                steps += 1;
                if tty {
                    let frac = 1.0 - 0.6_f64.powf(steps as f64 / 4.0);
                    eprint!("\r{}", bar(frac));
                    let _ = io::stderr().flush();
                }
            }
            log.push(line);
        }
    }

    let success = child.wait()?.success();
    if tty {
        eprint!("\r\x1b[2K");
        let _ = io::stderr().flush();
    }

    if !success {
        let tail: Vec<&String> = log.iter().rev().take(8).collect();
        for line in tail.into_iter().rev() {
            eprintln!("  {}", paint(line, "38;5;244"));
        }
    }
    Ok(success)
}

fn bar(frac: f64) -> String {
    let width = 22usize;
    let frac = frac.clamp(0.0, 1.0);
    let filled = (frac * width as f64).round() as usize;
    let fill = "█".repeat(filled);
    let rest = "░".repeat(width - filled);
    format!(
        "  {} {}{}{}{} {:>3}%",
        "updating".header(),
        paint("[", "38;5;240"),
        paint(&fill, "38;5;180"),
        paint(&rest, "38;5;240"),
        paint("]", "38;5;240"),
        (frac * 100.0).round() as u32,
    )
}
