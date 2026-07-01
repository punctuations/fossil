# benchmark

fossil against `gzip -9` and `zstd -19` on the example files. Run it yourself:

```sh
./bench.sh
```

| file | original | fossil | gzip -9 | zstd -19 |
|---|---|---|---|---|
| z | 100000 | 217 (99.8%) | 134 (99.9%) | 25 (100.0%) |
| mixed.bin | 360448 | 78449 (78.2%) | 93399 (74.1%) | 82787 (77.0%) |
| bigmix.bin | 901120 | 195706 (78.3%) | 228264 (74.7%) | 204357 (77.3%) |
| cat.ppm | 1848015 | 497028 (73.1%) | 904590 (51.1%) | 781646 (57.7%) |
| wave.pcm | 300000 | 57095 (81.0%) | 294794 (1.7%) | 293048 (2.3%) |
| cat.jpg | 60055 | 60073 (-0.0%) | 60059 (-0.0%) | 60069 (-0.0%) |

These numbers are only for these files. Other data would land somewhere else. Roughly why each row does what it does: the mixed files do well because fossil picks a model per 4 KB block, which fits data that keeps changing as you read it. The image gets filtered row by row first (PNG-style), so the models see small differences instead of raw pixels. wave.pcm is audio, so fossil fits a predictor to each block the way FLAC does and codes what's left over. gzip and zstd don't predict audio, so they barely touch it. cat.jpg is already compressed, so there's nothing left to take.

The blocks are still 4 KB, but the LZ models can look back up to 64 KB into what they've already seen, so a repeat far from the original only costs a pointer instead of a second copy. One of them, LZR, codes those pointers with a bit of context, like LZMA, which is what helps the mixed files.

The point of fossil isn't only to win rows. It's that `fossil explain` can tell you which model handled each block, and why.
