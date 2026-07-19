#!/usr/bin/env bash
# loop_bench.sh — second cross-language benchmark, COMPLEMENTARY to lang_shootout.sh (which is recursive fib).
# Workload: a tight integer loop with NO function-call overhead — a rolling hash:
#     h = 0;  for i in [0, N):  h = (h*31 + i) & 0xFFFF;   print h
# Why this workload:
#   • It stresses the LOOP path, not the call path. fib rewards good call/return codegen; this rewards good
#     REGISTER ALLOCATION — keeping the accumulator h and counter i in registers across the whole loop instead
#     of spilling to the stack every iteration. That is exactly where pblang's native codegen invests.
#   • It is bounded and DETERMINISTIC across every language and every pblang tier: h stays < 2^16, h*31 < 2^21,
#     +i (i < 2^27 for N≤10^8) < 2^27 — all well inside f64's exact-integer range (2^53). So the pblang
#     interpreter (f64 values) and the native 64-bit backends compute the identical result. No `**`, no div.
#
# Every language is OPTIONAL and AUTO-DETECTED, same as lang_shootout.sh.
# Usage:  bench/loop_bench.sh [N] [best_of_K]     (defaults: N=100000000, K=3)
set -u
N="${1:-100000000}"
K="${2:-3}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
PB="${PB:-$ROOT/target/release/pblang}"
CG="$ROOT/stdlib/native/codegen_x64.sapp"
ASM_SRC="$HERE/loop_asm.S"
W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
cd "$W"

ms() { date +%s%3N; }
row() { printf '%-26s %11s %16s %12s\n' "$1" "$2" "$3" "$4"; }
# best-of-K with a per-run wall-clock cap (TMO seconds): a pathologically slow interpreter (Tcl/AWK at 100M
# iters) is capped and reported as "timeout" instead of hanging the whole suite. Compiled langs finish instantly.
TMO="${TMO:-75}"
bestof() { local best=99999999 t0 t1 r i rc; for ((i=0;i<K;i++)); do t0=$(ms); r=$(timeout "$TMO" "$@" 2>/dev/null); rc=$?; t1=$(ms); if [ $rc -eq 124 ]; then REP=">${TMO}s"; RES="(timeout)"; return; fi; (( t1-t0<best )) && best=$((t1-t0)); done; REP=$best; RES=$r; }
have() { command -v "$1" >/dev/null 2>&1; }
skip() { row "$1" "n/a" "not installed" "$2"; }

echo "=== Loop benchmark — h=(h*31+i)&0xFFFF over $N iters, best-of-$K — $(date -u +%FT%TZ) ==="
echo "    (host: $(uname -m) $(nproc) cores; time_ms includes process startup; register-allocation stress test)"
row "target" "time_ms" "result" "size/notes"
row "--------------------------" "-----------" "----------------" "------------"

# ───────────────────────── pblang: three tiers ─────────────────────────
cat > loop.sapp <<EOF
let h = 0; let i = 0;
while i < $N { h = (h * 31 + i) & 65535; i = i + 1; }
print h;
EOF
cat > cgdrv.sapp <<EOF
let cg = import("$CG");
fs_write_bytes("loop_cg", cg.compile_fns(bytes_to_str(fs_read_bytes("loop.sapp"))));
EOF
if "$PB" run cgdrv.sapp --allow-all </dev/null >/dev/null 2>&1 && [ -f loop_cg ]; then
  chmod +x loop_cg; bestof ./loop_cg; row "pblang (native codegen-x64)" "$REP" "$RES" "$(stat -c %s loop_cg)"
else
  row "pblang (native codegen-x64)" "err" "compile failed" "-"
fi
cat > loopr.sapp <<EOF
let h = 0; let i = 0;
while i < $N { h = (h * 31 + i) & 65535; i = i + 1; }
print str(h);
EOF
if "$PB" compile loopr.sapp --out loop_pbc >/dev/null 2>&1; then
  bestof ./loop_pbc; row "pblang (native C99)" "$REP" "$RES" "$(stat -c %s loop_pbc)"
fi
bestof "$PB" run loopr.sapp; row "pblang (interpreter)" "$REP" "$RES" "-"

