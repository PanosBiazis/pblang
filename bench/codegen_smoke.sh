#!/usr/bin/env bash
# codegen_smoke.sh — regression suite for the pblang-written native code generator
# (stdlib/native/codegen_x64.sapp). Each case compiles a pblang program to a native
# x86-64 ELF and checks its exit code. Covers arithmetic + the optimization pipeline
# (constant folding, algebraic identities, immediate operands, inlining, strength
# reduction), control flow, functions + recursion, and memory-safe heap arrays.
set -u
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"   # pl-dev/pblang
PB="${PB:-$DIR/target/release/pblang}"
CG="$DIR/stdlib/native/codegen_x64.sapp"
W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
N=0; PASS=0; FAIL=0

t() {  # program  label  expected_exit
  N=$((N+1)); local bin="$W/b$N"
  printf '%s' "$1" > "$W/p$N.txt"
  cat > "$W/d$N.sapp" <<EOF
let cg = import("$CG");
fs_write_bytes("$bin", cg.compile_fns(bytes_to_str(fs_read_bytes("$W/p$N.txt"))));
EOF
  local e; e=$( "$PB" run "$W/d$N.sapp" --allow-all </dev/null 2>&1 | grep -i error | head -1 )
  if [ -n "$e" ]; then echo "FAIL [$2]: compile error: $e"; FAIL=$((FAIL+1)); return; fi
  chmod +x "$bin"; "$bin"; local rc=$?
  if [ "$rc" -eq "$3" ]; then PASS=$((PASS+1)); else echo "FAIL [$2]: got $rc, expected $3"; FAIL=$((FAIL+1)); fi
}

ts() {  # program  label  expected_stdout   — checks printed output (for heap strings)
  N=$((N+1)); local bin="$W/b$N"
  printf '%s' "$1" > "$W/p$N.txt"
  cat > "$W/d$N.sapp" <<EOF
let cg = import("$CG");
fs_write_bytes("$bin", cg.compile_fns(bytes_to_str(fs_read_bytes("$W/p$N.txt"))));
EOF
  local e; e=$( "$PB" run "$W/d$N.sapp" --allow-all </dev/null 2>&1 | grep -i error | head -1 )
  if [ -n "$e" ]; then echo "FAIL [$2]: compile error: $e"; FAIL=$((FAIL+1)); return; fi
  chmod +x "$bin"; local o; o=$( "$bin" )
  if [ "$o" = "$3" ]; then PASS=$((PASS+1)); else echo "FAIL [$2]: got '$o', expected '$3'"; FAIL=$((FAIL+1)); fi
}

