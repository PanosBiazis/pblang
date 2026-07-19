#!/usr/bin/env bash
# lang_shootout.sh — compare pblang against mainstream languages on a common compute benchmark
# (naive recursive Fibonacci: raw arithmetic + function-call overhead). Reports best-of-K wall-clock
# time, the result, and (for compiled targets) executable size.
#
# Every language is OPTIONAL and AUTO-DETECTED: if its toolchain is present it runs; if not, the row
# says "not installed" with the package to install. So C#, Pascal, Delphi, Go, etc. appear as soon as
# their compiler exists on the host — the benchmark already ships their source and wiring.
#
# Usage:  bench/lang_shootout.sh [N] [best_of_K]     (defaults: N=32, K=3)
set -u
N="${1:-32}"
K="${2:-3}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
PB="${PB:-$ROOT/target/release/pblang}"
CG="$ROOT/stdlib/native/codegen_x64.sapp"
ASM_SRC="$HERE/fib_asm.S"
W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
cd "$W"

ms() { date +%s%3N; }
row() { printf '%-26s %11s %16s %12s\n' "$1" "$2" "$3" "$4"; }
# best-of-K runtime (ms) of a command; sets REP and RES (last stdout)
# best-of-K with a per-run wall-clock cap (TMO seconds) so one slow interpreter can't hang the suite.
TMO="${TMO:-75}"
bestof() { local best=99999999 t0 t1 r i rc; for ((i=0;i<K;i++)); do t0=$(ms); r=$(timeout "$TMO" "$@" 2>/dev/null); rc=$?; t1=$(ms); if [ $rc -eq 124 ]; then REP=">${TMO}s"; RES="(timeout)"; return; fi; (( t1-t0<best )) && best=$((t1-t0)); done; REP=$best; RES=$r; }
have() { command -v "$1" >/dev/null 2>&1; }
skip() { row "$1" "n/a" "not installed" "$2"; }   # $2 = how to install

echo "=== Language shootout — fib($N), best-of-$K — $(date -u +%FT%TZ) ==="
echo "    (host: $(uname -m) $(nproc) cores; time_ms includes process startup)"
echo "    (fixed N is embedded in source: compiler specialization is part of this category)"
row "target" "time_ms" "result" "size/notes"
row "--------------------------" "-----------" "----------------" "------------"

# ───────────────────────── pblang: three tiers ─────────────────────────
# (1) NATIVE via the pblang-written direct x86-64 code generator (codegen_x64.sapp) — the star.
cat > fib.sapp <<EOF
fn fib(n) { if n < 2 { return n; } return fib(n - 1) + fib(n - 2); }
print fib($N);
EOF
cat > cgdrv.sapp <<EOF
let cg = import("$CG");
fs_write_bytes("fib_cg", cg.compile_fns(bytes_to_str(fs_read_bytes("fib.sapp"))));
EOF
if "$PB" run cgdrv.sapp --allow-all </dev/null >/dev/null 2>&1 && [ -f fib_cg ]; then
  chmod +x fib_cg; bestof ./fib_cg; row "pblang (native codegen-x64)" "$REP" "$RES" "$(stat -c %s fib_cg)"
else
  row "pblang (native codegen-x64)" "err" "compile failed" "-"
fi
# (2) NATIVE via the C99 transpiler.
cat > fibr.sapp <<EOF
fn fib(n) { if n < 2 { return n; } return fib(n - 1) + fib(n - 2); }
print str(fib($N));
EOF
if "$PB" compile fibr.sapp --out fib_pbc >/dev/null 2>&1; then
  bestof ./fib_pbc; row "pblang (native C99)" "$REP" "$RES" "$(stat -c %s fib_pbc)"
