use std::io::{self, IsTerminal, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

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

    n!();
    match install() {
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
    let log = std::env::temp_dir().join("fossil-update.log");
    let f = std::fs::File::create(&log)?;
    let mut child = Command::new("cargo")
        .args(["install", "--git", REPO, "--force"])
        .stdout(Stdio::null())
        .stderr(Stdio::from(f))
        .spawn()?;

    let success;
    if io::stderr().is_terminal() {
        let width = 22usize;
        let block = 4usize;
        let max = (width - block) as i32;
        let mut head: i32 = 0;
        let mut dir: i32 = 1;
        loop {
            if let Some(status) = child.try_wait()? {
                eprint!("\r\x1b[2K");
                let _ = io::stderr().flush();
                success = status.success();
                break;
            }
            eprint!("\r{}", bar(width, block, head as usize));
            let _ = io::stderr().flush();
            head += dir;
            if head <= 0 || head >= max {
                dir = -dir;
                head = head.clamp(0, max);
            }
            thread::sleep(Duration::from_millis(70));
        }
    } else {
        success = child.wait()?.success();
    }

    if !success {
        if let Ok(content) = std::fs::read_to_string(&log) {
            let tail: Vec<&str> = content.lines().rev().take(8).collect();
            for line in tail.iter().rev() {
                eprintln!("  {}", paint(line, "38;5;244"));
            }
        }
    }
    let _ = std::fs::remove_file(&log);
    Ok(success)
}

fn bar(width: usize, block: usize, head: usize) -> String {
    let head = head.min(width.saturating_sub(block));
    let pre = "░".repeat(head);
    let blk = "█".repeat(block);
    let post = "░".repeat(width - head - block);
    format!(
        "  {} {}{}{}{}{}",
        "updating".header(),
        paint("[", "38;5;240"),
        paint(&pre, "38;5;240"),
        paint(&blk, "38;5;180"),
        paint(&post, "38;5;240"),
        paint("]", "38;5;240"),
    )
}
