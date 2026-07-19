#!/usr/bin/env bash
# pipeline_bench.sh — measure every stage of the pblang self-hosted pipeline end to
# end (all written in pblang): tokenize → parse → { interpret | codegen → native }.
# The .sapp side times tokenize/parse/interpret/codegen (via now_ms) and writes the
# codegen ELF to /tmp/pipe_bin; this wrapper then times the native binary's execution.
set -u
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"   # pl-dev/pblang
PB="${PB:-$DIR/target/release/pblang}"

echo "=== pblang self-hosted pipeline — per-stage timing (workload: fib(20)) ==="
out=$( cd "$DIR" && timeout 180 "$PB" run bench/pipeline_stages.sapp --allow-all </dev/null 2>/dev/null )
tok=$(   echo "$out" | sed -n 's/^TOK_ms=//p')
parse=$( echo "$out" | sed -n 's/^PARSE_ms=//p')
interp=$(echo "$out" | sed -n 's/^INTERP_total_ms=//p')
cg=$(    echo "$out" | sed -n 's/^CODEGEN_ms=//p')
size=$(  echo "$out" | sed -n 's/^SIZE_bytes=//p')

# time native execution of the codegen output
nat="-"
if [ -f /tmp/pipe_bin ]; then
  chmod +x /tmp/pipe_bin
  best=99999
  for k in 1 2 3 4 5; do t0=$(date +%s%3N); /tmp/pipe_bin; t1=$(date +%s%3N); d=$((t1-t0)); [ $d -lt $best ] && best=$d; done
  nat="$best"
  rm -f /tmp/pipe_bin
fi

printf '%-38s %s\n' "stage" "time_ms"
printf '%-38s %s\n' "-------------------------------------" "-------"
printf '%-38s %s\n' "1. tokenize (source -> tokens)"        "$tok"
printf '%-38s %s\n' "2. parse (tokens -> AST)"              "$parse"
printf '%-38s %s\n' "3a. interpret (pblang tree-walker)"    "$interp"
printf '%-38s %s\n' "3b. codegen (AST -> x86-64 bytes)"     "$cg"
printf '%-38s %s\n' "3b. native execute (run machine code)" "$nat"
printf '%-38s %s\n' "codegen output size (bytes)"           "$size"
echo
echo "Front end (tokenize+parse) is ~sub-ms; the tree-walking interpreter is the slow"
echo "path, so compiling remains the speed route. Run this script repeatedly and compare"
echo "medians: sub-millisecond stages and process startup are especially noise-sensitive."
