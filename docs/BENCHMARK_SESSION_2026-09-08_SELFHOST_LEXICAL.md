# Selfhost lexical/build measurements — 2026-09-08

Baseline `9274342`, same dirty checkout before/after. No Rust edits, tool
installation, native host rebuild or physical-device qualification.

## Parser control

Unchanged `bench/parser_match_stages.sapp` parses an already-supported match
expression from pretokenized input fifty times per sample. The last AST's
statement count, error count and match node are checked outside the timed region.
One first sample, two warmups and five retained samples per stage; no timed
malformed-to-success comparison. Tokenization, codegen and execution are excluded.

| Stage | Median ms / 50 parses | p95 ms / 50 parses |
| --- | ---: | ---: |
| Before | 7.198 | 7.493 |
| After | 7.432 | 7.593 |

Raw first values and all retained samples:
`bench/results/selfhost-lexical-2026-09-08.jsonl`.
The final median is slightly higher; no speedup is claimed. These tiny samples
use existing now_ms/SystemTime, not a monotonic clock; CPU frequency, temperature,
caches and OS scheduling are uncontrolled. Five-sample p95 is not production
tail latency. One preliminary baseline run was excluded before comparison
because test-process completion had not yet been verified; both retained stages
were run after all test processes completed. No samples inside either retained
stage were removed. No allocation, parser RSS, tokenizer or security-pass speed
claim is made.

## Separate numeric comparison

Unchanged session_compare integer_kernel, 1,000,000 iterations, exact checksum
474518. All nine paths passed. One first run, one warmup and five retained run
samples; compiled paths additionally one first build and three repeated builds.

| Execution path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 266.407 | 268.881 | 8736.000 | N/A |
| pblang-c99 | 4.582 | 5.209 | 1572.000 | 14472 |
| pblang-codegen-x64 | 3.691 | 3.761 | 264.000 | 507 |
| c | 7.709 | 8.115 | 1524.000 | 14472 |
| cpp | 8.279 | 8.411 | 3436.000 | 14472 |
| go | 9.399 | 9.448 | 1500.000 | 1228952 |
| rust | 8.105 | 8.165 | 2024.000 | 350560 |
| python | 201.767 | 203.501 | 10200.000 | N/A |
| javascript | 137.158 | 143.219 | 51444.000 | N/A |

Raw timings, CPU/RSS summaries, first/repeated build times and source/artifact
sizes: `bench/results/session-2026-09-08-selfhost-lexical.jsonl`.
This numeric kernel does not invoke the changed tokenizer/parser/security APIs
at runtime and is not evidence of their speed. The retained PBLang interpreter
remains slower than Python/Node in this workload. Native timing cannot establish
interpreter superiority; Node may JIT. No embedded-polyglot benchmark ran.

Same local Linux x86-64 host and installed toolchains as the immediately prior
`BENCHMARK_SESSION_2026-09-08_ARRAY_WINDOWING.md` session. Existing compiler flags
unchanged. Child startup/wrappers are included; RSS is process peak, not total
allocation. Artifact sizes exclude dependent runtimes/libraries/packaging. First
run is not flushed-cache cold; repeated build caches remain uncontrolled.
Unavailable/absent metrics are not zero. The stats helper's RSS/CPU *_ms fields
represent KiB/percent, not milliseconds.

## Reproduce sequentially from repository root

```bash
ulimit -c 0
ulimit -v 2097152
ulimit -f 65536
timeout --kill-after=2s 30s target/release/pblang run bench/parser_match_stages.sapp --allow-all
PB_BENCH_LANGUAGE=python PB_BENCH_WORKLOAD=integer_kernel timeout --kill-after=2s 60s target/release/pblang run bench/session_compare.sapp --allow-all
```

Select each row's language id for numeric comparison. Numeric children have a
ten-second bound; outer processes use sixty seconds plus two-second kill grace,
2-GiB address space and 64-MiB file limits. No test/build/Graphify work ran during
the retained measurements. These trusted workload limits are not hostile-code
containment or cross-platform evidence.
