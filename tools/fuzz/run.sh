#!/bin/sh
# Fuzz targets in parallel: tools/fuzz/run.sh SECONDS [target ...]
# Build first: cargo +nightly fuzz build -O --fuzz-dir tools/fuzz --target-dir target/fuzz
# Crashing inputs land in target/fuzz/artifacts/<target>/, logs in target/fuzz/logs/.
cd "$(dirname "$0")/../.." || exit 1
secs=$1; shift
[ $# -gt 0 ] || set -- bsp formats progs packs nq_client qw_client netchan text server savegame
out=$PWD/target/fuzz
export QUAKE_BASEDIR="$out/base" XDG_DATA_HOME="$out/xdg" XDG_CONFIG_HOME="$out/xdg"
mkdir -p "$out/logs"
for t in "$@"; do
    mkdir -p "$out/corpus/$t" "$out/artifacts/$t"
    case $t in progs) max=900000 ;; bsp | savegame) max=200000 ;; *) max=40000 ;; esac
    nice -n 19 "$out/x86_64-unknown-linux-gnu/release/$t" "$out/corpus/$t" \
        -artifact_prefix="$out/artifacts/$t/" -fork=1 -ignore_crashes=1 -ignore_ooms=1 -ignore_timeouts=1 \
        -max_total_time="$secs" -timeout=10 -rss_limit_mb=1500 -malloc_limit_mb=512 -max_len=$max \
        >"$out/logs/$t.log" 2>&1 &
done
wait
for t in "$@"; do
    printf '%-10s artifacts %4s  %s\n' "$t" "$(find "$out/artifacts/$t" -type f | wc -l)" "$(grep -E '^#[0-9]+: cov' "$out/logs/$t.log" | tail -1)"
done
