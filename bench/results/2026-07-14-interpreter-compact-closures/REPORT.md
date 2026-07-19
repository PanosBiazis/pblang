# Pure-pblang interpreter compact closures

Date: 2026-07-14

## Delivered

- Replaced private tree-interpreter closure maps with the fixed layout
  `[tag, params, arity, body, env]`.
- Kept zero-, one-, two-, and generic-arity paths and existing language behavior.
- Cached guard-only `if` nodes and hoisted block statement counts.
- Added nested-return regressions plus control-flow and closure-layout fixtures.
- Added all four tree-interpreter fixtures and peak-RSS capture to the complete
  phase suite.

## Verification

- Canonical semantics: **652 passed, 0 failed, 0 ignored**.
- Native codegen smoke: **254/254 passed**.
- Native self-hosted tree interpreter: **28/28 passed**.
- Emulator gates: **10/10 passed** (6502, AVR, Z80, MIPS, MSP430, PowerPC,
  RISC-V, SPARC, WASM, quantum).
- Polyglot control-plane suite: passed.
- Full phase and broad local competitor suites: passed.

## Isolated representation benchmark

The fixture performs the same four logical closure metadata reads through the
old map layout and the new array layout. Each process alternates order and
reports the median of five samples over 400,000 iterations.

| Fresh pinned run | Map median | Array median | Lower time |
|---:|---:|---:|---:|
| 1 | 193.761 ms | 185.569 ms | 4.23% |
| 2 | 194.597 ms | 185.941 ms | 4.45% |
| 3 | 196.896 ms | 187.652 ms | 4.70% |
| 4 | 197.651 ms | 189.043 ms | 4.36% |
| 5 | 196.330 ms | 187.817 ms | 4.34% |

Independent full-suite processes measured 6.16% and 5.69% lower median time in
the same direction. One phase process reported 9,636 KiB peak RSS for the
combined paired fixture; it does not isolate per-layout memory consumption.

## Current-host comparison

`fib(32)`, best-of-three wall clock including process startup:

| Target | Time | Result | Artifact |
|---|---:|---:|---:|
| pblang direct x86-64 | 1 ms | 2,178,309 | 405 B |
| Fortran `-O2` | 2 ms | 2,178,309 | 16,240 B |
| pblang self-hosted C99 f64 | 4 ms | 2,178,309 | 15,992 B |
| GCC `-O2` | 5 ms | 2,178,309 | 15,992 B |
| Rust `-O` | 6 ms | 2,178,309 | 3,954,888 B |
| pblang primary interpreter | 848 ms | 2,178,309 | n/a |

For the 10,000,000-iteration register loop, direct pblang x86-64 measured 3 ms,
Clang and Rust 4 ms, and the primary interpreter 3,076 ms. These results show
where optimization work remains; they do not establish universal superiority.

## Rejected experiment

An allocation-free scalar return signal with a marked function frame passed
focused recursion, lexical-cache, and nested-return correctness. It slowed the
long recursive batch because the extra frame marker/probe cost exceeded the
saved one-element return-array allocation. The implementation was reverted;
raw before/experimental outputs remain in the adjacent
`2026-07-14-interpreter-control-signals` evidence directory.

## Method limits

- Only toolchains installed on this host are executed; absent rows remain
  `not installed` in the raw shootout.
- Fixed embedded inputs permit compiler specialization.
- Millisecond wall-clock results include startup and are frequency-sensitive.
- The isolated layout benchmark measures metadata access, not complete
  application throughput or heap bytes per closure.
- Raw evidence under `phases/`, `competitors/`, and `polyglot.txt` is the
  authoritative record.