# ───────────────────────── hand-written assembly (the floor) ─────────────────────────
if have as && have ld && [ -f "$ASM_SRC" ]; then
  sed "s/\bNVAL\b/$N/" "$ASM_SRC" > loop_asm.S
  if as --64 loop_asm.S -o loop_asm.o 2>/dev/null && ld loop_asm.o -o loop_asm 2>/dev/null; then
    bestof ./loop_asm; row "Assembly (hand-written)" "$REP" "$RES" "$(stat -c %s loop_asm)"
  else row "Assembly (hand-written)" "err" "assemble failed" "-"; fi
else skip "Assembly (hand-written)" "binutils"; fi

# ───────────────────────── C ─────────────────────────
printf '#include <stdio.h>\nint main(){long h=0;for(long i=0;i<%sL;i++)h=(h*31+i)&65535;printf("%%ld\\n",h);return 0;}\n' "$N" > loop.c
if have gcc; then
  gcc -O2 -o loop_c2 loop.c 2>/dev/null && { bestof ./loop_c2; row "C (gcc -O2)" "$REP" "$RES" "$(stat -c %s loop_c2)"; }
  gcc -O0 -o loop_c0 loop.c 2>/dev/null && { bestof ./loop_c0; row "C (gcc -O0)" "$REP" "$RES" "$(stat -c %s loop_c0)"; }
else skip "C (gcc)" "gcc"; fi
if have clang; then clang -O2 -o loop_clang loop.c 2>/dev/null && { bestof ./loop_clang; row "C (clang -O2)" "$REP" "$RES" "$(stat -c %s loop_clang)"; }
else skip "C (clang)" "clang"; fi
printf '#include <cstdio>\nint main(){long h=0;for(long i=0;i<%sL;i++)h=(h*31+i)&65535;printf("%%ld\\n",h);return 0;}\n' "$N" > loop.cpp
if have g++; then g++ -O2 -o loop_gpp loop.cpp 2>/dev/null && { bestof ./loop_gpp; row "C++ (g++ -O2)" "$REP" "$RES" "$(stat -c %s loop_gpp)"; }
else skip "C++ (g++)" "g++"; fi
if have clang++; then clang++ -O2 -o loop_clpp loop.cpp 2>/dev/null && { bestof ./loop_clpp; row "C++ (clang++ -O2)" "$REP" "$RES" "$(stat -c %s loop_clpp)"; }
else skip "C++ (clang++)" "clang"; fi

# ───────────────────────── Rust ─────────────────────────
if have rustc; then
  printf 'fn main(){let mut h:i64=0;for i in 0..%si64{h=(h*31+i)&65535;}println!("{}",h);}\n' "$N" > loop.rs
  rustc -O -o loop_rs loop.rs 2>/dev/null && { bestof ./loop_rs; row "Rust (rustc -O)" "$REP" "$RES" "$(stat -c %s loop_rs)"; } || skip "Rust (rustc)" "rustc"
else skip "Rust (rustc)" "rustup"; fi

# ───────────────────────── Fortran ─────────────────────────
if have gfortran; then
  cat > loop.f90 <<EOF
program main
  integer(8) :: h, i
  h = 0
  do i = 0, ${N}_8 - 1
    h = iand(h*31 + i, 65535_8)
  end do
  print '(I0)', h
end program main
EOF
  gfortran -O2 -o loop_f90 loop.f90 2>/dev/null && { bestof ./loop_f90; row "Fortran (gfortran -O2)" "$REP" "$RES" "$(stat -c %s loop_f90)"; } || skip "Fortran (gfortran)" "gfortran"
else skip "Fortran (gfortran)" "gfortran"; fi

# ───────────────────────── D ─────────────────────────
if have ldc2; then
  printf 'import std.stdio;\nvoid main(){ long h=0; for(long i=0;i<%s;i++) h=(h*31+i)&65535; writeln(h); }\n' "$N" > loop.d
  ldc2 -O3 -of=loop_d loop.d 2>/dev/null && { bestof ./loop_d; row "D (ldc2 -O3)" "$REP" "$RES" "$(stat -c %s loop_d)"; } || skip "D (ldc2)" "ldc"
