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

Sources and files are divided into contiguous groups of `ceil(count / limit)`
items. Each worker processes its group sequentially, so a call starts at most
the selected worker count in total. A batch no larger than the limit still uses
one item per worker. Every worker captures only its item/group, rather than the
whole input array. Successful joins are flattened in input order.

Every successfully started worker is joined, including when a later spawn or an
earlier join fails. Only after joining all started work does the existing
sequential fallback run. Result order and diagnostics remain in input order.
File batches omit cyclic symbol tables in both sequential and parallel modes,
as before. Work distribution is static: an unusually expensive group can delay
completion while other workers are idle. There is no work stealing or deadline.

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

## Grouped worker reuse (2026-09-26)

The next change avoids fresh child-VM snapshots for each window. Against
`a86e385`, identical executable/workloads, one warmup and seven alternating
measured runs per variant (including startup):

| Workload | CPU median before/after s | Peak RSS median before/after KiB | Elapsed median before/after s | Elapsed p95 before/after s |
| --- | --- | --- | --- | --- |
| Four small sources | 0.61 / 0.61 | 122772 / 122780 | 0.57 / 0.57 | 0.57 / 0.58 |
| 32 sources with large comments | 1.73 / 0.90 | 588664 / 220532 | 1.02 / 0.65 | 1.24 / 0.77 |
| 16 small files | 1.20 / 0.91 | 321664 / 188656 | 0.88 / 0.76 | 1.01 / 0.90 |
| Single source control | 0.57 / 0.57 | 36832 / 36628 | 0.58 / 0.58 | 1.46 / 0.65 |

The large-source batch used about 48% less CPU and 63% less peak RSS; the file
batch about 24% less CPU and 41% less peak RSS. Small/control medians were almost
unchanged, and the small-batch elapsed p95 increased slightly. An exploratory
fresh compiler import per worker cost 2.94 versus 0.62 s CPU on four sources and
was rejected. It is not shipped.

Real-thread instrumentation confirmed 32 sources used eight total workers and
17 files used six, with zero unjoined handles on normal, partial-spawn and
join-failure paths. `frontend_batch_groups_test.sapp` compares complete bytecode
and diagnostics for mixed valid/invalid inputs, uneven groups, several limits,
file inputs and spawn-denied fallback. No general GC/FFI cleanup, old CLI status
fix, async waiting fix or installed PB-MCP/Live View benefit is established.

## Small-load follow-up

The four-source and single-source controls were repeated against `a86e385`, with
the grouped implementation `cc43089`, identical executable, two warmups and 31
alternating measured runs per variant. Process CPU was measured using child
resource usage and elapsed time with a monotonic clock; startup is included.

| Workload | CPU median before/after s | Peak RSS median before/after KiB | Wall median before/after s | Wall p95 before/after s |
| --- | --- | --- | --- | --- |
| Four small sources | 0.89114 / 0.94115 | 123600 / 123908 | 0.82496 / 0.86250 | 1.23135 / 1.02876 |
| Single source control | 0.55247 / 0.55176 | 36748 / 36836 | 0.55535 / 0.55448 | 0.60117 / 0.77055 |

Nearest-rank p95 is the 30th sorted observation of 31. Four-source p95 improved,
but its median CPU rose about 5.6%; the single-source median was essentially
unchanged while its p95 worsened. Host activity varied. These mixed observations
do not prove removal of previous overhead or explain all tail latency. No new
compiler optimization is claimed from this follow-up, and no weaker validation
or default safety check was introduced to improve a benchmark score.
