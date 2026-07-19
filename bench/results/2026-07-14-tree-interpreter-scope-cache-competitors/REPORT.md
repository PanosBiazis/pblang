# Local comparison after tree-interpreter block caching

Date: 2026-07-14

The broad suite used fixed `fib(32)`, a 10,000,000-iteration register loop,
resource measurements, compiler pipeline stages, the complete pblang phase
suite, and ten emulator gates. It was pinned to CPU 3 with three repetitions
and a 45-second per-target timeout.

Direct pblang x86-64 remains 1 ms / 405 bytes for fixed `fib(32)` and 3 ms /
503 bytes for the register loop. Locally installed competitors include Fortran
at 2 ms for fixed Fibonacci, GCC/G++ at 5 ms, Clang and Rust at 6 ms. On the
register loop, pblang direct is 3 ms, Clang C and Rust 4 ms, Clang C++ 5 ms,
and D 6 ms.

The host pblang interpreter measured 914 ms on fixed `fib(32)`, compared with
952 ms in the immediately preceding broad run. Because these are separate
best-of-three process runs, the controlled A-B-A-B results in the sibling
`2026-07-14-tree-interpreter-scope-cache/` report are the accepted interpreter
claim.

All produced results were correct. Missing local toolchains remain marked
`not installed` in `fib.txt` and `loop.txt`. These small, fixed-input local
workloads do not establish universal language or application superiority.