t 'let x=7; return x+0;'                                                                "algebra x+0"          7
t 'let x=7; return x*1;'                                                                "algebra x*1"          7
t 'let x=7; return 0+x-0;'                                                              "algebra 0+x-0"        7
t 'return (2+3)*4;'                                                                     "constant folding"     20
t 'return 2 + 3*4 - 1;'                                                                 "precedence folding"   13
t 'let x=5; return x*8;'                                                                "strength reduce *8"   40
t 'let x=5; return x*3;'                                                                "imul *3"              15
t 'fn fib(n){ if n<2 {return n;} return fib(n-1)+fib(n-2); } return fib(13);'           "recursion (fib)"      233
t 'fn fact(n){ if n<2 {return 1;} return n*fact(n-1); } return fact(5);'                "recursion (fact)"     120
t 'fn sq(x){return x*x;} return sq(9);'                                                 "inlining"             81
t 'fn g(a,b,c){ return a*b - c; } return g(6,8,7);'                                     "multi-arg call"       41
t 'fn f9(a,b,c,d,e,f,g,h,i){ return a+b+c+d+e+f+g+h+i; } let z=f9(1,2,3,4,5,6,7,8,9); return z;' ">6 args (9, stack-passed)" 45
t 'fn f9(a,b,c,d,e,f,g,h,i){ if a==0 { return i; } return f9(a-1,b,c,d,e,f,g,h,i+1); } let z=f9(3,0,0,0,0,0,0,0,10); return z;' ">6 args recursion (stack arg threaded)" 13
t 'fn f12(a,b,c,d,e,f,g,h,i,j,k,l){ return a+h+i+l; } let z=f12(1,0,0,0,0,0,0,2,3,0,0,4); return z;' "12 args (reg+stack edges)" 10
t 'let a=[10,20,30]; return a[1]+a[2];'                                                 "array read"           50
t 'let a=[1,2,3]; a[1]=99; return a[1];'                                                "array write"          99
t 'let a=[0,0,0]; let i=0; while i<3 { a[i]=i*i; i=i+1; } return a[2];'                 "array + loop"         4
t 'let a=[1,2]; return a[5];'                                                           "OOB read traps"       134
t 'let a=[1,2,3]; a[9]=7; return 0;'                                                    "OOB write traps"      134
t 'let s=0; let i=1; while i<=10 { s=s+i; i=i+1; } return s;'                           "while loop"           55
t 'let x=5; if x>3 { return 42; } return 0;'                                            "if branch taken"      42
t 'let x=1; if x>3 { return 42; } return 7;'                                            "if branch skipped"    7
t 'let x=5; if x>3 { x=10; } else { x=20; } return x;'                                  "if/else no-fallthrough" 10
t 'let x=1; if x>3 { x=10; } else { x=20; } return x;'                                  "if/else else-branch"  20
t 'let g=0; let x=2; if x==1 { g=1; } elif x==2 { g=2; } else { g=9; } return g;'       "elif middle (no return)" 2
t 'let x=5; if x==1 { return 1; } elif x==2 { return 2; } else { return 9; } return 0;' "elif → else"          9
t 'let x=3; if x==1 {return 1;} elif x==2 {return 2;} elif x==3 {return 3;} return 0;'  "elif last arm"        3
t 'let p=map{x:10,y:20}; return p.x + p.y;'                                             "map literal + get"    30
t 'let p=map{a:5}; p.a=42; return p.a;'                                                 "map set"              42
t 'let p=map{x:1,y:2,z:3}; return p["z"];'                                              "map index-key get"    3
t 'let p=map{x:5}; return p.missing;'                                                   "map missing → trap"   134
t 'let f=fn(x){return x*2;}; return f(21);'                                             "closure non-capturing" 42
t 'fn twice(g,n){return g(g(n));} let inc=fn(x){return x+1;}; return twice(inc,40);'    "higher-order fn"      42
t 'let n=100; let g=fn(x){return x+n;}; return g(5);'                                   "closure captures n"   105
t 'let a=10; let b=20; let f=fn(){return a+b;}; return f()+12;'                         "closure captures 2"   42
t 'let m=map{x:1}; m.y=2; return m.x+m.y;'                                              "dynamic map insert"   3
t 'let m=map{}; m.k=42; return m.k;'                                                    "insert into empty map" 42
t 'let s="foo"; let u="bar"; let n=20; let m=22; return n+m;'                           "num add w/ strings present" 42
t 'fn make(x){ let c=fn(y){return x+y;}; return c(3); } return make(40);'               "nested closure (call inside)" 43
t 'fn mk(x){ return fn(y){return x+y;}; } let a=mk(40); return a(3);'                    "named fn returns capturing closure" 43
t 'fn add(a,b){ return a+b; } return add(20,22);'                                       "interproc: num params still add" 42
ts 'print str(0);'                                                                      "str(0)"               "0"
ts 'print str(12345);'                                                                  "str(n) itoa"          "12345"
ts 'print "n=" + str(7);'                                                               "string interpolation" "n=7"
ts 'let x=41; print str(x+1);'                                                          "str(expr)"            "42"
ts 'fn greet(name){ return "Hi " + name; } print greet("Bob");'                         "interproc string param" "Hi Bob"
ts 'fn tag(n){ return "id-" + str(n); } print tag(7);'                                  "interproc str return" "id-7"
ts 'fn wrap(s){ return "[" + s + "]"; } print wrap(wrap("x"));'                          "nested str-returning fn" "[[x]]"
ts 'let p="Hi "; let q="!"; let f=fn(){ return p+q; }; print f();'                       "closure returns string" "Hi !"
ts 'let f = fn(name){ return "Hi " + name; }; print f("Bob");'                           "fn_lit string param (unify)" "Hi Bob"
ts 'fn greeter(g){ return fn(nm){ return g+nm; }; } let hi=greeter("Hi "); print hi("Bob");' "closure factory (2nd order)" "Hi Bob"
ts 'let g = fn(x){ return x + 1; }; print str(g(41));'                                   "numeric fn_lit param stays num" "42"
ts 'fn adderf(n){ return fn(x){ return x+n; }; } let a5=adderf(5); print str(a5(37));'   "numeric factory stays num" "42"
ts 'let mk=fn(unit){ return fn(v){ return str(v)+unit; }; }; let px=mk("px"); print px(16);' "closure factory + body-inferred capture" "16px"
ts 'fn label(k,n){ return k+"="+str(n); } print label("w", 10*4+2);'                     "str param + numeric expr arg" "w=42"
# ---- native string builtins ----
t  'return len("hello");'                                                               "len(string)"          5
t  'return strcharcodeat("ABC", 2);'                                                    "strcharcodeat"        67
ts 'print str_slice("tokenizer", 0, 5);'                                                "str_slice"            "token"
ts 'let s="abcdef"; print str_slice(s, 2, 4);'                                          "str_slice(var)"       "cd"
t  'if "abc" == "abc" { return 1; } return 0;'                                          "string == equal"      1
t  'if "abc" == "abd" { return 1; } return 0;'                                          "string == unequal"    0
t  'if "abc" == "ab"  { return 1; } return 0;'                                          "string == diff-len"   0
t  'let a="cat"; let b="cat"; if a!=b { return 1; } return 9;'                          "string != false"      9
t  'fn is_kw(s){ if s=="if" {return 1;} if s=="while" {return 1;} return 0; } return is_kw("while");' "keyword match" 1
t  'fn nd(s){ let n=0; let i=0; while i<len(s) { let c=strcharcodeat(s,i); if c>=48 { if c<=57 {n=n+1;} } i=i+1; } return n; } return nd("y2026m07");' "native digit counter" 6
# ---- dynamic arrays (push) ----
t  'let a=[1,2,3]; push(a,4); return a[3];'                                             "push then read"       4
t  'let a=[1,2,3]; push(a,9); return len(a);'                                           "push grows len"       4
t  'let a=[]; push(a,7); push(a,8); return a[0]+a[1];'                                  "push into empty []"   15
t  'let a=[0]; let i=1; while i<=5 { push(a,i*i); i=i+1; } return a[3]+a[5];'           "push in a loop"       34
t  'let a=[]; let i=0; while i<20 { push(a,i); i=i+1; } return len(a);'                 "push GROWS past cap"  20
t  'let a=[]; let i=0; while i<1000 { push(a,i); i=i+1; } return a[999]-a[900];'        "1000 pushes (realloc)" 99
t  'let a=[5]; let i=0; while i<50 { push(a,i); i=i+1; } return a[0]+a[50];'            "orig elem survives realloc" 54
# ---- string indexing (safe), break/continue, %, /, bool ----
ts 'let s="hello"; print s[1];'                                                         "string index s[i]"    "e"
ts 'let s="world"; let r=""; let i=0; while i<len(s){ r=s[i]+r; i=i+1; } print r;'      "reverse via s[i]"     "dlrow"
t  'let s="hi"; return s[9];'                                                           "string OOB traps"     134
t  'let i=0; while i<10 { if i==3 { break; } i=i+1; } return i;'                        "break"                3
t  'let i=0; let n=0; while i<5 { i=i+1; if i==2 { continue; } n=n+1; } return n;'      "continue"             4
t  'let i=0; let c=0; while i<3 { let j=0; while j<3 { j=j+1; if j==2 {break;} c=c+1; } i=i+1; } return c;' "nested break" 3
t  'return 17 % 5;'                                                                     "modulo"               2
t  'return 100 / 7;'                                                                    "integer division"     14
t  'if true { return 42; } return 0;'                                                   "bool true literal"    42
t  'let i=0; while true { i=i+1; if i==7 {break;} } return i;'                          "while true + break"   7
# ---- for loops (all three forms) ----
t  'let a=[10,20,30]; let t=0; for x in a { t=t+x; } return t;'                        "for-in array"         60
t  'let t=0; for i in 1 to 5 { t=t+i; } return t;'                                      "for-range 1..5 incl"  15
t  'let t=0; for i 4 { t=t+i; } return t;'                                              "for i N (0..N-1)"      6
t  'let s=0; for i in 1 to 3 { for j in 1 to 3 { s=s+1; } } return s;'                  "nested for-range"     9
t  'let a=[5,5,5]; let t=0; for x in a { t=t+x; if t>=10 {break;} } return t;'          "for-in + break"       10
# ---- str_split (injected pblang helper) + string-typed elements ----
t  'let p=str_split("a,b,c,d", ","); return len(p);'                                    "str_split count"      4
ts 'let p=str_split("hello,world", ","); print p[1];'                                   "str_split element"    "world"
ts 'let p=str_split("x,yy,zzz", ","); for w in p { print w; }'                          "for-in over split"    $'x\nyy\nzzz'
t  'fn kw(w){ if w=="if" {return 1;} if w=="fn" {return 1;} return 0; } fn cnt(){ let n=0; for x in str_split("if a fn b if"," ") { n=n+kw(x); } return n; } return cnt();' "split+classify keywords" 3
# ---- native tokenizer: build a token-length array (push in loop + nested if/else + str) ----
ts 'fn tl(s){ let o=[]; let i=0; let st=0; let w=0; while i<len(s){ let c=strcharcodeat(s,i); if c==32 { if w==1 { push(o,i-st); w=0; } } else { if w==0 { st=i; w=1; } } i=i+1; } if w==1 { push(o,len(s)-st); } return o; } let r=tl("the quick fox"); print str(r[0])+str(r[1])+str(r[2]);' "native tokenizer" "353"