fi
# (2b) SELF-HOSTED C99 lowering, implemented by ctranspile.sapp. Normal f64
# semantics and explicit integer specialization stay separate because division differs.
cat > fibr_self.sapp <<EOF
fn fib(n) { if n < 2 { return n; } return fib(n - 1) + fib(n - 2); }
print fib($N);
EOF
if (cd "$ROOT" && "$PB" run bench/selfhost_c99_build.sapp --allow-exec -- \
      "$W/fibr_self.sapp" "$W/fib_pbself_f64" "$W/fib_pbself_int") >/dev/null 2>&1; then
  bestof ./fib_pbself_f64; row "pblang (selfhost C99 f64)" "$REP" "$RES" "$(stat -c %s fib_pbself_f64)"
  bestof ./fib_pbself_int; row "pblang (selfhost C99 int)" "$REP" "$RES" "$(stat -c %s fib_pbself_int)"
else
  row "pblang (selfhost C99)" "err" "compile failed" "-"
fi
# (3) INTERPRETER.
bestof "$PB" run fibr.sapp; row "pblang (interpreter)" "$REP" "$RES" "-"

# ───────────────────────── hand-written assembly (the floor) ─────────────────────────
if have as && have ld && [ -f "$ASM_SRC" ]; then
  sed "s/\bNVAL\b/$N/" "$ASM_SRC" > fib_asm.S
  if as --64 fib_asm.S -o fib_asm.o 2>/dev/null && ld fib_asm.o -o fib_asm 2>/dev/null; then
    bestof ./fib_asm; row "Assembly (hand-written)" "$REP" "$RES" "$(stat -c %s fib_asm)"
  else row "Assembly (hand-written)" "err" "assemble failed" "-"; fi
else skip "Assembly (hand-written)" "binutils"; fi

# ───────────────────────── C / C++ ─────────────────────────
printf '#include <stdio.h>\nlong fib(long n){return n<2?n:fib(n-1)+fib(n-2);}\nint main(){printf("%%ld\\n",fib(%s));return 0;}\n' "$N" > fib.c
if have gcc; then
  gcc -O2 -o fib_c2 fib.c 2>/dev/null && { bestof ./fib_c2; row "C (gcc -O2)" "$REP" "$RES" "$(stat -c %s fib_c2)"; }
  gcc -O0 -o fib_c0 fib.c 2>/dev/null && { bestof ./fib_c0; row "C (gcc -O0)" "$REP" "$RES" "$(stat -c %s fib_c0)"; }
else skip "C (gcc)" "gcc"; fi
if have clang; then clang -O2 -o fib_cl fib.c 2>/dev/null && { bestof ./fib_cl; row "C (clang -O2)" "$REP" "$RES" "$(stat -c %s fib_cl)"; }
else skip "C (clang)" "clang"; fi
printf '#include <cstdio>\nlong fib(long n){return n<2?n:fib(n-1)+fib(n-2);}\nint main(){printf("%%ld\\n",fib(%s));return 0;}\n' "$N" > fib.cpp
if have g++; then g++ -O2 -o fib_gpp fib.cpp 2>/dev/null && { bestof ./fib_gpp; row "C++ (g++ -O2)" "$REP" "$RES" "$(stat -c %s fib_gpp)"; }
else skip "C++ (g++)" "g++"; fi
if have clang++; then clang++ -O2 -o fib_clpp fib.cpp 2>/dev/null && { bestof ./fib_clpp; row "C++ (clang++ -O2)" "$REP" "$RES" "$(stat -c %s fib_clpp)"; }
else skip "C++ (clang++)" "clang"; fi

# ───────────────────────── Rust ─────────────────────────
if have rustc; then
  printf 'fn fib(n:u64)->u64{if n<2{n}else{fib(n-1)+fib(n-2)}}\nfn main(){println!("{}",fib(%s));}\n' "$N" > fib.rs
  rustc -O -o fib_rs fib.rs 2>/dev/null && { bestof ./fib_rs; row "Rust (rustc -O)" "$REP" "$RES" "$(stat -c %s fib_rs)"; }
else skip "Rust (rustc)" "rustup / rust"; fi

