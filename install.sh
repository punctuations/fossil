#!/usr/bin/env sh
set -e

cd "$(dirname "$0")"

if ! command -v cargo >/dev/null 2>&1; then
  echo "fossil needs Rust. Install it from https://rustup.rs and run this again." >&2
  exit 1
fi

echo "Building and installing fossil..."
cargo install --path . --force

echo
echo "Installed. Run 'fossil help' to get started."
echo "If 'fossil' isn't found, add ~/.cargo/bin to your PATH."
