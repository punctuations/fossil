use std::process::Command;

fn main() {
    let commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    println!("cargo:rustc-env=FOSSIL_COMMIT={commit}");
    println!("cargo:rerun-if-changed=.git/HEAD");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos")
        && std::env::var("CARGO_FEATURE_MOUNT").is_ok()
    {
        println!("cargo:rustc-link-arg-bins=-Wl,-weak-lfuse");
    }
}