# ───────────────────────── C# (.NET or Mono) ─────────────────────────
cat > Fib.cs <<EOF
using System;
class Fib { static long fib(long n) => n < 2 ? n : fib(n-1) + fib(n-2);
  static void Main() => Console.WriteLine(fib($N)); }
EOF
# Mono (mcs → mono runtime): the classic cross-platform CLR.
if have mcs && have mono; then
  mcs -optimize+ -out:Fib.exe Fib.cs 2>/dev/null && { bestof mono Fib.exe; row "C# (Mono mcs)" "$REP" "$RES" "$(stat -c %s Fib.exe)"; }
elif have csc; then
  csc -optimize+ -nologo -out:Fib.exe Fib.cs 2>/dev/null && { bestof ./Fib.exe; row "C# (Roslyn csc)" "$REP" "$RES" "$(stat -c %s Fib.exe)"; }
else skip "C# (Mono/Roslyn)" "mono-complete"; fi
# Modern .NET (RyuJIT) — run INDEPENDENTLY of Mono so both JITs appear side by side.
if have dotnet; then
  mkdir -p csproj && cp Fib.cs csproj/Program.cs
  printf '<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net8.0</TargetFramework><Nullable>disable</Nullable></PropertyGroup></Project>\n' > csproj/app.csproj
  ( cd csproj && DOTNET_CLI_TELEMETRY_OPTOUT=1 dotnet build -c Release -o out >/dev/null 2>&1 ) && { bestof dotnet csproj/out/app.dll; row "C# (.NET RyuJIT)" "$REP" "$RES" "-"; } || skip "C# (.NET)" "dotnet-sdk-8.0"
else skip "C# (.NET)" "dotnet-sdk-8.0"; fi

# ───────────────────────── Pascal & Delphi (Free Pascal) ─────────────────────────
if have fpc; then
  cat > fibp.pas <<EOF
program fibp;
function fib(n: Int64): Int64;
begin if n < 2 then fib := n else fib := fib(n-1) + fib(n-2); end;
begin writeln(fib($N)); end.
EOF
  fpc -O2 -ofib_pas fibp.pas >/dev/null 2>&1 && { bestof ./fib_pas; row "Pascal (fpc -O2)" "$REP" "$RES" "$(stat -c %s fib_pas)"; }
  cat > fibd.pas <<EOF
program fibd;
{\$MODE DELPHI}
function fib(n: Int64): Int64;
begin if n < 2 then Result := n else Result := fib(n-1) + fib(n-2); end;
begin writeln(fib($N)); end.
EOF
  fpc -O2 -Mdelphi -ofib_del fibd.pas >/dev/null 2>&1 && { bestof ./fib_del; row "Delphi (fpc -Mdelphi)" "$REP" "$RES" "$(stat -c %s fib_del)"; }
else skip "Pascal / Delphi (fpc)" "fpc"; fi

# ───────────────────────── Go ─────────────────────────
if have go; then
  printf 'package main\nimport "fmt"\nfunc fib(n int64) int64{if n<2{return n};return fib(n-1)+fib(n-2)}\nfunc main(){fmt.Println(fib(%s))}\n' "$N" > fib.go
  GOCACHE="$W/gocache" go build -o fib_go fib.go 2>/dev/null && { bestof ./fib_go; row "Go (gc)" "$REP" "$RES" "$(stat -c %s fib_go)"; } || skip "Go (gc)" "golang"
else skip "Go (gc)" "golang-go"; fi

# ───────────────────────── Fortran ─────────────────────────
if have gfortran; then
  cat > fib.f90 <<EOF
program main
  integer(8) :: r
  r = fib(${N}_8)
  print '(I0)', r
contains
  recursive function fib(n) result(res)
    integer(8), intent(in) :: n
    integer(8) :: res
    if (n < 2) then
      res = n
    else
      res = fib(n-1) + fib(n-2)
    end if
  end function fib