elif have gdc; then
  printf 'import std.stdio;\nvoid main(){ long h=0; for(long i=0;i<%s;i++) h=(h*31+i)&65535; writeln(h); }\n' "$N" > loop.d
  gdc -O2 -o loop_d loop.d 2>/dev/null && { bestof ./loop_d; row "D (gdc -O2)" "$REP" "$RES" "$(stat -c %s loop_d)"; } || skip "D (gdc)" "gdc"
else skip "D (ldc/gdc)" "ldc"; fi

# ───────────────────────── Nim ─────────────────────────
if have nim; then
  printf 'var h: int = 0\nfor i in 0 ..< %s:\n  h = (h*31 + i) and 65535\necho h\n' "$N" > loop.nim
  nim c --opt:speed --hints:off --nimcache:"$W/nc" -o:loop_nim loop.nim >/dev/null 2>&1 && { bestof ./loop_nim; row "Nim (--opt:speed)" "$REP" "$RES" "$(stat -c %s loop_nim)"; } || skip "Nim" "nim"
else skip "Nim" "nim"; fi

# ───────────────────────── Go ─────────────────────────
if have go; then
  printf 'package main\nimport "fmt"\nfunc main(){ var h int64 = 0; for i := int64(0); i < %s; i++ { h = (h*31 + i) & 65535 }; fmt.Println(h) }\n' "$N" > loop.go
  ( export GOCACHE="$W/gocache" GOFLAGS=-mod=mod; go build -o loop_go loop.go 2>/dev/null ) && { bestof ./loop_go; row "Go (gc)" "$REP" "$RES" "$(stat -c %s loop_go)"; } || skip "Go (gc)" "golang-go"
else skip "Go (gc)" "golang-go"; fi

# ───────────────────────── C# ─────────────────────────
CS='class P{static void Main(){long h=0;for(long i=0;i<NVAL;i++)h=(h*31+i)&65535;System.Console.WriteLine(h);}}'
if have mcs && have mono; then
  printf '%s\n' "${CS/NVAL/$N}" > loop.cs
  mcs -optimize+ -out:loop_cs.exe loop.cs 2>/dev/null && { bestof mono loop_cs.exe; row "C# (Mono mcs)" "$REP" "$RES" "$(stat -c %s loop_cs.exe)"; } || skip "C# (Mono)" "mono-complete"
else skip "C# (Mono)" "mono-complete"; fi
if have dotnet; then
  printf '%s\n' "${CS/NVAL/$N}" > loop.cs
  mkdir -p csproj && cp loop.cs csproj/Program.cs
  printf '<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net8.0</TargetFramework><Nullable>disable</Nullable></PropertyGroup></Project>\n' > csproj/app.csproj
  ( cd csproj && DOTNET_CLI_TELEMETRY_OPTOUT=1 dotnet build -c Release -o out >/dev/null 2>&1 ) && { bestof dotnet csproj/out/app.dll; row "C# (.NET RyuJIT)" "$REP" "$RES" "-"; } || skip "C# (.NET)" "dotnet-sdk-8.0"
else skip "C# (.NET)" "dotnet-sdk-8.0"; fi

# ───────────────────────── Pascal / Delphi ─────────────────────────
if have fpc; then
  cat > loop.pas <<EOF
program loop;
var h, i: int64;
begin
  h := 0;
  for i := 0 to ${N} - 1 do h := (h*31 + i) and 65535;
  writeln(h);
end.
EOF
  fpc -O2 -oloop_pas loop.pas >/dev/null 2>&1 && { bestof ./loop_pas; row "Pascal (fpc -O2)" "$REP" "$RES" "$(stat -c %s loop_pas)"; } || skip "Pascal (fpc)" "fpc"
else skip "Pascal (fpc)" "fpc"; fi

# ───────────────────────── JVM ─────────────────────────
if have javac; then
  printf 'public class Loop { public static void main(String[] a){ long h=0; for(long i=0;i<%sL;i++) h=(h*31+i)&65535; System.out.println(h); } }\n' "$N" > Loop.java
  javac Loop.java 2>/dev/null && { bestof java Loop; row "Java (OpenJDK)" "$REP" "$RES" "$(stat -c %s Loop.class)"; }
