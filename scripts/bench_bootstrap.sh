#!/usr/bin/env bash
# bench_bootstrap.sh — build/executable size + RAM benchmark for the pblang toolchain.
#
# It creates ONE executable path "with Rust" (the cargo-built pblang toolchain binary, which
# RUNS the target program interpreted) and THREE executable builds "with pblang" (the pblang
# C-transpiler compiling the SAME target program to a standalone native executable), then for
# each records:
#   - executable-on-disk size (bytes)  — the artifact you actually ship/run
#   - peak process RSS (KiB)           — Maximum resident set size while running the program
#
# The three pblang builds also serve as a determinism check (identical program ⇒ identical exe).
#
# Usage:  scripts/bench_bootstrap.sh            # uses the release binary; builds it if missing
#         PBLANG_FORCE_CARGO=1 scripts/bench_bootstrap.sh   # always re-run `cargo build --release`
#
# No arguments; writes a Markdown table to stdout and to bench_bootstrap_result.md.

set -u

# --- locate the repo (this script lives in <repo>/scripts/) ---------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PBLANG_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
RUST_BIN="$PBLANG_DIR/target/release/pblang"
WORK="$(mktemp -d)"
RESULT="$PBLANG_DIR/bench_bootstrap_result.md"
trap 'rm -rf "$WORK"' EXIT

# --- helpers --------------------------------------------------------------------------------
filesize() { stat -c %s "$1" 2>/dev/null || echo 0; }

# peak_rss <cmd...> → Maximum resident set size in KiB (GNU time -v). Discards program output.
peak_rss() {
    /usr/bin/time -v "$@" >/dev/null 2>"$WORK/time.txt"
    awk '/Maximum resident set size/ {print $NF}' "$WORK/time.txt"
}

human() { # bytes → human
    awk -v b="$1" 'BEGIN{ split("B KiB MiB GiB",u); i=1; while(b>=1024&&i<4){b/=1024;i++} printf (i==1?"%d %s":"%.1f %s"), b, u[i] }'
}

# --- the target program: a representative, deterministic, self-contained workload ------------
PROG="$WORK/bench_prog.sapp"
cat > "$PROG" <<'EOF'
// A small, deterministic workload: recursion + a loop + string building, no I/O or capabilities.
fn fib(n) { if n < 2 { return n; } return fib(n - 1) + fib(n - 2); }

let total = 0;
let i = 0;
while i < 25 { total = total + fib(i); i = i + 1; }

let s = "";
let k = 0;
while k < 200 { s = s + str(k % 10); k = k + 1; }

print "fib_sum(0..24)=" + str(total) + " len(s)=" + str(len(s));
EOF

echo "# pblang bootstrap / executable benchmark"
echo
echo "Target program: \`bench_prog.sapp\` (recursion + loop + string build, no I/O)."
echo

# --- 1) "with Rust": the cargo-built pblang toolchain binary --------------------------------
echo "## 1× with Rust (cargo)"
if [ "${PBLANG_FORCE_CARGO:-0}" = "1" ] || [ ! -x "$RUST_BIN" ]; then
    echo "Building the toolchain with \`cargo build --release\` (this can take a few minutes)…" >&2
    ( cd "$PBLANG_DIR" && cargo build --release -p pblang >/dev/null 2>&1 )
fi
if [ ! -x "$RUST_BIN" ]; then
    echo "ERROR: could not find or build $RUST_BIN" >&2
    exit 1
fi
RUST_SIZE=$(filesize "$RUST_BIN")
RUST_RSS=$(peak_rss "$RUST_BIN" run "$PROG")
echo "- toolchain binary: \`target/release/pblang\`"
echo "- runs the program **interpreted** (\`pblang run\`)"
echo

