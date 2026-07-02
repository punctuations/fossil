# fossil

![fossil](assets/banner.png)

I built fossil around one idea: a packed file should be able to tell you how it was
made. While it compresses, it writes down what it did, and `fossil explain` reads it
back.

## Install

You'll need [Rust](https://rustup.rs). Then install straight from the repo:

```sh
cargo install --git https://github.com/punctuations/fossil
```

This works on macOS, Linux, and Windows. From a clone, `./install.sh` (or
`cargo install --path .`) does the same, and `cargo build --release` leaves the
binary in `target/release`.

## Usage

```sh
fossil pack <input> [output]      # compress a file or directory → .fossil (no input packs the clipboard)
fossil lift                       # fossilize the clipboard, then copy the .fossil back
fossil unpack <file.fossil> [out] # restore the original (verifies CRC)
fossil inspect <file>             # per-block analysis: entropy, model, savings
fossil map <file>                 # entropy heatmap, or block models for a .fossil
fossil explain <file.fossil>      # the reconstruction recipe (--block N for one block)
```

Flags: `pack --lossy[=bits]` drops the low bits of each byte for a smaller file;
`--best-effort` packs already-compressed inputs losslessly instead of refusing, and
`--images-only` limits lossy to raw images. `pack --verify` round-trips before writing,
and `unpack --trust` skips the CRC check. `pack --fast` skips the slow models (BWT, PPM,
the audio predictor) and searches matches less deeply, so packing is much faster at some
cost to the ratio (audio and BWT-friendly text lose the most).

## Completions and man page

Shell completions and a man page live in `share/`.

```sh
source share/fossil.bash                                    # bash
cp share/fossil.zsh ~/.zfunc/_fossil                        # zsh (on your fpath)
cp share/fossil.fish ~/.config/fish/completions/            # fish
sudo cp share/fossil.1 /usr/local/share/man/man1/           # then: man fossil
```

## How it works

fossil cuts a file into 4 KB blocks and runs a handful of small models on each one,
keeping whichever output comes out smallest. The choice is written into the file, so
`fossil explain` can read it back block by block.

The models so far: RAW, RLE, Huffman, LZ, LZ+Huffman, LZR (LZ tokens range-coded with a
literal context, LZMA-style), BWT+MTF+range, adaptive range, order-1 PPM, a generator for
ramps and constant fills, a delta filter, CSV transpose, a word dictionary, and a FLAC-style
signal model (windowed adaptive LPC, mid/side stereo, partitioned Rice residuals) for 8/16/24-bit
audio and sensor data. The LZ
models can look back up to 64 KB into what they've already seen, so a repeat far from its
original only costs a pointer, not a second copy. Raw images (PPM and BMP) get filtered row
by row first (PNG-style, each row picks the filter that works best), so the models see small
differences instead of raw pixels.

Tiny or random files are stored as-is so they never grow, every file carries a CRC32 so
corruption shows up on unpack, and packing a directory makes one LZ pass over the whole
thing so duplicate files cost almost nothing. See [BENCHMARK.md](BENCHMARK.md) for how it
lands against gzip -9 and zstd -19 on the sample files.

## Format

A `.fossil` file is a small header, then the data. Integers marked `varint` are
unsigned LEB128 (7 bits per byte, low byte first).

| field | bytes | notes |
|---|---|---|
| magic | 4 | `FOSL` |
| version | 1 | currently 1 |
| mode | 1 | 0 = blocks, 1 = stored |
| filter | 1 | 0 = none, 1 = image (PNG-style rows) |
| ext length | 1 | length of the original extension |
| ext | n | the extension bytes (empty for clipboard input and directories) |
| original size | varint | length of the original input |
| crc32 | 4 | little-endian CRC32 of the original input |

Then the body depends on `mode`:

- **stored**: the original bytes, verbatim. Used when blocking wouldn't help
  (tiny, random, or already-compressed input), so a file never grows by more than
  the header.
- **blocks**: a `varint` block count, then each block as `model` (1 byte),
  `orig_len` (varint), `payload_len` (varint), and `payload_len` payload bytes.

To unpack, decode each block with its model and concatenate. If `filter` is set,
the result is the image residual stream; reversing the per-row filters gives the
original (the image header is kept at the front, so the geometry reads back from
it).

Each model has its own payload layout (RLE runs, Huffman and range tables, LZ
tokens, LPC coefficients plus Rice residuals for audio, and so on); the exact
encodings live in `src/core/models/`. A directory is packed by bundling its files
into one stream, running a single LZ pass over the whole thing, and storing that
as a file with the extension `/`.

## Complexity

Everything works on one 4 KB block at a time, so these are per-block bounds in the
block length `n` (at most 4096). The other letters are fixed constants: `A` is the
byte alphabet (256), `c` the LZ match-chain limit (128), `p` the LPC order (up to
32). Blocks are encoded in parallel across cores, and each block runs every model
and keeps the smallest.

| model | encode | decode |
|---|---|---|
| RAW, RLE, RANGE, PPM, GEN, DELTA, CSV, WORD | $O(n)$ | $O(n)$ |
| ENTROPY | $O(n + A)$ | $O(n)$ |
| LZ / LZH / LZR | $O(n\cdot c)$ | $O(n)$ |
| BWTM | $O(n \log(n) + n\cdot A)$ | $O(n\cdot A)$ |
| SIGNAL | $O(n\cdot p)$ per config, gated | $O(n \cdot p)$ |

Since `A`, `c`, and `p` are fixed, every model is linear in `n` except BWTM's
suffix sort, which is `n log n`. In practice the LZ family and BWTM dominate a
block; SIGNAL only runs when the cheaper models leave the block near its original
size (audio, sensor data), so its cost stays off the common path. The LZ family
also shares a single match-finder pass across LZ, LZH, and LZR rather than
repeating it three times.

Run `fossil help` for the full command list.
