# Bounded Self-Hosted Diagnostics Benchmark

Date: 2026-07-17

## Scope

This correctness-gated PBLang benchmark measures the self-hosted type checker
and the builtin registry lookup removed from its generic-builtin hot path. It
does not compare languages and does not establish general compiler leadership.

Command:

```sh
./target/release/pblang bench/selfhost_typecheck_stages.sapp
```

Seven independent processes were run without CPU pinning. Reported typecheck
values are milliseconds per `type_check`; registry values are total milliseconds
for 5,000 lookups.

| Phase | Median | Min | Max |
|---|---:|---:|---:|
| legacy-compatible (4 statements) | 0.620716 | 0.607099 | 0.717365 |
| typed signatures (128 statements) | 17.122773 | 16.575156 | 21.102056 |
| typed diagnostics (128 statements, 64 errors) | 18.215259 | 17.761523 | 19.949136 |
| generic builtin lookups (128 statements) | 6.432661 | 6.060156 | 7.218701 |
| legacy registry rebuild (5,000 lookups) | 388.618896 | 382.088135 | 418.727783 |
| cached registry lookup (5,000 lookups) | 1.938477 | 1.907715 | 2.856445 |

The isolated registry median decreased by 99.50%, or 200.48x. This directly
measures the eliminated map-rebuild path. It is not an old-vs-new whole-compiler
comparison because the old compiler benchmark did not contain a generic builtin
workload.

One `/usr/bin/time -v` run completed in 3.08 seconds at 99% CPU with 21,056 KiB
maximum RSS, zero major page faults, and zero swaps. Every run passed malformed
AST, resource-limit, diagnostic-count, builtin-presence, and checksum gates.

Raw samples are in `metrics.txt`; runtime and host details are in
`environment.txt`.