# ---- Round 56: try/catch, and/or/not, if_expr, negatives, globals ----
t  'let m=map{a:1}; return try m["a"] catch 9;'                                        "try present"          1
t  'let m=map{a:1}; return try m["z"] catch 9;'                                        "try missing default"  9
t  'let a=1; let b=0; if a==1 and b==0 { return 5; } return 0;'                        "and"                  5
t  'let a=0; if a==1 or a==0 { return 7; } return 0;'                                  "or"                   7
t  'let x=5; if not (x==3) { return 8; } return 0;'                                    "not"                  8
t  'let x = if 3>2 { 10 } else { 20 }; return x;'                                      "if_expr"              10
t  'let v=-5; let w=-3; return v*w;'                                                   "negative literals"    15
t  'let a=-100; return a+142;'                                                         "negative + positive"  42
t  'let G=42; fn f(){ return G; } return f();'                                         "global read from fn"  42
t  'let A=10; let B=5; fn add(){ return A+B; } return add();'                          "two globals in fn"    15
t  'let K=map{a:true}; fn has(w){ return (try K["a"] catch nil)!=nil; } if has(1){return 1;} return 0;' "global map + try" 1
# ---- Round 57: string-keyed maps (runtime keys, append) ----
t  'let m=map{fn:1,let:1}; let w="fn"; return m[w];'                                   "runtime string key"   1
t  'let K=map{a:true}; fn has(w){ return (try K[w] catch nil)!=nil; } if has("a"){return 1;} return 0;' "is_keyword hit" 1
t  'let K=map{a:true}; fn has(w){ return (try K[w] catch nil)!=nil; } if has("z"){return 1;} return 0;' "is_keyword miss" 0
t  'let m=map{x:1}; m.y=2; return m.x+m.y;'                                            "dynamic insert x+y"    3
t  'let m=map{}; m.a=1; m.b=2; m.c=3; return m.a+m.b+m.c;'                             "3 appends to empty"    6
t  'let m=map{a:1}; m.a=99; return m.a;'                                               "overwrite key"         99
t  'let m=map{one:1,two:2,three:3}; let k="two"; return m[k];'                         "runtime key middle"    2
# ---- Round 58: array/map element string-typing ----
ts 'let a=["foo","bar","baz"]; print a[1];'                                            "str array element"    "bar"
t  'let m=map{fn:1}; let a=["fn","x"]; return m[a[0]];'                                "array elem as map key" 1
t  'let a=["fn","let"]; let n=0; for w in a { if w=="fn" { n=n+1; } } return n;'       "for-in str array + =="  1
ts 'let t=map{kind:"keyword",line:5}; print t.kind;'                                   "map str field member"  "keyword"
ts 'let t=map{kind:"kw"}; print t["kind"];'                                            "map str field index"   "kw"
ts 'fn mk(k,ln){ return map{kind:k,line:ln}; } let t=mk("ident",3); print t.kind;'     "constructor str field" "ident"
t  'let t=map{kind:"fn",line:9}; return t.line+1;'                                     "numeric field stays num" 10
# ---- Round 59: str_split empty-sep, inlining substitution completeness ----
t  'let c=str_split("abc",""); return len(c);'                                        "str_split empty sep→chars" 3
ts 'let c=str_split("hi",""); print c[0]+c[1];'                                        "chars from empty split"    "hi"
t  'let K=map{fn:true}; fn ik(a){ return (try K[a] catch nil)!=nil; } fn t(b){ if ik(b){return 1;} return 0; } return t("ab");' "inline subst diff-name miss" 0
t  'let K=map{fn:true}; fn ik(a){ return (try K[a] catch nil)!=nil; } fn t(b){ if ik(b){return 1;} return 0; } return t("fn");' "inline subst diff-name hit"  1
t  'fn get(m,k){ return m[k]; } let mp=map{x:7}; let key="x"; return get(mp,key);'    "inline map index w/ param" 7
# ---- self-hosting: the pblang tokenizer, compiled native, matches the interpreter byte-for-byte ----
for inp in 'fn add(a, b)' 'let x = 42' 'if x >= 10 and y != 0' 'return foo.bar[i]'; do
  cat > "$W/it.sapp" <<EOF
