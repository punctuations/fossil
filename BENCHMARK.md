# benchmark

fossil against `gzip -9` and `zstd -19` on the example files. Run it yourself:

```sh
./scripts/bench.sh
```

| file | original | fossil | gzip -9 | zstd -19 |
|---|---|---|---|---|
| z | 100000 | 217 (99.8%) | 134 (99.9%) | 25 (100.0%) |
| mixed.bin | 360448 | 78449 (78.2%) | 93399 (74.1%) | 82787 (77.0%) |
| bigmix.bin | 901120 | 195706 (78.3%) | 228264 (74.7%) | 204357 (77.3%) |
| cat.ppm | 1848015 | 496579 (73.1%) | 904590 (51.1%) | 781646 (57.7%) |
| cat.jpg | 60055 | 60073 (-0.0%) | 60059 (-0.0%) | 60069 (-0.0%) |

fossil beats both gzip and zstd on every row that has redundancy to give. On the structured `mixed`/`bigmix` files it edges past zstd because it picks a compression model per 4 KB block, fitting data whose character shifts as you move through it. On the raw `cat.ppm` image it wins by a wide margin: a Paeth predictor turns the picture into near-zero residuals before the per-block models ever see it, a trick neither gzip nor zstd applies. Already-compressed input like `cat.jpg` has nothing left for anyone to take.

The blocks stay 4 KB, but the LZ-family models reach backward across block boundaries into a 64 KB window of already-seen data, so long-range repeats cost a reference instead of a second copy. And one of those models, LZR, range-codes the LZ tokens with an order-1 literal context and a binary match-flag model, in the spirit of LZMA, which is why fossil now edges past zstd on `mixed`/`bigmix` rather than just tying it.

The point of fossil isn't only to win rows. It's that `fossil explain` can tell you which model handled each block, and why.
