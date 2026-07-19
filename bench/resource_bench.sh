#!/usr/bin/env bash
# Resource comparison for the recursive Fibonacci workload used by lang_shootout.sh.
# Reports current-process peak RSS from /usr/bin/time and artifact bytes. Every
# result is checked before it is reported; unavailable toolchains are skipped.
set -euo pipefail

N="${1:-32}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PB="${PB:-$ROOT/target/release/pblang}"
CG="$ROOT/stdlib/native/codegen_x64.sapp"
W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT
cd "$W"

expected_fib() {
    if [ "$N" -lt 0 ] || [ "$N" -gt 92 ]; then
        echo "resource_bench: N must be in 0..92 for exact signed 64-bit validation" >&2
        return 2
    fi
    local a=0 b=1 next i
    for ((i=0; i<N; i++)); do next=$((a+b)); a=$b; b=$next; done
    printf '%s\n' "$a"
}
EXPECTED="$(expected_fib)"

cat > fib.sapp <<EOF
fn fib(n) { if n < 2 { return n; } return fib(n - 1) + fib(n - 2); }
print fib($N);
EOF
cat > driver.sapp <<EOF
let cg = import("$CG");
fs_write_bytes("fib_pblang", cg.compile_fns(bytes_to_str(fs_read_bytes("fib.sapp"))));
EOF
"$PB" run driver.sapp --allow-all >/dev/null
chmod +x fib_pblang

cat > fib.c <<EOF
#include <stdio.h>
long fib(long n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
int main(void) { printf("%ld\\n", fib($N)); return 0; }
EOF
cat > fib.rs <<EOF
fn fib(n: u64) -> u64 { if n < 2 { n } else { fib(n - 1) + fib(n - 2) } }
fn main() { println!("{}", fib($N)); }
EOF
cat > fib.go <<EOF
package main
import "fmt"
func fib(n int64) int64 { if n < 2 { return n }; return fib(n-1)+fib(n-2) }
func main() { fmt.Println(fib($N)) }
EOF
cat > Fib.java <<EOF
public class Fib { static long fib(long n){ return n<2?n:fib(n-1)+fib(n-2); }
  public static void main(String[] args){ System.out.println(fib($N)); } }
EOF
cat > fib.js <<EOF
function fib(n){ return n<2?n:fib(n-1)+fib(n-2); }
console.log(fib($N));
EOF
cat > fib.py <<EOF
def fib(n): return n if n < 2 else fib(n-1)+fib(n-2)
print(fib($N))
EOF

command -v gcc >/dev/null && gcc -O2 -o fib_c fib.c
command -v rustc >/dev/null && rustc -O -o fib_rust fib.rs
if command -v go >/dev/null; then GOCACHE="$W/gocache" go build -o fib_go fib.go; fi
command -v javac >/dev/null && javac Fib.java
"$PB" compile fib.sapp --out fib_c99 >/dev/null

printf '%-26s %12s %14s %12s\n' "target" "result" "peak_rss_kb" "artifact_B"
printf '%-26s %12s %14s %12s\n' "--------------------------" "------------" "--------------" "------------"

measure() {
    local label="$1" artifact="$2"; shift 2
    local got rss size="-"
    got=$("$@")
    if [ "$got" != "$EXPECTED" ]; then
        printf '%-26s %12s %14s %12s\n' "$label" "WRONG:$got" "-" "-"
        return 1
    fi
    rss=$({ /usr/bin/time -f '%M' "$@" >/dev/null; } 2>&1)
    [ "$artifact" = "-" ] || size=$(stat -c %s "$artifact")
    printf '%-26s %12s %14s %12s\n' "$label" "$got" "$rss" "$size"
}

measure "pblang direct x86-64" fib_pblang ./fib_pblang
measure "pblang native C99" fib_c99 ./fib_c99
measure "pblang interpreter" - "$PB" run fib.sapp
[ -x fib_c ] && measure "C gcc -O2" fib_c ./fib_c
[ -x fib_rust ] && measure "Rust rustc -O" fib_rust ./fib_rust
[ -x fib_go ] && measure "Go gc" fib_go ./fib_go
[ -f Fib.class ] && measure "Java OpenJDK" Fib.class java -cp "$W" Fib
command -v node >/dev/null && measure "JavaScript Node" - node fib.js
command -v python3 >/dev/null && measure "Python CPython" - python3 fib.py
