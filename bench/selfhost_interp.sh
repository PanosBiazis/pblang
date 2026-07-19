#!/usr/bin/env bash
# selfhost_interp.sh — the pblang EVALUATOR (interp.sapp), together with the pblang tokenizer + parser,
# compiled to a SINGLE native x86-64 ELF by the pblang-written code generator, must correctly evaluate
# pblang source. This is "pblang interpreting pblang, natively" — no Rust in the run path.
#
# Supported subset (grows over time): number/string literals, +/-/* with precedence, print, VARIABLES
# (let/assign), while loops, FUNCTIONS + recursion, and CLOSURES with captured variables — the evaluator's
# environment is a chain of string-keyed maps, reached via the codegen's runtime `[]` tag-dispatch.
set -u
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"   # pl-dev/pblang
PB="${PB:-$DIR/target/release/pblang}"
CG="$DIR/stdlib/native/codegen_x64.sapp"
TOK="$DIR/stdlib/compiler/tokenizer.sapp"
PAR="$DIR/stdlib/compiler/parser.sapp"
INT="$DIR/stdlib/compiler/interp.sapp"
W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
PASS=0; FAIL=0

# parser minus self-test + duplicate push; interp minus imports/export, module calls → top-level names.
sed '/^fn print_ast/,$d' "$PAR" | perl -0pe 's/fn push\(arr, value\) \{\s*array_push\(arr, value\);\s*\}//' > "$W/par.sapp"
sed -e '/^let _tok = import/d' -e '/^let _par = import/d' -e '/^export /d' \
    -e 's/_tok\.tokenize/tokenize/g' -e 's/_par\.parse/parse/g' "$INT" > "$W/int.sapp"
cat "$TOK" "$W/par.sapp" "$W/int.sapp" > "$W/base.txt"

check() {  # target-source  expected-stdout   (target may contain " — escaped into the pblang string literal)
  cp "$W/base.txt" "$W/n.txt"
  local esc="${1//\"/\\\"}"
  printf '\nlet src = "%s";\neval_program(src);\n' "$esc" >> "$W/n.txt"
  cat > "$W/n.sapp" <<EOF
let cg=import("$CG"); fs_write_bytes("$W/n.bin", cg.compile_fns(bytes_to_str(fs_read_bytes("$W/n.txt"))));
EOF
  local err; err=$( "$PB" run "$W/n.sapp" --allow-all </dev/null 2>&1 | grep -iE "error|byte|panic" | head -1 )
  if [ -n "$err" ]; then echo "FAIL [$1]: native compile error: $err"; FAIL=$((FAIL+1)); return; fi
  chmod +x "$W/n.bin"; local got; got=$( "$W/n.bin" 2>/dev/null | tr '\n' ' ' | sed 's/ $//' )
  if [ "$got" = "$2" ]; then PASS=$((PASS+1)); else echo "FAIL [$1]: got '$got' expected '$2'"; FAIL=$((FAIL+1)); fi
}

check 'print 42;'              '42'
check 'print 3 + 4 * 2 - 1;'   '10'
check 'print 2 * 3 + 4 * 5;'   '26'
check 'print 10 + 20 + 30;'    '60'
check 'print 100 - 6 * 3;'     '82'
check 'print 7 - 2 - 1;'       '4'
# variables, loops, functions, recursion, closures — env is a chain of string-keyed maps (runtime [] dispatch)
check 'let x = 5; print x + 3;'                                            '8'
check 'let a = 10; let b = 20; print a + b;'                              '30'
check 'let i = 0; while i < 5 { print i; i = i + 1; }'                    '0 1 2 3 4'
check 'fn sq(n){ return n * n; } print sq(7);'                           '49'
check 'fn fib(n){ if n<2 { return n; } return fib(n-1)+fib(n-2); } print fib(10);'  '55'
check 'fn mk(a){ return fn(b){ return a+b; }; } let add5=mk(5); print add5(10);'    '15'
# strings, equality (runtime tag-dispatch), target arrays & maps as values
check 'print "hi";'                                                       'hi'
check 'let s = "ab"; if s == "ab" { print 1; } else { print 0; }'        '1'
check 'let w = "cat"; if w != "dog" { print 1; } else { print 0; }'      '1'
check 'fn g(n){ if n == 0 { return "zero"; } return "pos"; } print g(0); print g(9);' 'zero pos'
check 'let a = [10, 20, 30]; print a[1];'                                '20'
check 'let a = [1, 2, 3]; a[0] = 99; print a[0];'                        '99'
check 'let a = [5, 6, 7]; let s = 0; let i = 0; while i < 3 { s = s + a[i]; i = i + 1; } print s;' '18'
check 'let m = map{ a: 1, b: 2 }; print m.a + m.b;'                      '3'
check 'let m = map{}; m.k = 5; print m.k;'                               '5'
check 'let m = map{ name: "bob" }; print m.name;'                        'bob'
# for loops (3 forms) + break + continue
check 'let s = 0; for i 5 { s = s + i; } print s;'                                       '10'
check 'for i in 1 to 3 { print i; }'                                                     '1 2 3'
check 'let a=[10,20,30]; let t=0; for x in a { t = t + x; } print t;'                     '60'
check 'let s=0; for i 10 { if i == 5 { break; } s = s + 1; } print s;'                    '5'
check 'let s=0; for i 5 { if i == 2 { continue; } s = s + i; } print s;'                  '8'
check 'let i=0; while i<8 { i = i+1; if i==3 { continue; } if i==6 { break; } print i; }' '1 2 4 5'

echo "selfhost_interp: $PASS/$((PASS+FAIL)) passed$( [ $FAIL -gt 0 ] && echo " ($FAIL FAILED)" )"