end program main
EOF
  gfortran -O2 -o fib_f90 fib.f90 2>/dev/null && { bestof ./fib_f90; row "Fortran (gfortran -O2)" "$REP" "$RES" "$(stat -c %s fib_f90)"; } || skip "Fortran (gfortran)" "gfortran"
else skip "Fortran (gfortran)" "gfortran"; fi

# ───────────────────────── D ─────────────────────────
if have ldc2; then
  printf 'import std.stdio;\nlong fib(long n){ return n<2 ? n : fib(n-1)+fib(n-2); }\nvoid main(){ writeln(fib(%s)); }\n' "$N" > fib.d
  ldc2 -O3 -of=fib_d fib.d 2>/dev/null && { bestof ./fib_d; row "D (ldc2 -O3)" "$REP" "$RES" "$(stat -c %s fib_d)"; } || skip "D (ldc2)" "ldc"
elif have gdc; then
  printf 'import std.stdio;\nlong fib(long n){ return n<2 ? n : fib(n-1)+fib(n-2); }\nvoid main(){ writeln(fib(%s)); }\n' "$N" > fib.d
  gdc -O2 -o fib_d fib.d 2>/dev/null && { bestof ./fib_d; row "D (gdc -O2)" "$REP" "$RES" "$(stat -c %s fib_d)"; } || skip "D (gdc)" "gdc"
else skip "D (ldc/gdc)" "ldc"; fi

# ───────────────────────── Haskell ─────────────────────────
if have ghc; then
  printf 'fib :: Int -> Int\nfib n = if n < 2 then n else fib (n-1) + fib (n-2)\nmain :: IO ()\nmain = print (fib %s)\n' "$N" > fib.hs
  ghc -O2 -outputdir "$W/hsout" -o fib_hs fib.hs >/dev/null 2>&1 && { bestof ./fib_hs; row "Haskell (ghc -O2)" "$REP" "$RES" "$(stat -c %s fib_hs)"; } || skip "Haskell (ghc)" "ghc"
else skip "Haskell (ghc)" "ghc"; fi

# ───────────────────────── Nim ─────────────────────────
if have nim; then
  printf 'proc fib(n: int): int =\n  if n < 2: n else: fib(n-1) + fib(n-2)\necho fib(%s)\n' "$N" > fib.nim
  nim c --opt:speed --hints:off --nimcache:"$W/nc" -o:fib_nim fib.nim >/dev/null 2>&1 && { bestof ./fib_nim; row "Nim (--opt:speed)" "$REP" "$RES" "$(stat -c %s fib_nim)"; } || skip "Nim" "nim"
else skip "Nim" "nim"; fi

# ───────────────────────── COBOL ─────────────────────────
if have cobc; then
  cat > fib.cob <<EOF
       IDENTIFICATION DIVISION.
       PROGRAM-ID. mainp.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 NN PIC S9(18) USAGE COMP VALUE $N.
       01 RR PIC S9(18) USAGE COMP.
       01 OUT PIC Z(17)9.
       PROCEDURE DIVISION.
           CALL 'fibp' USING NN RR
           MOVE RR TO OUT
           DISPLAY FUNCTION TRIM(OUT)
           STOP RUN.
       END PROGRAM mainp.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. fibp RECURSIVE.
       DATA DIVISION.
       LOCAL-STORAGE SECTION.
       01 A PIC S9(18) USAGE COMP.
       01 B PIC S9(18) USAGE COMP.
       01 RA PIC S9(18) USAGE COMP.
       01 RB PIC S9(18) USAGE COMP.
       LINKAGE SECTION.
       01 N PIC S9(18) USAGE COMP.
       01 R PIC S9(18) USAGE COMP.
       PROCEDURE DIVISION USING N R.
           IF N < 2
               MOVE N TO R
           ELSE
               COMPUTE A = N - 1
               COMPUTE B = N - 2
               CALL 'fibp' USING A RA
               CALL 'fibp' USING B RB
               COMPUTE R = RA + RB
           END-IF
           GOBACK.
       END PROGRAM fibp.
