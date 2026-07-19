#!/usr/bin/env bash
# selfhost_parity.sh — the pblang front end (tokenizer + parser), written in pblang and COMPILED TO
# NATIVE by the pblang-written code generator, must produce byte-identical output to the interpreter.
# This is the self-hosting regression: native `tokenize`/`parse` == reference `tokenize`/`parse`.
set -u
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"   # pl-dev/pblang
PB="${PB:-$DIR/target/release/pblang}"
CG="$DIR/stdlib/native/codegen_x64.sapp"
TOK="$DIR/stdlib/compiler/tokenizer.sapp"
PAR="$DIR/stdlib/compiler/parser.sapp"
W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
PASS=0; FAIL=0

# parser.sapp minus its self-test (import + self_test + print_ast use `import`/`type`, which don't
# native-compile) and minus the duplicate `push` (the tokenizer already defines it).
sed '/^fn print_ast/,$d' "$PAR" | perl -0pe 's/fn push\(arr, value\) \{\s*array_push\(arr, value\);\s*\}//' > "$W/par.sapp"

# a recursive expression serializer shared by both the interpreter driver and the native driver
SER='fn ser(n){ if n.kind=="binary"{return "("+ser(n.left)+n.op+ser(n.right)+")";} if n.kind=="unary"{return "("+n.op+ser(n.operand)+")";} if n.kind=="call"{let s=ser(n.func)+"("; let i=0; while i<len(n.args){ if i>0{s=s+",";} s=s+ser(n.args[i]); i=i+1;} return s+")";} if n.kind=="ident"{return n.name;} if n.kind=="number"{return n.value;} if n.kind=="index"{return ser(n.obj)+"["+ser(n.index)+"]";} if n.kind=="member"{return ser(n.obj)+"."+n.name;} return "?"+n.kind; }'

check() {  # label   pblang-driver-body (uses tokenize/parse; must `print` a string)
  local label="$1" body="$2"
  # interpreter reference
  cat > "$W/i.sapp" <<EOF
let tk=import("$TOK"); let pr=import("$PAR");
fn tokenize(s){ return tk.tokenize(s); }
fn parse(t){ return pr.parse(t); }
$SER
$body
EOF
  local iexp; iexp=$( "$PB" run "$W/i.sapp" --allow-all </dev/null 2>/dev/null )
  # native: tokenizer + parser + serializer + driver, all compiled to one binary
  cat "$TOK" "$W/par.sapp" > "$W/n.txt"
  printf '\n%s\n%s\n' "$SER" "$body" >> "$W/n.txt"
  cat > "$W/n.sapp" <<EOF
let cg=import("$CG"); fs_write_bytes("$W/n.bin", cg.compile_fns(bytes_to_str(fs_read_bytes("$W/n.txt"))));
EOF
  local cerr; cerr=$( "$PB" run "$W/n.sapp" --allow-all </dev/null 2>&1 | grep -i "error\|byte" | head -1 )
  if [ -n "$cerr" ]; then echo "FAIL [$label]: native compile error: $cerr"; FAIL=$((FAIL+1)); return; fi
  chmod +x "$W/n.bin"; local ngot; ngot=$( "$W/n.bin" 2>/dev/null )
  if [ "$iexp" = "$ngot" ] && [ -n "$iexp" ]; then PASS=$((PASS+1))
  else echo "FAIL [$label]: interp='$iexp' native='$ngot'"; FAIL=$((FAIL+1)); fi
}

# ---- tokenizer parity: kind:text stream ----
tokdriver='let ts=tokenize("SRC"); let s=""; let i=0; while i<len(ts){s=s+ts[i].kind+":"+ts[i].text+"|"; i=i+1;} print s;'
for src in 'fn add(a, b)' 'let x = 42' 'if x >= 10 and y != 0' 'return foo.bar[i]' 'while i < n'; do
  check "tokenize '$src'" "${tokdriver/SRC/$src}"
done

# ---- parser parity: top-level statement kinds ----
stmtdriver='let r=parse(tokenize("SRC")); let s="errors:"+str(len(r.errors))+" n:"+str(len(r.stmts)); let i=0; while i<len(r.stmts){s=s+" ["+r.stmts[i].kind+"]"; i=i+1;} print s;'
for src in 'let x = 42;' 'fn sq(n){ return n*n; }' 'if x>10 { return 1; } while x>0 { x=x-1; }' 'let a = [1,2,3]; let m = map{k: 1};'; do
  check "parse-stmts '$src'" "${stmtdriver/SRC/$src}"
done

# ---- parser parity: expression precedence trees ----
exprdriver='let r=parse(tokenize("SRC;")); print ser(r.stmts[0].expr);'
for src in 'a + b * c - d' 'f(x, y + 1)' 'a.b.c[i]' '1 + 2 * 3 == 7' 'not a and b or c'; do
  check "parse-expr '$src'" "${exprdriver/SRC/$src}"
done

echo "selfhost_parity: $PASS/$((PASS+FAIL)) passed$( [ $FAIL -gt 0 ] && echo " ($FAIL FAILED)" )"
[ $FAIL -eq 0 ]