let t=import("$DIR/stdlib/compiler/tokenizer.sapp"); let ts=t.tokenize("$inp"); let s=""; let i=0; while i<len(ts){s=s+ts[i].kind+":"+ts[i].text+"|"; i=i+1;} print s;
EOF
  IEXP=$( "$PB" run "$W/it.sapp" --allow-all </dev/null 2>/dev/null )
  cp "$DIR/stdlib/compiler/tokenizer.sapp" "$W/nt.txt"
  printf 'let ts=tokenize("%s"); let s=""; let i=0; while i<len(ts){s=s+ts[i].kind+":"+ts[i].text+"|"; i=i+1;} print s; return 0;\n' "$inp" >> "$W/nt.txt"
  cat > "$W/nt.sapp" <<EOF
let cg=import("$CG"); fs_write_bytes("$W/nt.bin", cg.compile_fns(bytes_to_str(fs_read_bytes("$W/nt.txt"))));
EOF
  "$PB" run "$W/nt.sapp" --allow-all </dev/null >/dev/null 2>&1
  chmod +x "$W/nt.bin" 2>/dev/null; NGOT=$( "$W/nt.bin" 2>/dev/null )
  N=$((N+1))
  if [ "$IEXP" = "$NGOT" ] && [ -n "$IEXP" ]; then PASS=$((PASS+1)); else echo "FAIL [tokenizer-parity '$inp']: interp='$IEXP' native='$NGOT'"; FAIL=$((FAIL+1)); fi
