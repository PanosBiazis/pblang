#!/usr/bin/env bash
# pbi_parity.sh — the SELF-HOSTED pblang interpreter (tools/pbi.sapp = the pblang-written tokenizer + parser
# + evaluator) must produce the SAME output as the reference Rust VM for every program in its supported
# subset. This is the "pblang running pblang" fidelity guard: `pblang run <p>` == `pblang run tools/pbi.sapp <p>`.
set -u
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"   # pl-dev/pblang
PB="${PB:-$DIR/target/release/pblang}"
W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
PASS=0; FAIL=0

check() {  # label  program-source
  local label="$1" prog="$2"
  printf '%s\n' "$prog" > "$W/p.sapp"
  local vm pi
  vm=$( cd "$DIR" && "$PB" run "$W/p.sapp" --allow-all </dev/null 2>&1 )
  pi=$( cd "$DIR" && "$PB" run tools/pbi.sapp "$W/p.sapp" --allow-all </dev/null 2>&1 )
  if [ "$vm" = "$pi" ] && [ -n "$vm" ]; then PASS=$((PASS+1))
  else echo "FAIL [$label]:"; echo "   VM : $(printf '%s' "$vm" | tr '\n' '|')"; echo "   pbi: $(printf '%s' "$pi" | tr '\n' '|')"; FAIL=$((FAIL+1)); fi
}

check "arithmetic precedence" 'print 2 + 3 * 4 - 1;'
check "recursion (fib)"       'fn fib(n){ if n<2 { return n; } return fib(n-1)+fib(n-2); } print fib(16);'
check "while + modulo"        'let n=0; let s=0; while n<30 { if n%4==0 { s=s+n; } n=n+1; } print s;'
check "for count"             'let s=0; for i 6 { s=s+i*i; } print s;'
check "for range inclusive"   'let s=0; for i in 3 to 7 { s=s+i; } print s;'
check "break / continue"      'let s=0; for i 20 { if i==10 { break; } if i%2==0 { continue; } s=s+i; } print s;'
check "closures (curry)"      'fn add(a){ return fn(b){ return a+b; }; } let f=add(40); print f(2)+f(0);'
check "arrays index + sum"    'let a=[5,10,15,20]; let t=0; let i=0; while i<len(a) { t=t+a[i]; i=i+1; } print t;'
check "array push + pop"      'let a=[]; push(a,3); push(a,4); push(a,5); let x=pop(a); print x + len(a);'
check "array write"           'let a=[1,2,3]; a[1]=99; print a[0]+a[1]+a[2];'
check "maps + map_keys"       'let m=map{x:1,y:2,z:3}; let s=0; for k in map_keys(m) { s=s+m[k]; } print s;'
check "map member + insert"   'let m=map{a:10}; m.b=20; print m.a+m.b;'
check "map_has"               'let m=map{k:1}; if map_has(m,"k") { print 1; } else { print 0; }'
check "strings + equality"    'let s="cat"; if s=="cat" { print 1; } else { print 0; } if s!="dog" { print 2; }'
check "str_split + for-in"    'let w=str_split("a,bb,ccc",","); let t=0; for p in w { t=t+len(p); } print t;'
check "str_slice + charcode"  'let s="pblang"; print str_slice(s,0,2); print strcharcodeat(s,0);'
check "parse_number"          'print parse_number("123") + parse_number("77");'
check "mixed print (num+str)" 'fn f(b){ if b { return "yes"; } return 7; } print f(1); print f(0);'
check "nested data"           'let g=map{n:"pb",xs:[1,2,3]}; let t=0; for v in g.xs { t=t+v; } print g.n; print t;'
check "compound assign"       'let x=5; x+=3; x-=1; x*=4; print x;'
check "compound in loop"      'let s=0; let i=0; while i<6 { s+=i*i; i+=1; } print s;'
check "str builtins"          'print str_trim("  hi  "); if str_starts_with("pblang","pb") { print 1; } print str_replace_all("a.b.c",".","-");'
check "power operator"        'print 2 ** 10; print 3 ** 5;'
check "for k,v in map"        'let m=map{a:1,b:2,c:3}; let s=0; for k, v in m { s+=v; } print s;'
check "for + continue"        'let s=0; for i 8 { if i%2==0 { continue; } s+=i; } print s;'

echo "pbi_parity: $PASS/$((PASS+FAIL)) passed$( [ $FAIL -gt 0 ] && echo " ($FAIL FAILED)" )"
