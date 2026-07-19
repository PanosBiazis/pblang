# pblang language-category and platform-emulator baseline

Date: 2026-07-15  
Host scope: this local machine and the toolchains available on its `PATH`  
Implementation scope: benchmark generators, runner, and platform model are pblang

## Correctness gates

- Canonical semantics suite: 654 passed, 0 failed, 0 ignored.
- File-by-file `.sapp` suite: 651/651 files passed.
- Language matrix: 90 rows, 54 passed, 36 unavailable, 0 failed.
- Execution matrix: ten ISA/quantum models plus one platform/ABI model passed.
- Every measured language row printed the exact expected checksum.

## Language matrix methodology

`bench/language_category_matrix.sapp` runs one warm-up followed by three
measured subprocess samples for each installed runtime/category pair. The
reported time includes process startup. Categories are intentionally separate:
startup, recursive `fib(22)`, 100,000 integer/modulo iterations, 20,000-element
array construction/traversal, 5,000 repeated string appends, and 5,000 hash-map
insertions/lookups.

Supported definitions are pblang, Python, JavaScript, Ruby, Perl, Lua, PHP,
Tcl, AWK, Bash, R, Julia, Raku, Dart, and Swift. This host had nine available:
pblang, Python, JavaScript/Node, Ruby, Perl, Lua, Tcl, AWK, and Bash. Missing
toolchains were not silently removed.

## pblang interpreter medians

| Category | Median wall time |
|---|---:|
| Startup | 1.736 ms |
| Recursion | 9.133 ms |
| Integer loop | 31.681 ms |
| Array | 13.260 ms |
| String | 3.304 ms |
| Hash map | 6.236 ms |

The complete matrix is in `language-categories.txt`; process resource metrics
are in `language-categories-resources.txt`. The matrix process completed in
8.95 seconds with 51,296 KiB peak RSS.

## Platform matrix

The platform stage resolves twelve representative OS/architecture targets per
round, normalizes sandboxed paths, exercises bounded memory, and dispatches
portable deterministic syscalls. The recorded rates were:

| Stage | Work units/s |
|---|---:|
| Resolve 12 targets | 27,837 |
| Normalize 64 paths | 131,318 |
| Bounded memory | 1,208,251 |
| Portable syscall dispatch | 51,144 |

Full phase and resource data are under `phases/`.

## Interpretation boundary

These results do not prove that pblang is globally faster than every language.
They measure interpreter subprocess workloads on one host. Existing direct
x86-64, C99, compiler-phase, artifact-size, peak-RSS, polyglot, and ISA tests
cover different surfaces and must remain separate. Likewise, the platform
emulator is a bounded userspace/ABI contract, not a complete implementation of
every kernel release, driver, GUI stack, or proprietary mobile framework.

The clearest interpreter optimization targets from this baseline are integer
loops first, then array construction/traversal, followed by recursive calls and
hash-map operations. Future changes should retain these raw files and compare
fresh pinned before/after samples rather than replacing the baseline.
