# fossil

Structure-aware compression that explains how it rebuilds your files. A `.fossil`
is a per-block reconstruction, with reasoning built-in (see `fossil explain`).

## Build

```sh
cargo build --release
```

## Usage

```sh
fossil pack <input> [output]      # compress a file or directory → .fossil
fossil unpack <file.fossil> [out] # restore the original (verifies CRC)
fossil inspect <file>             # per-block analysis: entropy, model, savings
fossil map <file>                 # entropy heatmap
fossil explain <file.fossil>      # reconstruction recipe (--block N for detail)
```

Flags: `pack --lossy[=bits]` (quantize raw images), `pack --verify` (round-trip check).

## How it works

Files are split into blocks; each block is stored with the cheapest of several
models (RAW, RLE, Huffman, LZ, LZ+Huffman, BWT+MTF+range, adaptive range). Run
`fossil help` for the full command list.
