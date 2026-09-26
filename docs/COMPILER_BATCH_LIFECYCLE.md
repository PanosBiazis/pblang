# Compiler batch ownership and concurrency

The self-hosted frontend exposes bounded batch compilation:

```text
frontend_compile_many(sources, max_workers = 8)
frontend_compile_file_many(paths, type_options = nil, max_workers = 8)
frontend_compile_project(directory, max_workers = 8)
```

`max_workers` must be a finite integer from 1 through 64. Invalid values are
rejected even for empty batches and before project directory access. Existing
calls keep working with the new default cap of eight. Set the limit to one for
direct sequential compilation and debugging: it starts no workers. A batch of
one item also uses direct compilation. These are per-call limits, not a global
limit shared by independent processes or nested invocations.

Sources and files are processed in windows of at most the selected size. Each
worker captures its own source/path rather than the entire input batch. Every
successfully started worker in a window is joined, including when a later spawn
or an earlier join fails. No more windows are started after such a failure.
Only after joining all started work does the existing sequential fallback run.
Result order and diagnostics remain in input order. File batches omit cyclic
symbol tables in both sequential and parallel modes, as before.

Fallback still recompiles the complete input, including previously successful
items. It is not cancellation, rollback, a consistent filesystem snapshot or an
exactly-once read guarantee. Files can change between the first attempt and its
fallback. A nonterminating compile can prevent a join from returning. Uncatchable
runtime/OOM failures are not covered. The parent retains input and output arrays;
limiting workers does not bound total bytes or reclaim retained runtime cycles.

## Validation

`tests/semantics/frontend_batch_limits_test.sapp` compares sequential/bounded
source, file and project results and exercises invalid limits. Existing parallel
compiler, lexical, numeric and source-analysis suites remain relevant. The
`examples/compiler_batch/main.sapp` example uses direct sequential debug mode.

Fault injection with real worker threads reproduced the old missing joins:
failed second spawn left one handle unjoined; failed first result in a 12-source
batch left 11. The new implementation left zero unjoined handles in both cases,
and its default maximum outstanding handle count was eight. Injected failure
wrappers reaped baseline handles after recording evidence. This measures owned
handles, not a claim that every unjoined OS thread was still executing.

## Measured tradeoffs

Compared with `8be7287`, identical Linux retained executable, one warmup and seven
alternating measured samples per variant; values include startup:

| Workload | CPU median before/after s | Peak RSS median before/after KiB | Elapsed median before/after s | Elapsed p95 before/after s |
| --- | --- | --- | --- | --- |
| Four small sources | 0.63 / 0.62 | 122720 / 123396 | 0.58 / 0.57 | 0.59 / 0.64 |
| 32 sources with large comments | 2.42 / 2.31 | 584916 / 589620 | 1.32 / 1.35 | 2.18 / 1.48 |
| 16 small files | 1.31 / 1.32 | 317692 / 321152 | 0.92 / 0.96 | 1.42 / 1.67 |
| Single source control | 0.59 / 0.59 | 36876 / 36728 | 0.60 / 0.60 | 0.62 / 0.62 |

CPU fell about 4.5% for the large-comment batch, but its median elapsed time and
peak RSS increased slightly. The file batch and some tail times worsened.
Seven-sample nearest-rank p95 is the largest observed sample; timings vary with
host activity. No general speedup or RAM saving is demonstrated. Windows/macOS,
general runtime/FFI/GC, legacy `--check --via-pblang` status and installed product
integration remain separate work. This change does not establish a Live View
benefit or resolve the earlier async waiting regressions.