else skip "Java (OpenJDK)" "default-jdk"; fi
if have kotlinc; then
  printf 'fun main() { var h = 0L; for (i in 0L until %sL) h = (h*31 + i) and 65535L; println(h) }\n' "$N" > loop.kt
  kotlinc loop.kt -include-runtime -d loop_kt.jar >/dev/null 2>&1 && { bestof java -jar loop_kt.jar; row "Kotlin (JVM)" "$REP" "$RES" "$(stat -c %s loop_kt.jar)"; } || skip "Kotlin (JVM)" "kotlin"
else skip "Kotlin (JVM)" "kotlin"; fi

# ───────────────────────── Haskell ─────────────────────────
if have ghc; then
  cat > loop.hs <<EOF
import Data.Bits ((.&.))
go :: Int -> Int -> Int
go h i = if i >= ${N} then h else go ((h*31 + i) .&. 65535) (i+1)
main :: IO ()
main = print (go 0 0)
EOF
  ghc -O2 -outputdir "$W/hsout" -o loop_hs loop.hs >/dev/null 2>&1 && { bestof ./loop_hs; row "Haskell (ghc -O2)" "$REP" "$RES" "$(stat -c %s loop_hs)"; } || skip "Haskell (ghc)" "ghc"
else skip "Haskell (ghc)" "ghc"; fi

# ───────────────────────── scripting ─────────────────────────
if have node; then printf 'let h=0;for(let i=0;i<%s;i++)h=(h*31+i)&65535;console.log(h);\n' "$N" > loop.js
  bestof node loop.js; row "JavaScript (Node/V8)" "$REP" "$RES" "-"; else skip "JavaScript (Node)" "nodejs"; fi
if have python3; then printf 'h=0\nfor i in range(%s):\n    h=(h*31+i)&65535\nprint(h)\n' "$N" > loop.py
  bestof python3 loop.py; row "Python 3 (CPython)" "$REP" "$RES" "-"; else skip "Python 3" "python3"; fi
if have ruby; then printf 'h=0\n(0...%s).each{|i| h=(h*31+i)&65535}\nputs h\n' "$N" > loop.rb
  bestof ruby loop.rb; row "Ruby (MRI)" "$REP" "$RES" "-"; else skip "Ruby" "ruby"; fi
if have perl; then printf 'my $h=0;for my $i (0..%s-1){$h=($h*31+$i)&65535;}print "$h\\n";\n' "$N" > loop.pl
  bestof perl loop.pl; row "Perl" "$REP" "$RES" "-"; else skip "Perl" "perl"; fi
if have lua; then printf 'local h=0\nfor i=0,%s-1 do h=(h*31+i)&65535 end\nprint(h)\n' "$N" > loop.lua
  bestof lua loop.lua; row "Lua 5.4" "$REP" "$RES" "-"; else skip "Lua" "lua5.4"; fi
if have php; then printf '<?php $h=0;for($i=0;$i<%s;$i++)$h=($h*31+$i)&65535;echo $h,"\\n";\n' "$N" > loop.php
  bestof php loop.php; row "PHP" "$REP" "$RES" "-"; else skip "PHP" "php-cli"; fi
if have tclsh; then printf 'set h 0\nfor {set i 0} {$i<%s} {incr i} {set h [expr {($h*31+$i)&65535}]}\nputs $h\n' "$N" > loop.tcl
  bestof tclsh loop.tcl; row "Tcl (tclsh)" "$REP" "$RES" "-"; else skip "Tcl" "tcl"; fi
if have gawk; then printf 'BEGIN{h=0;for(i=0;i<%s;i++)h=and(h*31+i,65535);print h}\n' "$N" > loop.awk
  bestof gawk -f loop.awk; row "AWK (gawk)" "$REP" "$RES" "-"; else skip "AWK (gawk)" "gawk"; fi

echo
echo "Notes: workload has NO function calls — it isolates loop/register-allocation quality (vs lang_shootout.sh's"
echo "       recursive fib, which stresses call/return). pblang keeps h and i in registers across the loop body."
