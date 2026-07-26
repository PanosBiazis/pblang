# PBLang codegen, build and security baseline - 2026-07-19

## Environment

- OS: Linux 7.0.0-28-generic x86-64
- CPU: Intel Core i7-1195G7, 8 logical CPUs
- PBLang: 0.5.7 release build
- GCC: 13.3.0
- Rust: 1.94.0
- Go: 1.22.2
- Java: 21.0.11
- Mono: 6.8.0.105
- Python: 3.12.3
- Node.js: 18.19.1

## Commands

```sh
./target/release/pblang test semantics --jobs smart --quiet
./target/release/pblang run bench/x64_compile_pipeline_stages.sapp --allow-all
./target/release/pblang run bench/x64_batch_compile_stages.sapp --allow-all
./target/release/pblang run bench/smart_build_stages.sapp --allow-all
./target/release/pblang run bench/reproducible_core_matrix.sapp --allow-all
./target/release/pblang run bench/language_category_matrix.sapp --allow-all
cargo audit --json
```

## Verification

- Canonical PBLang semantics: 714 passed, 0 failed, 0 ignored.
- Reproducible compiled/interpreted core: 22 successful rows, 2 Zig-unavailable
  rows, 0 failed.
- Interpreter category matrix: 54 successful rows, 36 unavailable rows, 0
  failed.
- Refreshed advisory scan: 0 known vulnerabilities and 0 unsound advisories.
  Four unmaintained and one yanked transitive dependency warnings remain.

## Direct x86-64 compiler phases

| Phase | Median |
|---|---:|
| Process startup | 1.480 ms |
| Backend import | 37.404 ms |
| In-process emission | 1.652 ms |
| Full uncached publication | 42.357 ms |
| Verified warm publication | 9.342 ms |

The emitter is 3.90% of full uncached publication. Cold backend loading and
non-emitter orchestration remain the dominant single-file costs.

For six artifacts, independent processes took 249.950 ms, one cold session
took 55.485 ms, and one warm session took 10.653 ms. The warm path is 23.46x
faster than independent processes and publishes each artifact in 1.776 ms.

## Adaptive build orchestration

The smart planner measured 13.01 microseconds per call across nine 100-plan
samples. Creating an explicit reusable build session took 5.084 ms. Within that
session, a cold direct compile took 35.700 ms, verified warm publication took
0.752 ms median, and a six-artifact warm batch took 4.566 ms median (0.761 ms
per artifact). The measured cold-to-warm ratio was 47.49x.

These warm results require reuse of one `smart_session`, whose dependency
fingerprint is an intentional process-local snapshot. Independent CLI processes
still pay startup, module loading and fingerprint costs. The builder never runs
an artifact automatically; release workflows must opt into an execution gate.

## Fixed integer-kernel comparison

| Metric | PBLang direct x86-64 | C | Ratio |
|---|---:|---:|---:|
| Warm compile median | 42.760 ms | 25.005 ms | PBLang 1.71x slower |
| Runtime median | 1.963 ms | 3.973 ms | PBLang 2.02x faster |
| Peak RSS | 264 KiB | 1,496 KiB | PBLang 5.67x smaller |
| Artifact | 507 bytes | 14,472 bytes | PBLang 28.54x smaller |

The direct artifact is 96.50% smaller on disk for this fixture. These results
apply only to the fixed one-million-item integer kernel and do not establish a
universal language ranking. The interpreter remains slower on several category
workloads; for example, its integer-loop median was 30.270 ms versus Python at
24.985 ms and Lua at 2.648 ms.

## Build measurement

The dependency-refresh incremental `cargo build --release` took 58.51 seconds,
used 875,000 KiB peak RSS, and averaged 222% CPU. This measures the retained
bootstrap build after two dependency recompilations, not a clean build and not
the PBLang direct-codegen path.

Raw pipe-delimited samples and environment records are stored beside this file.