# --- 2) "with pblang" ×3: compile the SAME program to a native executable -------------------
echo "## 3× with pblang (native C-transpiler: \`pblang compile\`)"
declare -a P_SIZE P_RSS
for n in 1 2 3; do
    EXE="$WORK/bench_pblang_$n"
    "$RUST_BIN" compile "$PROG" --out "$EXE" >/dev/null 2>&1
    if [ ! -x "$EXE" ]; then echo "  build #$n FAILED" >&2; P_SIZE[$n]=0; P_RSS[$n]=0; continue; fi
    P_SIZE[$n]=$(filesize "$EXE")
    P_RSS[$n]=$(peak_rss "$EXE")
done
echo

# --- comparison table -----------------------------------------------------------------------
{
echo "## Comparison"
echo
printf "| # | Build | Executable (bytes) | Executable (human) | Peak RSS (KiB) |\n"
printf "|---|-------|-------------------:|-------------------:|---------------:|\n"
printf "| 0 | Rust: \`cargo\` toolchain (interpret) | %s | %s | %s |\n" "$RUST_SIZE" "$(human "$RUST_SIZE")" "$RUST_RSS"
for n in 1 2 3; do
    printf "| %d | pblang: \`compile\` native exe #%d | %s | %s | %s |\n" "$n" "$n" "${P_SIZE[$n]}" "$(human "${P_SIZE[$n]}")" "${P_RSS[$n]}"
done
echo

# determinism + ratio commentary
if [ "${P_SIZE[1]}" = "${P_SIZE[2]}" ] && [ "${P_SIZE[2]}" = "${P_SIZE[3]}" ]; then
    echo "- **Determinism:** all three pblang builds produced byte-identical executables (${P_SIZE[1]} bytes)."
else
    echo "- **Determinism:** pblang executable sizes varied: ${P_SIZE[1]} / ${P_SIZE[2]} / ${P_SIZE[3]} bytes."
fi
if [ "${P_SIZE[1]:-0}" -gt 0 ] && [ "$RUST_SIZE" -gt 0 ]; then
    printf -- "- **On-disk:** the pblang-compiled native exe is ~%d× smaller than the Rust toolchain binary.\n" \
        "$(( RUST_SIZE / P_SIZE[1] ))"
fi
if [ "${P_RSS[1]:-0}" -gt 0 ] && [ "${RUST_RSS:-0}" -gt 0 ]; then
    printf -- "- **In RAM:** the pblang-compiled native exe peaks at ~%d× less RSS than the interpreter.\n" \
        "$(( RUST_RSS / P_RSS[1] ))"
fi
} | tee "$RESULT"

# --- 3) backend probe: which native backends actually work vs fall back to C99 --------------
{
echo
echo "## Native backend probe (same program, each \`--backend\`)"
echo
printf "| Backend flag | Path actually used | Exe (bytes) | Peak RSS (KiB) |\n"
printf "|--------------|--------------------|------------:|---------------:|\n"
probe_backend() { # <label> <compile-flags...>
    local label="$1"; shift
    local exe="$WORK/probe_$label"
    local out
    out="$("$RUST_BIN" compile "$PROG" --out "$exe" "$@" 2>&1)"
    local via="C99"
    echo "$out" | grep -qi "via LLVM"      && via="LLVM"
    echo "$out" | grep -qi "via cranelift" && via="Cranelift"
    if echo "$out" | grep -qi "falling back"; then via="C99 (fell back)"; fi
    local size rss
    if [ -x "$exe" ]; then size=$(filesize "$exe"); rss=$(peak_rss "$exe"); else size=0; rss=0; fi
    printf "| %s | %s | %s | %s |\n" "$label" "$via" "$size" "$rss"
}
probe_backend "ctranspiler" --backend ctranspiler
probe_backend "cranelift"   --backend cranelift
probe_backend "llvm"        --llvm
echo
echo "> A \"fell back\" row means that native backend errored on this program/toolchain and the"
echo "> compiler transparently used the C99 path instead — worth knowing when a backend is advertised."
} | tee -a "$RESULT"

echo
echo "(Full table written to $RESULT)"
