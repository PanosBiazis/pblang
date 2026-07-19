# Parser, polyglot, and competitor benchmark report

Date: 2026-07-14. Host evidence is in `environment.txt`. Timed competitor runs
were pinned to one CPU with `taskset -c 3`, use best-of-three process time, and
include startup. Fixed fib input permits compiler specialization in every
language; these results do not establish universal performance dominance.

## Changes under test

- `compiler/parser.sapp` constructs internal AST nodes with one map allocation
  instead of allocating an `extra` map and copying it through `map_keys`.
- `polyglot_registry.sapp` keeps immutable language/alias metadata once per
  module and builds a cached extension index.
- `polyglot_router.sapp` adds deterministic alias/file/task routing and secure
  execution planning without invoking a foreign runtime.
- `polyglot.sapp` adds a content-addressed pblang fallback for D cdylibs with
  compiler-specific `dmd`/`ldc2`/`gdc` argv and atomic cache publication.

## Focused pblang benchmarks

`bench/selfhost_parser_stages.sapp`:

| Phase | Work | Result |
|---|---:|---:|
| direct AST node map | 100,000 nodes | 164.675 ms |
| compatibility `make_node` | 100,000 nodes | 396.782 ms |
| direct/compatibility speedup | | 2.41x |
| small tokenize+parse | 5,000 rounds | 598.882 ms |
| medium tokenize+parse | 2,000 rounds | 1,341.863 ms |

`bench/polyglot_control_plane_stages.sapp` before/after registry caching:

| Phase | Before | After | Change |
|---|---:|---:|---:|
| registry routing, 100 rounds | 29.302 ms | 0.227 ms | 129.0x faster |
| aliases, 10,000 rounds | 47.619 ms | 7.496 ms | 6.35x faster |
| process peak RSS | 19,452 KiB | 17,416 KiB | 10.5% lower |
| filename routing, 10,000 rounds | n/a | 70.868 ms | new phase |

Guard and policy phases remained stable within run noise because their security
logic was intentionally unchanged.

## Competitor results

Full tables and unavailable-toolchain rows are in `fib.txt` and `loop.txt`.

| Workload | pblang direct x86-64 | Best measured peer | pblang artifact |
|---|---:|---:|---:|
| recursive fib(32) | 1 ms | Fortran 1 ms | 405 B |
| 10,000,000 register loop | 3 ms | Rust/C++ 4 ms | 503 B |

The pblang interpreter measured 837 ms on fib(32) and 2,832 ms on the loop, so
interpreter dispatch/call/scope work remains the highest-value optimization
target. Resource measurements in `resources.txt` show 264 KiB peak RSS for the
direct x86-64 fib executable versus 1,512 KiB for GCC C, 2,060 KiB for Rust,
and 8,628 KiB for the pblang interpreter.

## Verification

- canonical semantics: 646 passed, 0 failed, 3 intentionally serial filters
- serial polyglot cached/native: passed
- available direct cdylib roundtrips: Rust, C, C++, C#, F#, D, Nim, Fortran,
  Objective-C passed; unavailable runtimes and RTS-specific ABIs are explicit
  skips
- self-host subset: 39 passed
- explicit legacy full-parser oracle: 1 passed in 0.96s
- codegen smoke: 254/254 passed
- phase benchmark: `PHASE_SUITE|ok`
