#!/usr/bin/env bash
set -e

cd "$(dirname "$0")"
cargo build --release -q
BIN=target/release/fossil

files=(examples/z examples/mixed.bin examples/bigmix.bin examples/cat.ppm examples/wave.pcm examples/cat.jpg)

have_zstd=0
command -v zstd >/dev/null 2>&1 && have_zstd=1

pct() {
    awk -v o="$1" -v n="$2" 'BEGIN { if (o == 0) { print "-" } else { printf "%.1f%%", (1 - n/o) * 100 } }'
}

printf "| %-12s | %10s | %10s | %10s | %10s |\n" file original "fossil" "gzip -9" "zstd -19"
printf "|%s|%s|%s|%s|%s|\n" "--------------" "------------" "------------" "------------" "------------"

for f in "${files[@]}"; do
    [ -f "$f" ] || continue
    orig=$(wc -c < "$f" | tr -d ' ')

    "$BIN" pack "$f" /tmp/bench >/dev/null 2>&1
    fos=$(wc -c < /tmp/bench.fossil | tr -d ' ')

    gz=$(gzip -9 -c "$f" | wc -c | tr -d ' ')

    if [ "$have_zstd" -eq 1 ]; then
        zs=$(zstd -19 -c "$f" 2>/dev/null | wc -c | tr -d ' ')
        zcol="$zs ($(pct "$orig" "$zs"))"
    else
        zcol="n/a"
    fi

    printf "| %-12s | %10s | %s | %s | %s |\n" \
        "$(basename "$f")" "$orig" \
        "$fos ($(pct "$orig" "$fos"))" \
        "$gz ($(pct "$orig" "$gz"))" \
        "$zcol"
done

printf "\n"
printf "| %-12s | %10s | %10s | %10s | %10s |\n" directory "tar bytes" "fossil" "gzip -9" "zstd -19"
printf "|%s|%s|%s|%s|%s|\n" "--------------" "------------" "------------" "------------" "------------"

dirs=(src share)

for d in "${dirs[@]}"; do
    [ -d "$d" ] || continue
    orig=$(tar -cf - "$d" 2>/dev/null | wc -c | tr -d ' ')

    "$BIN" pack "$d" /tmp/bench >/dev/null 2>&1
    fos=$(wc -c < /tmp/bench.fossil | tr -d ' ')

    gz=$(tar -cf - "$d" 2>/dev/null | gzip -9 | wc -c | tr -d ' ')

    if [ "$have_zstd" -eq 1 ]; then
        zs=$(tar -cf - "$d" 2>/dev/null | zstd -19 2>/dev/null | wc -c | tr -d ' ')
        zcol="$zs ($(pct "$orig" "$zs"))"
    else
        zcol="n/a"
    fi

    printf "| %-12s | %10s | %s | %s | %s |\n" \
        "$d/" "$orig" \
        "$fos ($(pct "$orig" "$fos"))" \
        "$gz ($(pct "$orig" "$gz"))" \
        "$zcol"
done

rm -f /tmp/bench.fossil