EOF
  cobc -x -O2 -free -o fib_cob fib.cob >/dev/null 2>&1 && { bestof ./fib_cob; row "COBOL (GnuCOBOL)" "$REP" "$RES" "$(stat -c %s fib_cob)"; } || skip "COBOL (GnuCOBOL)" "gnucobol"
else skip "COBOL (GnuCOBOL)" "gnucobol"; fi

# ───────────────────────── JVM ─────────────────────────
if have javac; then
  cat > Fib.java <<EOF
public class Fib { static long fib(long n){ return n<2?n:fib(n-1)+fib(n-2); }
  public static void main(String[] a){ System.out.println(fib($N)); } }
EOF
  javac Fib.java 2>/dev/null && { bestof java Fib; row "Java (OpenJDK)" "$REP" "$RES" "$(stat -c %s Fib.class)"; }
else skip "Java (OpenJDK)" "default-jdk"; fi

# ───────────────────────── Kotlin (JVM) ─────────────────────────
if have kotlinc; then
  printf 'fun fib(n: Long): Long = if (n < 2) n else fib(n-1) + fib(n-2)\nfun main() { println(fib(%sL)) }\n' "$N" > fib.kt
  kotlinc fib.kt -include-runtime -d fib_kt.jar >/dev/null 2>&1 && { bestof java -jar fib_kt.jar; row "Kotlin (JVM)" "$REP" "$RES" "$(stat -c %s fib_kt.jar)"; } || skip "Kotlin (JVM)" "kotlin"
else skip "Kotlin (JVM)" "kotlin"; fi

# ───────────────────────── scripting ─────────────────────────
if have node; then printf 'function fib(n){return n<2?n:fib(n-1)+fib(n-2);}console.log(fib(%s));\n' "$N" > fib.js
  bestof node fib.js; row "JavaScript (Node/V8)" "$REP" "$RES" "-"; else skip "JavaScript (Node)" "nodejs"; fi
if have python3; then printf 'import sys\nsys.setrecursionlimit(100000)\ndef fib(n):return n if n<2 else fib(n-1)+fib(n-2)\nprint(fib(%s))\n' "$N" > fib.py
  bestof python3 fib.py; row "Python 3 (CPython)" "$REP" "$RES" "-"; else skip "Python 3" "python3"; fi
if have ruby; then printf 'def fib(n);n<2 ? n : fib(n-1)+fib(n-2);end;puts fib(%s)\n' "$N" > fib.rb
  bestof ruby fib.rb; row "Ruby (MRI)" "$REP" "$RES" "-"; else skip "Ruby (MRI)" "ruby"; fi
if have perl; then printf 'sub fib{my $n=shift;$n<2?$n:fib($n-1)+fib($n-2)}print fib(%s),"\\n";\n' "$N" > fib.pl
  bestof perl fib.pl; row "Perl" "$REP" "$RES" "-"; else skip "Perl" "perl"; fi
if have lua; then printf 'local function fib(n) if n<2 then return n else return fib(n-1)+fib(n-2) end end\nprint(fib(%s))\n' "$N" > fib.lua
  bestof lua fib.lua; row "Lua 5.4" "$REP" "$RES" "-"; else skip "Lua" "lua5.4"; fi
if have php; then printf '<?php function fib($n){return $n<2?$n:fib($n-1)+fib($n-2);} echo fib(%s),"\\n";\n' "$N" > fib.php
  bestof php fib.php; row "PHP" "$REP" "$RES" "-"; else skip "PHP" "php-cli"; fi
if have tclsh; then printf 'proc fib {n} {expr {$n<2 ? $n : [fib [expr {$n-1}]] + [fib [expr {$n-2}]]}}\nputs [fib %s]\n' "$N" > fib.tcl
  bestof tclsh fib.tcl; row "Tcl (tclsh)" "$REP" "$RES" "-"; else skip "Tcl" "tcl"; fi
