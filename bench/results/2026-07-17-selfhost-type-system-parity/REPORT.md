# Self-hosted recursive type-system and ADT parity

Date: 2026-07-17

## Scope

This pure-PBLang slice extends the self-hosted parser and checker with bounded
recursive type expressions, generic substitution, typed struct fields, typed
enum payloads, and enum-match exhaustiveness diagnostics. No Rust code was
added or changed by this slice.

## Correctness and safety

- `array<T>`, `map<K,V>`, `result<T,E>`, legacy `[T]`, `T?`, and `T!` parse to
  canonical annotation strings, including tokenizer-safe nested `>>` handling;
- type strings are bounded to 4,096 bytes and 64 nesting levels;
- struct fields and enum payload annotations survive parser desugaring;
- generic parameters are unified and substituted in nested argument and return
  positions while non-generic calls retain a direct fast path;
- struct/enum names are predeclared, enabling forward cross-references;
- unknown fields/variants, payload mismatches, conflicting generic arguments,
  non-null violations, and non-exhaustive enum matches are diagnosed;
- struct fields and enum variants are capped at 4,096 before metadata scans;
- parser token arrays remain immutable and reusable across compilations.

## Verification

- all `44/44` self-host semantic tests passed;
- the complete semantic suite passed `677/677`, with zero failed, ignored, or
  filtered tests;
- the bootstrap self-test passed;
- `--check --via-pblang` passed for the parser, checker, frontend, and the new
  benchmark.

## Legacy-path cost

The existing four-statement phase fixture was measured in seven fresh
processes pinned to CPU 2. Each process performs 100 checks after imports and
parsing. The prior checker median is retained in the preceding parity report.

| implementation | median ms/check | change |
|---|---:|---:|
| prior bounded checker | 0.246829 | baseline |
| first recursive-type implementation | 0.307373 | +24.53% vs prior |
| final optimized implementation | 0.266436 | +7.94% vs prior; -13.32% vs first draft |

The remaining 7.94% is reported as feature cost, not as an optimization win.
The accepted optimization uses two-phase declaration hoisting only where type
annotations require it and bypasses generic substitution for proven
non-generic signatures.

## Dedicated phase benchmark

`selfhost_type_system_stages.sapp` uses 16 unique struct/enum/function groups,
30 samples per phase, immutable token reuse, and exact statement/diagnostic
checks. Timings are milliseconds.

| phase | units | p50 | p95 | peak process RSS |
|---|---:|---:|---:|---:|
| recursive type parse | 112 statements | 7.1533 | 7.9385 | 27,136 KiB |
| valid typed ADTs | 112 statements | 7.6194 | 7.8638 | shared process |
| invalid ADTs/exhaustiveness | 96 statements, 48 diagnostics | 6.7209 | 6.8208 | shared process |

The process completed in 0.74 seconds wall time at 99% CPU. The existing
typechecker benchmark separately reported 0.2661 ms/check for the small legacy
AST, 7.1530 ms/check for 128 valid typed statements, and 7.9678 ms/check for
128 statements producing 64 diagnostics, at 20,380 KiB peak RSS.

## Limits

This is an internal compiler-phase benchmark, not a cross-language leadership
claim. Postfix Result propagation, richer typo suggestions, generic constraints,
and full differential parity with the legacy checker remain open work. Dynamic
CPU frequency and the dirty local worktree are recorded in `environment.txt`.
