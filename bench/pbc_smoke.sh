#!/usr/bin/env bash
# pbc_smoke.sh — regression suite for the one-command native compiler CLI (tools/pbc.sapp).
# Covers the plain lowering, --safe (memory-certified), and --overflow (checked arithmetic)
# flags and their composition. Each case compiles a pblang program to a native ELF via pbc,
# makes it executable, runs it, and checks the exit code.
set -u
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"   # products/pblang
PB="${PB:-$DIR/target/release/pblang}"
W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
N=0; PASS=0; FAIL=0

# c <program> <flags> <label> <expected_exit>
c() {
  N=$((N+1)); local src="$W/p$N.sapp" bin="$W/b$N"
  printf '%s' "$1" > "$src"
  "$PB" run "$DIR/tools/pbc.sapp" "$src" $2 --out "$bin" --allow-all >/dev/null 2>&1
  if [ ! -f "$bin" ]; then echo "FAIL [$3]: pbc produced no binary"; FAIL=$((FAIL+1)); return; fi
  chmod +x "$bin"; "$bin" >/dev/null 2>&1; local rc=$?
  if [ "$rc" -eq "$4" ]; then PASS=$((PASS+1)); else echo "FAIL [$3]: got $rc, expected $4"; FAIL=$((FAIL+1)); fi
}

# 3037000500^2 overflows signed i64; 5*5 and 100+27 do not.
OVF='let x=3037000500; return x*x;'
ADD_OVF='let x=4611686018427387904; let y=4611686018427387904; return x+y;'   # 2^62 + 2^62 = 2^63
SAFE='let x=5; return x*x;'

c "$SAFE"    ""                    "plain: safe program returns 25"          25
c "$OVF"     ""                    "plain: overflow WRAPS (not 132)"         144
c "$SAFE"    "--safe"              "--safe: safe program still 25"           25
c "$OVF"     "--overflow"          "--overflow: mul overflow traps (132)"    132
c "$ADD_OVF" "--overflow"          "--overflow: add overflow traps (132)"    132
c "$SAFE"    "--overflow"          "--overflow: safe program no false trap"  25
c "$OVF"     "--safe --overflow"   "compose: --safe --overflow traps (132)"  132
c "$SAFE"    "--safe --overflow"   "compose: safe program returns 25"        25

# division: checked mode traps div/0 and mod/0 as ud2 (132), consistent with the other arithmetic,
# instead of the raw hardware #DE (136). Normal division is unaffected.
c 'let x=6; let y=2; return x/y;' "--overflow" "div 6/2 checked returns 3"        3
c 'let x=5; let y=0; return x/y;' "--overflow" "div-by-zero checked traps (132)"  132
c 'let x=5; let y=0; return x%y;' "--overflow" "mod-by-zero checked traps (132)"  132
c 'let x=5; let y=0; return x/y;' ""           "div-by-zero plain is raw #DE (136)" 136

echo "pbc_smoke: $PASS/$N passed"
[ "$FAIL" -eq 0 ]
