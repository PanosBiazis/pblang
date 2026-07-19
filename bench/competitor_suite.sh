#!/usr/bin/env bash
# Run the reproducible cross-language benchmark set and preserve raw evidence.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIB_N="${FIB_N:-32}"
LOOP_N="${LOOP_N:-10000000}"
REPS="${REPS:-3}"
TMO="${TMO:-45}"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
OUT="${1:-$ROOT/bench/results/$STAMP}"
mkdir -p "$OUT"

{
    echo "utc=$STAMP"
    echo "kernel=$(uname -srmo)"
    echo "cpu=$(lscpu | sed -n 's/^Model name:[[:space:]]*//p' | head -1)"
    echo "logical_cpus=$(nproc)"
    echo "memory=$(free -h | awk '/^Mem:/{print $2}')"
    "$ROOT/target/release/pblang" --version 2>&1 | sed 's/^/pblang=/'
    for tool in gcc clang rustc go python3 node java ruby lua perl fpc dotnet mono; do
        command -v "$tool" >/dev/null 2>&1 && printf '%s=%s\n' "$tool" "$(command -v "$tool")"
    done
} > "$OUT/environment.txt"

TMO="$TMO" "$ROOT/bench/lang_shootout.sh" "$FIB_N" "$REPS" | tee "$OUT/fib.txt"
TMO="$TMO" "$ROOT/bench/loop_bench.sh" "$LOOP_N" "$REPS" | tee "$OUT/loop.txt"
"$ROOT/target/release/pblang" run "$ROOT/bench/language_category_matrix.sapp" --allow-all | tee "$OUT/language-categories.txt"
grep -q '^LANGUAGE_CATEGORY_MATRIX_SUITE|ok$' "$OUT/language-categories.txt"
"$ROOT/bench/resource_bench.sh" "$FIB_N" | tee "$OUT/resources.txt"
"$ROOT/bench/pipeline_bench.sh" | tee "$OUT/pipeline.txt"
"$ROOT/bench/pblang_phase_suite.sh" "$OUT/pblang-phases"

printf 'Benchmark evidence written to %s\n' "$OUT"
