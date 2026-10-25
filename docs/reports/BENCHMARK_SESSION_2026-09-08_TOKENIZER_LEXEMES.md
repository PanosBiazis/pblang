# Tokenizer lexeme construction measurements — 2026-09-08

Baseline 072ff3e on the same dirty Linux x86-64 checkout. PBLang-owned changes
only; no Rust edit, toolchain installation or host executable refresh.

## Performance regression concern remains open

The preceding 5.97 to 7.22 ms analyzer observation was unfavorable. It is not
dismissed and has not been proved resolved. Current independent ten-analysis
controls: baseline median 5.998 ms, initial slice/join 6.538 ms, final one-scalar
fast path 6.891 ms. Five samples show scheduling variation, but no end-to-end
speedup. All first/samples/p95 statistics are retained in
bench/results/tokenizer-taint-control-2026-09-08.jsonl.

## Paired comparison, same process and alternating order

bench/tokenizer_lexeme_pair.sapp alternates reference/candidate then
candidate/reference, takes two warmup pairs and twelve retained pairs per case.
No retained samples are removed. Fixed fixture results are compared outside timed
loops against the reference's full JSON. Small program: 200 tokenizations/sample;
mixed program: 100; long lexemes: one at 16384 scalars; taint: 100 analyses.

Reference tokenizer is an exact temporary copy from 072ff3e. Reference taint is
from that same revision, with only its TOKENIZER import redirected to the
reference module; all other imports use the current checkout. Both therefore
retain the same source/token/finding safety controls. No bound is disabled to
make the candidate appear faster.

| Workload | Final paired median candidate/reference | Confirmation |
| --- | ---: | ---: |
| taint-analyze | 1.0173 | 1.0064 |
| small-program | 1.0088 | 0.9861 |
| mixed-program | 0.9664 | 0.9754 |
| identifier | 0.8588 | 0.9386 |
| line-comment | 0.8974 | 0.8892 |
| block-comment | 0.8682 | 0.8855 |
| decimal | 0.8526 | 0.8343 |

Below 1 means less elapsed time for the candidate. Long-token confirmation
reductions are approximately 6–17%; they are not application/compiler-wide gains.
Small-program ratios cross 1 between runs. Taint remains approximately 0.6–1.7%
higher by the paired median-ratio metric, so its overhead remains an investigation
item. Median of ratios need not equal ratio of medians. Peak allocation was not
measured: slice + joined string coexist and memory improvement is NOT established.

Raw pairs: bench/results/tokenizer-paired-2026-09-08.jsonl. Four stages remain:
initial-lexemes-only (before the taint row was added), slice-join-initial,
single-scalar-fastpath, single-scalar-confirmation. Adding the taint row changed
allocation/cache history; compare reference/candidate within a stage, not raw
times across those harness versions. This is an exploratory and confirmation
record, not a preregistered statistical study.

## Independent long-token size sweep

bench/tokenizer_lexeme_stages.sapp uses sizes 128, 2048, 16384 and four token kinds:
identifier, line comment, block comment and decimal. Each is one token plus EOF,
with exact token kind/text checked outside timing. One first sample, two warmups,
five retained samples, no removal. Raw before/initial/final data:
bench/results/tokenizer-lexemes-2026-09-08.jsonl. Initial long-token gains looked
larger than final paired gains. Some final 128-scalar measurements were worse
than baseline; these unfavorable samples are retained, not replaced. Do not
infer a universal performance improvement or promote only favorable cases.

## Separate numeric comparison

Existing session_compare integer_kernel, one million iterations, status=ok and
checksum 474518 on all nine paths. One first run, one warmup, five retained runs;
compiled paths one first build and three repeated builds.

| Execution path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 277.902 | 284.025 | 8720.000 | N/A |
| pblang-c99 | 7.067 | 7.287 | 1572.000 | 14472 |
| pblang-codegen-x64 | 3.495 | 3.650 | 264.000 | 507 |
| c | 7.586 | 7.835 | 1572.000 | 14472 |
| cpp | 7.888 | 8.323 | 3360.000 | 14472 |
| go | 9.515 | 9.704 | 1372.000 | 1228952 |
| rust | 8.291 | 8.391 | 1948.000 | 350560 |
| python | 199.388 | 209.596 | 10252.000 | N/A |
| javascript | 134.245 | 135.468 | 51444.000 | N/A |

Raw: bench/results/session-2026-09-08-tokenizer-lexemes.jsonl.
This numeric workload does not execute the modified tokenizer at runtime.
The retained interpreter remains slower than Python/Node here. No universal
language superiority, embedded-polyglot or other-device/platform claim.

## Reproduction and limits

From repository root, use the fixed stages script, the unchanged
bench/taint_admission_stages.sapp, and session_compare with each language id.
For paired reproduction, make local reviewed reference copies from the revision
above. In the taint copy replace only
let TOKENIZER = import("stdlib/compiler/tokenizer.sapp");
with let TOKENIZER = import(env_get("PB_TOKENIZER_REFERENCE"));
then provide PB_TOKENIZER_REFERENCE and PB_TAINT_REFERENCE to the paired script.
These imports execute module code: never point them at unreviewed sources.

Run sequentially with a 2-GiB address-space limit, core dumps disabled, 45-second
outer limits for lexical/paired runs, 25 seconds for the tiny taint control and
60 seconds for numeric rows (2-second kill grace). Numeric children have ten
seconds and a 64-MiB file limit. No tests/build/Graphify ran during retained
measurements. Temporary reference copies were local only and are recoverable from
Git; they are not release artifacts.

now_ms/SystemTime is not monotonic; CPU frequency, thermal/cache/GC state and OS
scheduling are uncontrolled. Tiny five-sample p95 is not production tail latency.
First does not mean flushed-cache cold. Startup/wrappers are included in numeric
runs. Peak RSS is not total allocation and artifact sizes omit dependent runtimes/
libraries/packaging. Missing metrics are not zero; RSS/CPU *_ms fields represent
KiB/percent. No result here proves hard resource enforcement or a hostile sandbox.
