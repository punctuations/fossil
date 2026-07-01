#!/usr/bin/env sh
set -e

cd "$(dirname "$0")"

if ! command -v cargo >/dev/null 2>&1; then
  echo "fossil needs Rust. Install it from https://rustup.rs and run this again." >&2
  exit 1
fi

echo "Building and installing fossil..."
cargo install --path . --force

echo "Installing completions and man page..."
data="${XDG_DATA_HOME:-$HOME/.local/share}"
mkdir -p "$data/man/man1" "$data/bash-completion/completions"
cp share/fossil.1 "$data/man/man1/fossil.1"
cp share/fossil.bash "$data/bash-completion/completions/fossil"

if command -v fish >/dev/null 2>&1; then
  mkdir -p "$HOME/.config/fish/completions"
  cp share/fossil.fish "$HOME/.config/fish/completions/fossil.fish"
fi

if command -v zsh >/dev/null 2>&1; then
  mkdir -p "$data/zsh/site-functions"
  cp share/fossil.zsh "$data/zsh/site-functions/_fossil"
fi

echo
echo "Installed. Run 'fossil help' to get started."
echo "If 'fossil' isn't found, add ~/.cargo/bin to your PATH."
if command -v zsh >/dev/null 2>&1; then
  echo "zsh: ensure $data/zsh/site-functions is on your fpath for completions."
fi
