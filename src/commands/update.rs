use std::process::Command;

use crate::utils::color::Color;
use crate::{error, n};

const REPO: &str = "https://github.com/punctuations/fossil";

pub fn run() {
    if Command::new("cargo").arg("--version").output().is_err() {
        error!("update needs cargo — install Rust from https://rustup.rs and try again");
        return;
    }

    n!();
    println!(
        "  {} fossil from {}",
        "updating".header(),
        REPO.accent()
    );
    n!();

    let status = Command::new("cargo")
        .args(["install", "--git", REPO, "--force"])
        .status();

    match status {
        Ok(s) if s.success() => {
            n!();
            println!("  {} fossil is up to date", "✓".header());
            n!();
        }
        Ok(_) => error!("update failed (cargo install returned an error)"),
        Err(e) => error!("couldn't run cargo: {}", e),
    }
}