if have gawk; then printf 'function fib(n){return n<2?n:fib(n-1)+fib(n-2)} BEGIN{print fib(%s)}\n' "$N" > fib.awk
  bestof gawk -f fib.awk; row "AWK (gawk)" "$REP" "$RES" "-"; else skip "AWK (gawk)" "gawk"; fi

# ───────────────────────── more compiled / AOT languages ─────────────────────────
if have zig; then
  printf 'const std=@import("std");\nfn fib(n:i64)i64{return if(n<2) n else fib(n-1)+fib(n-2);}\npub fn main()!void{try std.io.getStdOut().writer().print("{d}\\n",.{fib(%s)});}\n' "$N" > fib.zig
  zig build-exe -O ReleaseFast --name fib_zig fib.zig >/dev/null 2>&1 && { bestof ./fib_zig; row "Zig (ReleaseFast)" "$REP" "$RES" "$(stat -c %s fib_zig)"; } || skip "Zig" "zig"
else skip "Zig" "zig"; fi
if have crystal; then
  printf 'def fib(n : Int64) : Int64\n  n < 2 ? n : fib(n-1) + fib(n-2)\nend\nputs fib(%s_i64)\n' "$N" > fib.cr
  crystal build --release -o fib_cr fib.cr >/dev/null 2>&1 && { bestof ./fib_cr; row "Crystal (--release)" "$REP" "$RES" "$(stat -c %s fib_cr)"; } || skip "Crystal" "crystal"
else skip "Crystal" "crystal"; fi
if have v; then
  printf 'fn fib(n i64) i64 { return if n < 2 { n } else { fib(n-1) + fib(n-2) } }\nfn main() { println(fib(%s)) }\n' "$N" > fib.v
  v -prod -o fib_v fib.v >/dev/null 2>&1 && { bestof ./fib_v; row "V (-prod)" "$REP" "$RES" "$(stat -c %s fib_v)"; } || skip "V" "vlang"
else skip "V" "vlang"; fi
if have ocamlfind || have ocamlopt; then
  printf 'let rec fib n = if n < 2 then n else fib (n-1) + fib (n-2)\nlet () = Printf.printf "%%d\\n" (fib %s)\n' "$N" > fib.ml
  ocamlopt -O3 -o fib_ml fib.ml >/dev/null 2>&1 && { bestof ./fib_ml; row "OCaml (ocamlopt)" "$REP" "$RES" "$(stat -c %s fib_ml)"; } || skip "OCaml" "ocaml"
else skip "OCaml" "ocaml"; fi
if have swiftc; then
  printf 'func fib(_ n: Int) -> Int { return n < 2 ? n : fib(n-1) + fib(n-2) }\nprint(fib(%s))\n' "$N" > fib.swift
  swiftc -O -o fib_sw fib.swift >/dev/null 2>&1 && { bestof ./fib_sw; row "Swift (swiftc -O)" "$REP" "$RES" "$(stat -c %s fib_sw)"; } || skip "Swift" "swift"
else skip "Swift" "swift"; fi
if have dart; then
  printf 'int fib(int n)=>n<2?n:fib(n-1)+fib(n-2);\nvoid main(){print(fib(%s));}\n' "$N" > fib.dart
  dart compile exe fib.dart -o fib_dart >/dev/null 2>&1 && { bestof ./fib_dart; row "Dart (AOT exe)" "$REP" "$RES" "$(stat -c %s fib_dart)"; } || { bestof dart run fib.dart; row "Dart (JIT)" "$REP" "$RES" "-"; }
else skip "Dart" "dart"; fi
if have valac; then
  printf 'int fib(int n){ return n<2 ? n : fib(n-1)+fib(n-2); }\nvoid main(){ print("%%d\\n", fib(%s)); }\n' "$N" > fib.vala
  valac -X -O2 -o fib_vala fib.vala >/dev/null 2>&1 && { bestof ./fib_vala; row "Vala (valac)" "$REP" "$RES" "$(stat -c %s fib_vala)"; } || skip "Vala" "valac"
