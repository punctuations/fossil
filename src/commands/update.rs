use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::utils::color::{Color, paint};
use crate::{error, n};

const REPO: &str = "https://github.com/punctuations/fossil";
const MAN_URL: &str = "https://fossilize.vercel.app/man";
const MAN_PAGE: &str = "fossil.1";

const BASH_COMPLETION_URL: &str = "https://fossilize.vercel.app/bash";
const ZSH_COMPLETION_URL: &str = "https://fossilize.vercel.app/zsh";
const FISH_COMPLETION_URL: &str = "https://fossilize.vercel.app/fish";

pub fn run(completions: bool, man: bool) {
    if completions {
        match install_completions() {
            Ok(paths) => {
                n!();
                println!("  {} installed completions", "✓".header());

                for path in paths {
                    println!("    {}", path.display());
                }

                n!();
            }
            Err(e) => error!("{}", e),
        }
        return;
    }

    if man {
        match install_man_page() {
            Ok(path) => {
                n!();
                println!(
                    "  {} installed man page to {}",
                    "✓".header(),
                    path.display()
                );
                println!("  {} try: {}", "usage:".header(), "man fossil".accent());
                n!();
            }
            Err(e) => error!("{}", e),
        }
        return;
    }

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

fn install_man_page() -> io::Result<PathBuf> {
    #[cfg(windows)]
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "man pages are not supported on Windows",
        ));
    }

    #[cfg(not(windows))]
    {
        let bytes = fetch_url(MAN_URL)?;

        let mut tried = Vec::new();

        for root in man_roots() {
            let dir = root.join("man1");
            let dest = dir.join(MAN_PAGE);

            if let Err(e) = fs::create_dir_all(&dir) {
                tried.push(format!("{} ({})", dir.display(), e));
                continue;
            }

            match fs::write(&dest, &bytes) {
                Ok(_) => return Ok(dest),
                Err(e) => tried.push(format!("{} ({})", dest.display(), e)),
            }
        }

        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "could not write to any man directory. tried: {}",
                tried.join(", ")
            ),
        ))
    }
}

fn install_completions() -> io::Result<Vec<PathBuf>> {
    #[cfg(windows)]
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "shell completions are not supported on Windows",
        ));
    }

    #[cfg(not(windows))]
    {
        let bash = fetch_url(BASH_COMPLETION_URL)?;
        let zsh = fetch_url(ZSH_COMPLETION_URL)?;
        let fish = fetch_url(FISH_COMPLETION_URL)?;

        let mut installed = Vec::new();
        let mut errors = Vec::new();

        try_install_completion(&bash_completion_paths(), &bash, &mut installed, &mut errors);

        try_install_completion(&zsh_completion_paths(), &zsh, &mut installed, &mut errors);

        try_install_completion(&fish_completion_paths(), &fish, &mut installed, &mut errors);

        if installed.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "could not write any completion files. tried: {}",
                    errors.join(", ")
                ),
            ));
        }

        Ok(installed)
    }
}

#[cfg(not(windows))]
fn try_install_completion(
    paths: &[PathBuf],
    bytes: &[u8],
    installed: &mut Vec<PathBuf>,
    errors: &mut Vec<String>,
) {
    for path in paths {
        let Some(dir) = path.parent() else {
            continue;
        };

        if let Err(e) = fs::create_dir_all(dir) {
            errors.push(format!("{} ({})", dir.display(), e));
            continue;
        }

        match fs::write(path, bytes) {
            Ok(_) => {
                installed.push(path.clone());
                return;
            }
            Err(e) => errors.push(format!("{} ({})", path.display(), e)),
        }
    }
}

#[cfg(not(windows))]
fn bash_completion_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);

        push_unique(
            &mut paths,
            home.join(".local/share/bash-completion/completions/fossil"),
        );
    }

    push_unique(
        &mut paths,
        PathBuf::from("/opt/homebrew/etc/bash_completion.d/fossil"),
    );

    push_unique(
        &mut paths,
        PathBuf::from("/usr/local/etc/bash_completion.d/fossil"),
    );

    push_unique(
        &mut paths,
        PathBuf::from("/usr/share/bash-completion/completions/fossil"),
    );

    paths
}

#[cfg(not(windows))]
fn zsh_completion_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);

        push_unique(&mut paths, home.join(".zsh/completions/_fossil"));
        push_unique(
            &mut paths,
            home.join(".local/share/zsh/site-functions/_fossil"),
        );
    }

    push_unique(
        &mut paths,
        PathBuf::from("/opt/homebrew/share/zsh/site-functions/_fossil"),
    );

    push_unique(
        &mut paths,
        PathBuf::from("/usr/local/share/zsh/site-functions/_fossil"),
    );

    push_unique(
        &mut paths,
        PathBuf::from("/usr/share/zsh/site-functions/_fossil"),
    );

    paths
}

#[cfg(not(windows))]
fn fish_completion_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);

        push_unique(
            &mut paths,
            home.join(".config/fish/completions/fossil.fish"),
        );
    }

    push_unique(
        &mut paths,
        PathBuf::from("/opt/homebrew/share/fish/vendor_completions.d/fossil.fish"),
    );

    push_unique(
        &mut paths,
        PathBuf::from("/usr/local/share/fish/vendor_completions.d/fossil.fish"),
    );

    push_unique(
        &mut paths,
        PathBuf::from("/usr/share/fish/vendor_completions.d/fossil.fish"),
    );

    paths
}

#[cfg(not(windows))]
fn fetch_url(url: &str) -> io::Result<Vec<u8>> {
    let out = Command::new("curl").args(["-fsSL", url]).output()?;

    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr);
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("{}", msg.trim()),
        ));
    }

    Ok(out.stdout)
}

#[cfg(not(windows))]
fn man_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(manpath) = std::env::var("MANPATH") {
        for path in manpath.split(':').filter(|p| !p.is_empty()) {
            push_unique(&mut roots, PathBuf::from(path));
        }
    }

    if let Ok(out) = Command::new("manpath").output() {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for path in text.trim().split(':').filter(|p| !p.is_empty()) {
                push_unique(&mut roots, PathBuf::from(path));
            }
        }
    }

    if let Some(home) = std::env::var_os("HOME") {
        push_unique(&mut roots, PathBuf::from(home).join(".local/share/man"));
    }

    push_unique(&mut roots, PathBuf::from("/usr/local/share/man"));
    push_unique(&mut roots, PathBuf::from("/opt/homebrew/share/man"));
    push_unique(&mut roots, PathBuf::from("/usr/share/man"));

    roots
}

#[cfg(not(windows))]
fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|p| p == &path) {
        paths.push(path);
    }
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
