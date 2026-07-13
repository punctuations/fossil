# benchmark

fossil against `gzip -9` and `zstd -19` on the example files. Run it yourself:

```sh
./bench.sh
```

| file | original | fossil | gzip -9 | zstd -19 |
|---|---|---|---|---|
| z | 100000 | 170 (99.8%) | 134 (99.9%) | 25 (100.0%) |
| mixed.bin | 360448 | 75670 (79.0%) | 93399 (74.1%) | 82787 (77.0%) |
| bigmix.bin | 901120 | 188742 (79.1%) | 228264 (74.7%) | 204357 (77.3%) |
| cat.ppm | 1848015 | 496127 (73.2%) | 904590 (51.1%) | 781646 (57.7%) |
| wave.pcm | 300000 | 56950 (81.0%) | 294794 (1.7%) | 293048 (2.3%) |
| cat.jpg | 60055 | 60065 (-0.0%) | 60059 (-0.0%) | 60069 (-0.0%) |

These numbers are only for these files. Other data would land somewhere else. Roughly why each row does what it does: the mixed files do well because fossil picks a model per 4 KB block, which fits data that keeps changing as you read it. The image gets filtered row by row first (PNG-style), so the models see small differences instead of raw pixels. wave.pcm is audio, so fossil fits a predictor to each block the way FLAC does and codes what's left over. gzip and zstd don't predict audio, so they barely touch it. cat.jpg is already compressed, so there's nothing left to take.

The blocks are still 4 KB, but the LZ models can look back up to 256 KB into what they've already seen, so a repeat far from the original only costs a pointer instead of a second copy. One of them, LZR2, codes those pointers like LZMA: each byte gets its own adaptive context, and reusing the last match distance costs one bit. That's what helps the mixed files.

## Directories

Directories trade some ratio for random access. Files are packed into one stream, and
the LZ window resets every 256 KB, so `take` and `mount` can decode one file without
touching the rest. Matches can't cross a segment boundary; that's the whole cost.
Compared against a `tar` of the same directory:

| directory | tar bytes | fossil | gzip -9 | zstd -19 |
|---|---|---|---|---|
| src/ | 337920 | 54645 (83.8%) | 54484 (83.9%) | 46647 (86.2%) |
| share/ | 30720 | 2606 (91.5%) | 3186 (89.6%) | 3037 (90.1%) |

fossil is level with gzip on source trees and beats both on smaller ones. zstd wins on
directories because its window is much larger than 256 KB, so it dedups across the whole
archive. That's the price of cheap `take` and `mount`. Single-file numbers are
unaffected; only directory fossils are segmented.

The point of fossil isn't only to win rows. It's that `fossil explain` can tell you which
model handled each block, and why.