else skip "Vala" "valac"; fi
if have odin; then
  printf 'package main\nimport "core:fmt"\nfib::proc(n:i64)->i64{return n<2?n:fib(n-1)+fib(n-2)}\nmain::proc(){fmt.println(fib(%s))}\n' "$N" > fib_odin.odin
  odin build fib_odin.odin -file -o:speed -out:fib_odin >/dev/null 2>&1 && { bestof ./fib_odin; row "Odin (-o:speed)" "$REP" "$RES" "$(stat -c %s fib_odin)"; } || skip "Odin" "odin"
else skip "Odin" "odin"; fi

# ───────────────────────── JIT / VM languages ─────────────────────────
if have julia; then printf 'fib(n)=n<2 ? n : fib(n-1)+fib(n-2)\nprintln(fib(%s))\n' "$N" > fib.jl
  bestof julia fib.jl; row "Julia" "$REP" "$RES" "-"; else skip "Julia" "julia"; fi
if have scala; then
  printf 'object Fib{def fib(n:Long):Long=if(n<2)n else fib(n-1)+fib(n-2)\ndef main(a:Array[String]):Unit=println(fib(%s))}\n' "$N" > Fib.scala
  scalac Fib.scala -d fib_scala.jar >/dev/null 2>&1 && { bestof scala -cp fib_scala.jar Fib; row "Scala (JVM)" "$REP" "$RES" "$(stat -c %s fib_scala.jar)"; } || skip "Scala" "scala"
else skip "Scala" "scala"; fi
if have elixir; then printf 'defmodule F do\n  def fib(n) when n<2, do: n\n  def fib(n), do: fib(n-1)+fib(n-2)\nend\nIO.puts F.fib(%s)\n' "$N" > fib.exs
  bestof elixir fib.exs; row "Elixir (BEAM)" "$REP" "$RES" "-"; else skip "Elixir" "elixir"; fi
if have escript; then printf 'main(_)->io:format("~p~n",[fib(%s)]).\nfib(N) when N<2 -> N;\nfib(N)->fib(N-1)+fib(N-2).\n' "$N" > fib.erl
  bestof escript fib.erl; row "Erlang (escript)" "$REP" "$RES" "-"; else skip "Erlang" "erlang"; fi
if have racket; then printf '#lang racket/base\n(define (fib n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))\n(displayln (fib %s))\n' "$N" > fib.rkt
  bestof racket fib.rkt; row "Racket" "$REP" "$RES" "-"; else skip "Racket" "racket"; fi
if have sbcl; then printf '(defun fib (n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))\n(format t "~d~%%" (fib %s))\n' "$N" > fib.lisp
  bestof sbcl --script fib.lisp; row "Common Lisp (SBCL)" "$REP" "$RES" "-"; else skip "Common Lisp (SBCL)" "sbcl"; fi
if have guile; then printf '(define (fib n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))\n(display (fib %s))(newline)\n' "$N" > fib.scm
  bestof guile fib.scm; row "Scheme (Guile)" "$REP" "$RES" "-"; else skip "Scheme (Guile)" "guile"; fi

# ───────────────────────── more JS runtimes ─────────────────────────
if have bun; then printf 'function fib(n){return n<2?n:fib(n-1)+fib(n-2);}console.log(fib(%s));\n' "$N" > fibb.js
  bestof bun fibb.js; row "JavaScript (Bun)" "$REP" "$RES" "-"; else skip "JavaScript (Bun)" "bun"; fi
if have deno; then printf 'function fib(n){return n<2?n:fib(n-1)+fib(n-2);}console.log(fib(%s));\n' "$N" > fibd.js
  bestof deno run fibd.js; row "JavaScript (Deno)" "$REP" "$RES" "-"; else skip "JavaScript (Deno)" "deno"; fi

echo
echo "Notes: 'pblang (native codegen-x64)' is the pblang-written DIRECT backend — no C in the compile path."
echo "       Absent toolchains show 'not installed'; install the named package and re-run to include them."