done
# ---- Round 61 (evaluator prep): map_has, map_keys, throw ----
t  'let m=map{a:1,b:2}; if map_has(m,"a") { return 1; } return 0;'                     "map_has present"      1
t  'let m=map{a:1}; if map_has(m,"z") { return 1; } return 0;'                         "map_has absent"       0
t  'let m=map{}; m.k=5; if map_has(m,"k") { return 1; } return 0;'                     "map_has after insert" 1
t  'let m=map{a:1,b:2,c:3}; return len(map_keys(m));'                                  "map_keys count"       3
ts 'let m=map{x:1,y:2}; let k=map_keys(m); print k[0]+k[1];'                           "map_keys are strings" "xy"
t  'let x=5; if x>3 { throw "boom"; } return 0;'                                       "throw aborts (70)"    70
t  'let x=1; if x>3 { throw "boom"; } return 9;'                                       "throw not reached"    9
# ── tagged values: runtime type tags (is_string / is_array / is_map) ───────────────
t  'let s="hi"; if is_string(s) { return 1; } return 0;'                               "is_string on string"  1
t  'let a=[1,2]; if is_string(a) { return 1; } return 0;'                              "is_string on array→0" 0
t  'let n=7; if is_string(n) { return 1; } return 0;'                                  "is_string on num→0"   0
t  'let a=[1,2,3]; if is_array(a) { return 1; } return 0;'                             "is_array on array"    1
t  'let m=map{x:1}; if is_array(m) { return 1; } return 0;'                            "is_array on map→0"    0
t  'let m=map{x:1}; if is_map(m) { return 1; } return 0;'                              "is_map on map"        1
t  'let s="hi"; if is_map(s) { return 1; } return 0;'                                  "is_map on string→0"   0
t  'let n=0; if is_map(n) { return 1; } return 0;'                                     "is_map on zero→0"     0
t  'fn tag(x){ if is_string(x){return 1;} if is_array(x){return 2;} if is_map(x){return 3;} return 0; } return tag(map{});' "tag dispatch → map" 3
# string ops still correct under the tagged layout (bytes moved +8 → +16)
ts 'let a="foo"; let b="bar"; print a+b;'                                              "concat under tag"     "foobar"
t  'let s="hello"; return len(s);'                                                     "len under tag"        5
ts 'let s="world"; print s[0]+s[4];'                                                   "index under tag"      "wd"
ts 'let s="abcdef"; print str_slice(s,1,4);'                                           "slice under tag"      "bcd"
# ── arena region reclamation (arena_mark / arena_reset) ────────────────────────────
t  'let m=arena_mark(); let a=[1,2,3]; arena_reset(m); return len(a);'                 "reset preserves result" 3
t  'let acc=0; let i=0; while i<100000 { let m=arena_mark(); let a=[i,i,i,i]; acc=acc+a[3]; arena_reset(m); i=i+1; } return acc%7;' "reset keeps loop bounded" 3
t  'let m1=arena_mark(); let s="x"; let m2=arena_mark(); let t="y"; arena_reset(m2); arena_reset(m1); return 1;' "nested marks reset" 1
# ── runtime-tag-dispatched print (dynamically-typed values print correctly) ─────────
# pick() returns a string on one path and a number on the other → the compiler types it as string;
# printing its result must dispatch on the RUNTIME tag (string→bytes, number→itoa), not crash.
ts 'fn pick(b){ if b { return "yes"; } return 7; } print pick(1);'                     "dyn print string arm" "yes"
ts 'fn pick(b){ if b { return "yes"; } return 7; } print pick(0);'                     "dyn print number arm" "7"
ts 'fn pick(b){ if b { return "hi"; } return 42; } print pick(1); print pick(0);'      "dyn print both arms"  "hi
42"
ts 'let parts = str_split("a-b-c", "-"); print parts[1];'                              "parse_number-adjacent split" "b"
# runtime-dispatched `+`: string-typed operands that aren't literals concat when strings, add when numbers.
ts 'fn g(b){ if b { return "x"; } return "y"; } print g(1) + g(0);'                    "runtime-+ concat non-literal" "xy"
t  'fn h(b){ if b { return 10; } return 20; } return h(1) + h(0);'                     "runtime-+ add via string-typed fn" 30
# array pop (remove + return last element; length shrinks)
t  'let a=[10,20,30]; let x=pop(a); return x + len(a);'                                 "pop returns last + shrinks" 32
t  'let a=[1,2,3,4,5]; pop(a); pop(a); return len(a);'                                  "pop twice → len 3"    3
t  'let a=[7]; let x=pop(a); return x;'                                                 "pop single element"   7
t  'let a=[]; push(a,1); push(a,2); let x=pop(a); push(a,9); return a[0]+a[1]+x;'       "pop then push roundtrip" 12
# bitwise operators (parser desugars & | ^ << >> to bit_* calls; direct backend does 64-bit integer ops)
t  'return 12 & 10;'                                                                    "bitwise AND"          8
t  'return 12 | 3;'                                                                     "bitwise OR"           15
t  'return 6 ^ 3;'                                                                      "bitwise XOR"          5
t  'return 1 << 4;'                                                                     "shift left"           16
t  'return 255 >> 2;'                                                                   "shift right"          63
t  'let m=13; let k=6; return (m & k) + (m | k) + (m ^ k);'                             "bitwise combined"     30
t  'let h=2166136261; let c=97; h = (h ^ c); return h >> 24;'                           "fnv-ish hash step"    129
t  'return bit_and(bit_not(0), 255);'                                                   "bit_not + mask"       255
# 64-bit integer literals (>= 2^32 must use movabs, not a truncated mov eax)
t  'let x = 4294967296; return x >> 32;'                                                "2^32 literal (movabs)" 1
t  'let x = 4294967296; if x >= 4294967296 { return 7; } return 9;'                     "large-literal compare" 7
t  'let x = 4294967295; return x >> 24;'                                                "0xffffffff literal"   255
t  'let x = 5000000000; if x > 4294967296 { return 1; } return 0;'                      "large-const compare (>2^31)" 1
t  'let x = 8; return x + 5000000000 - 5000000000;'                                     "large-const add/sub"  8
# str_to_bytes → byte array (unblocks native crypto/encoding)
t  'let b = str_to_bytes("AB"); return b[0] + b[1] + len(b);'                           "str_to_bytes"         133
ts 'let b = str_to_bytes("pb"); print str(b[0]); print str(b[1]);'                      "str_to_bytes values"  "112
98"
# injected string builtins (str_trim / str_starts_with / str_replace_all) — written in pblang, match the VM
ts 'print str_trim("   hi there   ");'                                                  "str_trim"             "hi there"
t  'if str_starts_with("pblang","pb") { return 1; } return 0;'                          "str_starts_with true" 1
t  'if str_starts_with("pblang","xy") { return 1; } return 0;'                          "str_starts_with false" 0
ts 'print str_replace_all("a.b.c",".","-");'                                            "str_replace_all"      "a-b-c"
ts 'print str_replace_all("aaaa","aa","b");'                                            "str_replace_all overlap" "bb"
# compound assignment (parser desugars x OP= e → x = x OP e)
t  'let x=5; x += 3; return x;'                                                         "plus-equals"          8
t  'let x=10; x -= 4; x *= 2; return x;'                                                "minus/star-equals"    12
t  'let s=0; let i=0; while i<5 { s += i; i += 1; } return s;'                          "compound in loop"     10
t  'let x=100; x /= 4; x %= 7; return x;'                                               "slash/percent-equals" 4
# power operator ** (parser desugars a ** b → num_pow(a,b))
t  'return 2 ** 7;'                                                                     "power 2**7"           128
ts 'print 2 ** 10;'                                                                     "power 2**10 stdout"   1024
t  'return 3 ** 4;'                                                                     "power 3**4"           81
t  'let b=5; let e=3; return b ** e;'                                                   "power with vars"      125
# for k,v in map + the for-loop continue fix (increment-first desugar; continue must NOT infinite-loop)
t  'let m=map{a:1,b:2,c:3}; let s=0; for k, v in m { s = s + v; } return s;'            "for k,v in map"       6
t  'let m=map{a:1,b:2,c:3,d:4}; let s=0; for k, v in m { if v==3 { continue; } s=s+v; } return s;' "for_map + continue" 7
t  'let s=0; for i 5 { if i==2 { continue; } s=s+i; } return s;'                        "for + continue (no hang)" 8
t  'let s=0; for i in 1 to 6 { if i==3 { continue; } s=s+i; } return s;'                "for_range + continue" 18
t  'let a=[1,2,3,4]; let s=0; for x in a { if x==2 { continue; } s=s+x; } return s;'    "for_in + continue"    8
# strength reduction: x*2^k → shl, x*{3,5,9} → lea, others → imul; all must match multiply semantics
t  'let x=6; return x*8;'                                                               "mul pow2 (shl)"       48
t  'let x=7; return x*9;'                                                               "mul *9 (lea)"         63
t  'let x=9; return x*5;'                                                               "mul *5 (lea)"         45
t  'let x=11; return x*3;'                                                              "mul *3 (lea)"         33
t  'let x=4; return x*31;'                                                              "mul *31 (imul)"       124
t  'let x=5; return x*17;'                                                              "mul *17 (imul)"       85
t  'let x=2; return x*64;'                                                              "mul *64 (shl)"        128
t  'let x=3; return x*1000000;'                                                         "mul big const"        192  # 3000000 & 0xFF
# signed integer printing (VM prints -8, not 2^64-8) — itoa now emits sign
ts 'print 0 - 8;'                                                                       "print negative"       "-8"
ts 'let x = 0 - 5; print x * 3;'                                                        "print neg product"    "-15"
ts 'let x = 0 - 1000000; print x;'                                                      "print neg large"      "-1000000"
ts 'print 7 - 7;'                                                                       "print zero"           "0"
# loop register promotion (main-private globals → r12/r13/r14) + register-direct operands + immediate bitwise
t  'let s=0; let i=0; while i<10 { s=s+i; i=i+1; } return s;'                           "reg-promote accumulator" 45
t  'let h=0; let i=0; while i<50 { h=(h*31+i)&255; i=i+1; } return h;'                  "reg-promote rolling hash" 25
t  'let x=200; return x | 55;'                                                         "bit_or immediate"     255
t  'let x=1000; return x & 255;'                                                       "bit_and immediate"    232
t  'let x=170; return x ^ 255;'                                                        "bit_xor immediate"    85
# SAFETY: a global read by a function must stay in memory (not promoted), else fn/main diverge
t  'let g=5; fn getg(){ return g; } let s=0; let i=0; while i<3 { g=g+10; s=s+getg(); i=i+1; } return s;' "no-promote global read by fn" 75
# a promoted global must survive a call inside the loop (callee saves r12-r14)
t  'fn dbl(x){ return x*2; } let acc=0; let i=0; while i<5 { acc=acc+dbl(i); i=i+1; } return acc;'        "promote across call" 20
# a global captured by a closure must not be promoted
t  'let base=7; let mk=fn(){ return base+1; }; return mk();'                            "no-promote captured global" 8
# register-stack for deeper sub-expression temps (r10/r9 spill instead of memory), incl. non-commutative & calls
t  'let a=3; let b=4; let c=5; let d=6; return (a*b)+(c*d);'                            "deep temps mul+add"   42
t  'let a=100; let b=3; let c=50; let d=2; return (a-b)-(c-d);'                         "deep temps sub (order)" 49
t  'let a=2; let b=3; let c=4; let d=5; return ((a+b)*(c+d)) - 10;'                     "deep temps nested"    35
t  'fn sq(x){ return x*x; } let a=5; return (a+1) + sq(3);'                             "deep temps call-in-right" 15
t  'let s=0; let a=2; let b=3; let i=0; while i<7 { s = s + ((a+i)*(b+i)); i=i+1; } return s;' "deep temps in loop" 238
# power-of-two divide/modulo strength reduction (div → shr / and); results match the unsigned div path
t  'let x=100; return x / 8;'                                                          "div pow2 (shr)"       12
t  'let x=100; return x % 8;'                                                          "mod pow2 (and)"       4
t  'let x=255; return x % 16;'                                                         "mod 16"               15
t  'let x=1000; return (x / 64) & 255;'                                                "div 64"               15
t  'let x=12345; return (x / 3) & 255;'                                                "div non-pow2 (div)"   19
# boolean printing matches the VM (true/false, not 1/0) for comparisons / not / bool literals
ts 'print 10 > 3;'                                                                     "print bool true"      "true"
ts 'print 3 > 10;'                                                                     "print bool false"     "false"
ts 'print 2 + 3 == 5;'                                                                 "print eq true"        "true"
ts 'print not (3 > 5);'                                                                "print not"            "true"
ts 'let a=5; let b=9; print a < b;'                                                    "print var compare"    "true"
t  'let x=8; return (0 - x) & 255;'                                                    "0 - x -> neg"         248
t  'let x=0-5; return 0 - x;'                                                          "0 - (negative)"       5
t  'let x=42; return 0 - (0 - x);'                                                     "double negate"        42
t  'let x=0; if x==0 { return 1; } return 0;'                                          "test x==0"            1
t  'let x=5; if x==0 { return 1; } return 0;'                                          "test x==0 false"      0
t  'let x=0-3; if x<0 { return 7; } return 0;'                                         "signed x<0"           7
t  'let i=5; let s=0; while i>0 { s=s+i; i=i-1; } return s;'                            "while i>0 (test)"     15
# tail-call optimization: self-tail-recursion becomes a loop (constant stack, no overflow)
t  'fn sum(n,acc){ if n==0 { return acc; } return sum(n-1, acc+n); } return sum(100,0) & 255;' "TCO accumulator" 186
t  'fn fact(n,acc){ if n<2 { return acc; } return fact(n-1, acc*n); } return fact(5,1);'   "TCO factorial"        120
t  'fn cd(n){ if n==0 { return 42; } return cd(n-1); } return cd(500000);'                 "TCO deep (no overflow)" 42
t  'fn dn(n,a){ if n==0 { return a; } return dn(n-1, a+1); } return dn(1000000,0) & 255;'  "TCO 1M deep counts"   64
# register-stack path (r14→r10→r9 temps): direct-register add/sub/imul/cmp against the parked temp.
# ((a-b)-(c-d)) - ((e*g)-(h-k)) with f(100,1,2,3,4,5,6,7) = (99-(-1)) - (20-(-1)) = 100 - 21 = 79.
t  'fn f(a,b,c,d,e,g,h,k){ return ((a-b)-(c-d)) - ((e*g)-(h-k)); } return f(100,1,2,3,4,5,6,7);' "register-stack direct-reg ops" 79
t  'fn g(a,b,c,d){ if ((a-b)-(c-d)) > ((a*b)-(c*d)) { return 1; } return 0; } return g(9,2,3,1);' "register-stack compare path" 0
# register-resident loop bound: `while i < n` where n is a register var → cmp rax,reg (no push/pop spill).
t  'fn f(a,n){ let s=0; let i=0; while i<n { s=s+a; i=i+1; } return s; } return f(3,10);'        "cond vs register bound"  30
t  'fn h(n){ let c=0; let i=0; while i<n { let j=0; while j<n { c=c+1; j=j+1; } i=i+1; } return c; } return h(6);' "nested register-bound loops" 36
# constant call arguments → push imm8/imm32 (not mov+push). Covers small +, 0, imm32, and a negative arg.
t  'fn dn(n,a){ if n==0 { return a; } return dn(n-1,a+1); } return (dn(50,0)+dn(30,5)+dn(70,3)) & 255;' "const args via push-imm" 158
t  'fn f4(a,b,c,d){ if a>9000 { return a; } return a+b+c+d; } return f4(1000, 5, 0-3, 100) & 255;'      "const args incl negative"  78
# single constant arg → mov edi,imm32 (or xor edi,edi for 0), skipping the mov rax + mov rdi round-trip.
t  'fn fib(n){ if n<2 { return n; } return fib(n-1)+fib(n-2); } return (fib(10) + fib(0)) & 255;'       "single const arg (incl 0)" 55
# accurate frame sizing: deep NON-tail recursion that would overflow the old flat 512B/frame (~15K deep).
t  'fn s(n){ if n<=0 { return 0; } return n + s(n-1); } return s(20000) & 255;'                         "deep non-tail recursion (sized frame)" 16
# affine-recurrence collapse: `while i<N { h=(A*h+i)&MASK; i++ }` → stride-8 fold by A^8 + remainder loop.
# i<50 exercises the collapsed loop (6 strides) AND the remainder (2 iters); result must match the scalar loop.
t  'let h=0; let i=0; while i<50 { h=(h*31+i)&65535; i=i+1; } return h & 255;'                          "affine-recurrence collapse" 25
t  'let h=0; let i=0; while i<7 { h=(h*31+i)&65535; i=i+1; } return h & 255;'                            "recurrence collapse remainder-only (N<8)" 99
# WIDER collapse: two independent masked accumulators in one loop, each folded separately (+ shared stride).
t  'let a=0; let b=1; let i=0; while i<30 { a=(a*3+i)&255; b=(b*5+i)&255; i=i+1; } return (a+b)&255;'      "multi-accumulator collapse" 59
# POLYNOMIAL collapse: f(i) is quadratic / cubic in the loop var, folded via moment sums.
t  'let h=0; let i=0; while i<40 { h=(h*7+i*i)&65535; i=i+1; } return h & 255;'                          "polynomial collapse (quadratic i*i)" 108
t  'let h=0; let i=0; while i<40 { h=(h*3+i*i*i)&65535; i=i+1; } return h & 255;'                        "polynomial collapse (cubic i*i*i)" 64
# array build with in-place grow (buffer at arena top → no copy) + interleaved (copy-path fallback) both correct.
t  'let a=[];let i=0;while i<100{push(a,i*3);i=i+1;} let s=0;let j=0;while j<len(a){s=s+a[j];j=j+1;} return s & 255;'  "array build in-place grow" 2
t  'let a=[];let b=[];let i=0;while i<50{push(a,i);push(b,i*2);i=i+1;} return (a[49]+b[49]+len(a)) & 255;'  "array build interleaved (copy path)" 197
# hotness register promotion: a COLD var declared first must NOT steal a register from the hot loop vars (a,b,i).
t  'let cold=9; let a=1; let b=2; let i=0; while i<1000 { a=(a+i)&255; b=(b*3+i)&255; i=i+1; } return (a+b+cold)&255;'  "hotness register promotion" 76
echo "codegen_smoke: $PASS/$N passed$( [ $FAIL -gt 0 ] && echo " ($FAIL FAILED)" )"
[ $FAIL -eq 0 ]
