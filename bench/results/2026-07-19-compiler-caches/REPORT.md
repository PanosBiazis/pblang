# PBLang compiler cache and batch-session report

Date: 2026-07-19. Host and tool version details are in `environment.txt`.

## Problems found

- Single-file direct x86-64 compilation spent most of its time importing the
  PBLang backend in each fresh process.
- Imported modules were compiled again on every self-hosted frontend run.
- PBCS could not encode optimizer opcodes 41/42, trusted trailing/oversized
  input too far, silently converted unsupported constants, and failed to write
  u32 values above 255 correctly.
- Public `.pbck` loader paths used the unbounded VM.
- Artifact fingerprints omitted transitive tokenizer/parser inputs.
- Bootstrap documentation claimed checked-in compiler snapshots that do not
  exist in this checkout.

## Implemented

- PBCS v2 with explicit fused operands, v1 reader compatibility, strict bounded
  structural validation, exact consumption, and corrected u32 encoding.
- Bounded loader execution by default, with explicit options and runtime stats.
- Opt-in persistent self-hosted imported-module cache with compiler/VM/source
  invalidation, digest and PBCS verification, staged publication, and telemetry.
- One-process direct x86-64 compile sessions with one dependency fingerprint,
  bounded multi-job input, safety certificates, ELF verification, cache reuse,
  compile-all-before-publish behavior, and cache failure telemetry.
- Focused tests, runnable example, benchmark drivers, documentation, and honest
  bootstrap status.

No Rust code was added.

## Results

Single-file phase medians were 2.983 ms process startup, 70.479 ms backend
import, 2.911 ms in-process emission, and 74.673 ms full uncached publication.
This confirms import remains the cold single-process bottleneck.

For six different artifacts:

| Mode | Median total | p95 | Median/artifact |
|---|---:|---:|---:|
| Six independent processes | 445.675 ms | 499.126 ms | 74.279 ms |
| One process, cold artifacts | 102.972 ms | 108.266 ms | 17.162 ms |
| One process, warm artifacts | 81.758 ms | 97.063 ms | 13.626 ms |

The cold batch was 4.33x faster than independent processes; verified warm
artifacts were 5.45x faster. This is project-build import amortization, not a
claim that single-file cold compilation became 5.45x faster.

For an imported 300-function module:

| Mode | Median | p95 |
|---|---:|---:|
| Compile on every run | 128.271 ms | 159.936 ms |
| First serialize/verify/publish | 271.464 ms | 290.013 ms |
| Verified warm PBCS load | 84.549 ms | 92.912 ms |

Warm reuse was 1.52x faster. First publication was 2.12x slower, so this cache
benefits repeated runs and is not enabled by default.

## Verification

- `pblang build .`: pass.
- Focused cache, PBCS, loader, and native backend tests: pass.
- Canonical PBLang semantics: 698 passed, 0 failed, 0 ignored.
- Release Rust reference semantics: 203 passed, 0 failed, 1 documented ignore.
- Compiler/backend, C99, interpreter, polyglot, stdlib, product, resource, and
  platform-emulator phase suites: pass.

## Limits and next work

- A fresh one-file process still pays the host dynamic import cost. Eliminating
  it requires a persistent compiler service or a self-hosted loader selected by
  the CLI; no Rust host shortcut was added in this slice.
- The module cache still reads and preprocesses source to prove its key. It does
  not yet cache the main module or provide source-free bootstrapping.
- PBCS checks the artifact cap after the host filesystem read because the
  current builtin does not expose a bounded streaming file read.
- Multi-file jobs are all compiled before publication, but filesystem failure
  during the final per-file renames cannot provide a cross-file transaction.
- PBCS v2 readers accept v1; old v1 readers cannot read v2 fused operands.
- Deterministic checked-in `_pbck` compiler snapshots and freshness CI remain
  unimplemented and must be completed before claiming that bootstrap layer.
- Direct source codegen remains a Linux x86-64 subset; other targets need their
  own equivalent verified sessions rather than silent fallback.

Reproduce with:

```sh
./target/release/pblang bench/x64_compile_pipeline_stages.sapp --allow-all
./target/release/pblang bench/x64_batch_compile_stages.sapp --allow-all
./target/release/pblang bench/selfhost_module_cache_stages.sapp --allow-all
./target/release/pblang examples/compiler_cache/main.sapp --allow-all
```
