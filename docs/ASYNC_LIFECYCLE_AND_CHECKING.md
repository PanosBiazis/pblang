# Async lifecycle and self-hosted source checking

## Automatic data transfer for parallel maps

`async.map_parallel` automatically tries a data queue when there are at least
64 items, more than one worker, and the first item is an array, a map or a string
of at least 256 characters. This is a cheap shape heuristic, not a full input
cost model or a universal performance guarantee. Small lists, scalar lists and
single-worker calls retain the capture-based backend. The default worker count
and the `async.pool` function-list API are unchanged.

The data worker captures only the mapper and channel handles. Each input is
transferred once through the shared queue rather than copied into every worker.
Ordered results, task-error-as-nil behavior, nil/sentinel-shaped values and caller
data isolation are preserved. Mapper captures are still copied to each worker.

The retained host uses different wire formats for captures and channels: JSON
channels replace non-finite numbers with nil and do not carry functions/modules.
Before starting any worker, a bounded native JSON round trip and value comparison
check each queued item. If it throws or changes a value, the staged queue is
closed/drained and the previous capture backend is used. This fallback occurs
before any mapper effects; it never retries executed tasks. Exact numeric inputs
also take the fallback when JSON changes their types, preserving the retained
capture route's existing string conversion. Cyclic/deep values remain subject to
the retained transfer limits. This is not a new exact-numeric wire format.

Started workers are joined on partial launch failure or output-transfer failure.
Result-channel allocation failure drains already queued input. A nonterminating
mapper can still delay return indefinitely. Queue slots are bounded by item
count, but total input/output bytes and application concurrency need their own
budgets. Retained closed channel registry entries are not reclaimed here.

The 23 focused suites include ordered results, isolated mutation, non-finite and
exact numeric inputs, function fallback without duplicate effects, cyclic input,
and sibling cleanup on result failure. Fault injection checks failed first/second
spawn, result-channel allocation, fallback and abandoned payload cleanup. See
`tests/semantics/async_map_transfer_test.sapp` and
`examples/async_data_map/main.sapp`.

Against `bb9ca45`, with identical Linux retained executables, one warmup and nine
alternating measured samples:

| Workload | CPU median before/after (s) | Peak RSS median before/after (KiB) | Elapsed median before/after (s) |
| --- | --- | --- | --- |
| Three maps, 1024 rows of 64 numbers, eight workers | 0.22 / 0.12 | 43816 / 28768 | 0.14 / 0.12 |
| Ten maps, 1024 scalars, eight workers | 0.12 / 0.12 | 28184 / 28272 | 0.07 / 0.07 |
| 32 waits of 10 ms, default workers | 0.05 / 0.05 | 23196 / 23112 | 0.09 / 0.09 |
| 128 waits of 5 ms, default workers | 0.05 / 0.04 | 23236 / 23172 | 0.13 / 0.13 |
| 40 small four-item maps | 0.06 / 0.06 | 32864 / 32888 | 0.05 / 0.05 |
| Ten function pools of 64 tasks | 0.07 / 0.06 | 27932 / 27920 | 0.05 / 0.05 |
| 500 futures control | 0.08 / 0.08 | 48456 / 48348 | 0.07 / 0.07 |

The large-row workload used about 45% less CPU and 34% less peak RSS. Timer
resolution is 0.01 s; nine-sample nearest-rank p95 is the observed maximum.
Scalar elapsed p95 worsened 0.09 to 0.11 s; future elapsed p95 0.07 to 0.08 s.
An exploratory queue-for-all-large-lists variant increased scalar CPU and was
not retained. Prior async/thread waiting regressions are not proven eliminated;
there is no general RAM, energy, GPU/NPU or Live View improvement claim.

## Owned cleanup for thread pools

`thread.thread_pool` and its `thread.parallel_map` caller now use a small,
on-demand `thread_pool_worker` module. They preserve completion-order results,
including nil and marker-shaped maps. This is distinct from the ordered,
task-failure-as-nil contract of `async.pool`.

Input values are queued before any worker starts. If transfer fails, the queue
is closed and drained and no mapper has run. This deliberately removes the old
possibility of partial mapper effects before discovering a bad later input.
Jobs are wrapped as data and the jobs channel is closed after queuing; separate
stop messages are no longer needed. Job/result queues each hold at most the
input count. The complete input and output still require application budgets.

On a partial spawn failure, a mapper exception or a non-transferable result,
every successfully started worker is joined before reporting the error. Joining
continues after an individual failure. Both queues are closed and abandoned
payloads are drained. Existing workers may finish queued jobs before that error
returns. This is cleanup, not cancellation or rollback of external effects.
A nonterminating mapper can still prevent return; runtime aborts and allocation
failures outside catchable paths are not covered. Closed registry entries remain
in the retained host; this is not general handle or cycle reclamation.

Compatibility: nil/zero/negative finite worker counts keep the legacy one-worker
fallback. Positive fractions and non-finite/nonnumeric counts are now rejected.
Valid counts clamp to the input length, and empty input starts no worker. Failure
still throws, but cleanup reports a generic pool-stage error instead of the
original mapper exception text. Code matching old exception strings must migrate.
See `examples/thread_pool_cleanup/main.sapp` and
`tests/semantics/thread_pool_ownership_test.sapp`.

Against source revision `37ee7aa`, identical Linux retained executables, one
warmup and seven alternating measured samples produced these process medians:

| Workload | CPU before / after (s) | Peak RSS before / after (KiB) | Elapsed before / after (s) |
| --- | --- | --- | --- |
| 40 pools, four workers, 32 simple jobs each | 0.17 / 0.06 | 92000 / 33776 | 0.08 / 0.05 |
| 128 numeric jobs, eight workers | 0.08 / 0.08 | 23588 / 23472 | 0.03 / 0.04 |
| 32 waits of 10 ms, eight workers | 0.03 / 0.04 | 23424 / 23420 | 0.06 / 0.07 |
| 20 alternating future/pool calls | 0.09 / 0.06 | 59936 / 28020 | 0.06 / 0.05 |
| Unchanged async control, 500 futures | 0.08 / 0.08 | 48400 / 48392 | 0.07 / 0.07 |

The repeated pool workload used about 65% less CPU and 63% less peak RSS. These
are workload-specific reductions from avoiding large module snapshots. The
single waiting pool cost more CPU and elapsed time; numeric elapsed p95 increased
0.04 to 0.07 s, waiting elapsed p95 0.06 to 0.09 s. Time readings have 0.01-second
granularity and seven-sample nearest-rank p95 is the maximum observed sample.
The previous async regressions, general runtime/FFI/GC, legacy CLI exit status,
platform parity and installed product integration remain separate open work.

## Thread future completion and shared launch boundary

`thread.go` now uses the minimal `async_worker` launch module, loaded on first
use. Workers no longer snapshot the entire `thread` module just to execute a
single task. `thread` and `async` share terminal envelope delivery and its
serialization-failure fallback; their public failure contracts remain distinct.

- `thread.await_timeout(...).ok` means **completed**, including a successful
  `nil` or a task failure represented as `nil`. It is not an authentication,
  application-success or task-success flag. Use `async.all_settled` when task
  success must be distinguished from a legitimate `nil`.
- `thread.is_ready` and `thread.try_await` harvest and cache a terminal result,
  join the owned worker and close its result channel. Repeated observations do
  not consume or join twice. `try_await` alone still cannot distinguish pending
  from completed `nil`; inspect `is_ready` or the timeout result.
- `thread.promise` stays eager. Its task errors still propagate immediately.
- A waiting deadline does not cancel work. The creating VM retains the future
  and eventually awaits it. Join after result receipt may include runtime
  teardown, so these calls provide no hard real-time latency guarantee.
- Channel registry entries are still retained by the host after close. General
  cycle reclamation, host aborts before the worker body, external resource
  cleanup and broader backend parity are not solved here. Future maps and their
  internal handles must not be shared across VMs or consumed independently.

Example: `examples/thread_future/main.sapp`. Regression coverage includes nil,
false, envelope-shaped values, copied captures, task exceptions, unsupported
function/cyclic results, mixed polling, cached observations and late completion.

On Linux, against the preceding source revision with the same retained runtime,
one warmup and seven alternating samples gave these process medians:

| Workload | CPU before / after (s) | Peak RSS before / after (KiB) | Elapsed before / after (s) |
| --- | --- | --- | --- |
| 100 sequential thread futures | 0.11 / 0.04 | 64708 / 26876 | 0.09 / 0.04 |
| 500 sequential thread futures | 0.50 / 0.07 | 245352 / 47404 | 0.37 / 0.07 |
| 32 concurrent thread futures waiting 30 ms | 0.05 / 0.04 | 41844 / 26156 | 0.06 / 0.06 |
| Pool-only control | 0.03 / 0.02 | 27012 / 27080 | 0.02 / 0.02 |
| Unchanged async future control, 500 tasks | 0.07 / 0.07 | 49400 / 47392 | 0.07 / 0.07 |

Small CPU readings have 0.01-second tool granularity. The waiting workload's
elapsed p95 increased 0.06 to 0.07 s; the unchanged async control's elapsed p95
increased 0.07 to 0.10 s. Seven-sample nearest-rank p95 is the sample maximum.
The control variation is not evidence that this change optimized async itself.
Earlier async default-pool wait regressions and sustained-future memory costs
are not established as eliminated. No automatic I/O classification, worker-limit
increase, GPU/NPU or Live View speedup is claimed.

The separate installed CLI was also found to deny `sleep_ms` in spawned workers
despite allowing it in the parent with the same invocation. The paired results
above use the current project executable, not that older installed executable.
No installed runtime or desktop application was replaced in this change.

The PBLang-owned implementation is `stdlib/async.sapp`. It runs on the retained
host's spawn/channel/join primitives; this is not a new event-loop runtime or a
claim of native backend parity.

## Futures and deadlines

```pblang
let A = import("stdlib:async.sapp");
let future = A.go(fn() { return 42; });
let status = A.await_timeout(future, 10);
// status.ready=false: waiting ended, worker can still run.
// status.ready=true: status.ok distinguishes success/failure, even for nil.
let final = A.all_settled([future]);
```

The creating VM owns the future. Do not send future maps into another worker,
consume their channels directly, or concurrently mutate them. Transfer of a
handle does not transfer or synchronize its ownership.

- `await` caches successful values and failures. Repeated calls do not consume
  another channel message or join an already joined worker.
- `all_settled` works with already awaited and repeated futures, and records
  task failures separately from legitimate `nil` results.
- `await_race` settles only its winner. The caller keeps all other futures and
  must eventually await/settle them. Empty input returns `nil`; a winning task
  failure throws. Already resolved futures are considered in input order.
- `await_timeout(future, ms)` requires a finite nonnegative integer duration.
  It does not terminate the worker or undo external effects. The returned map
  contains `ready`, `ok`, and `value`. On timeout, retain the future and settle
  it at an appropriate shutdown or completion point.
- `gather` owns its internally created futures and settles all of them before
  reporting failure, including a failure to start a later worker. Consequently
  it can wait for the slowest task even if another task has already failed.
- `pool` and an explicit `map_parallel` size require finite positive integers,
  including with empty input. Oversized valid pools still clamp to task count.

Worker count, queue sizes and total retained results still need application
budgets. A worker that never terminates cannot safely be killed by this library.
No hard real-time guarantee is made: joining after a completion message still
includes runtime teardown. Host-level failures before sending the completion
message are not equivalent to a user task throwing a catchable exception.
`channel_close` closes a channel but the retained host keeps its registry entry;
this change does not claim complete native handle reclamation.

### Timeout migration (alpha API correction)

`timeout(ms, fn)` now returns `{ready, ok, value, future}` for **every** result.
This is a deliberate breaking correction to the old value-or-nil contract,
which could block past its deadline and had no way to return an unfinished
worker's ownership. `ready:false` means the wait expired; `ready:true, ok:false`
means the task threw. A successful `nil` is `ready:true, ok:true, value:nil`.

```pblang
let pending = A.timeout(5, fn(){ sleep_ms(40); return 42; });
// Retain pending.future even when ready is false.
let final = A.all_settled([pending.future])[0];
```

The duration bounds the channel wait after launch; creating a worker and runtime
teardown add overhead. This is not a hard wall-clock execution limit. It does
not kill or undo the task. Discarding the returned future is not automatic
cleanup. A hidden global reaper is not used because module snapshots do not
establish unique cross-VM ownership.

Worker launch now lives in the small `async_worker.sapp` module. The async
library uses the existing buffered-channel builtin directly instead of
importing the whole thread library for a one-line wrapper. Parent authority is
still inherited through the retained spawn implementation. Supported closed
captures transfer as copies; caller globals and arbitrary OS handles are not
guaranteed to transfer.

Example: `examples/async_lifecycle/main.sapp`.
Regression coverage: `tests/semantics/async_lifecycle_test.sapp`.

### Future result-transfer failures

Single-future channels now have capacity one: exactly one terminal envelope is
expected. One catch boundary covers both task execution and serialization of
its result. If either fails, the worker attempts a plain failure envelope and
closes the channel. This prevents unsupported payloads (such as functions or
cyclic arrays) from leaving an owner waiting for an impossible result until the
host's 30-second receive timeout. Repeated awaits cache failure as before.

`await_timeout` distinguishes an empty closed channel from a live timed-out
channel and settles the former. When select returns no winner and a channel is
closed, `await_race` settles that failed future rather than reporting nil success.
The ordinary failure-envelope path also wakes races involving other live tasks.
If even the fallback send fails, a race containing both an empty failed channel
and an open channel can still wait inside the retained host's select; there is
no new polling loop or host-level failure notification mechanism. Runtime aborts
before the worker body and hard resource exhaustion are not covered generally.

The public failure result remains generic (`ok:false`, or the existing await
error), not a preserved original exception value. `nil`, `false` and maps shaped
like internal envelopes remain valid successful results. Channel registry entries
are still retained by the host after close. This is not general handle reclamation
or cycle collection. Example: `examples/async_transfer/main.sapp`; regression:
`tests/semantics/async_transfer_failure_test.sapp`.

#### Transfer measurements (2026-09-25)

An identical unsupported-function result probe waited approximately 30001 ms
before the change and less than 1 ms afterward, measured around await after
launch. This is one correctness probe, not an overall latency guarantee.
Normal-workload measurements used the same executable and sources, one warmup,
seven alternating samples, whole-process CPU and peak RSS:

| Workload | CPU median before / after (s) | RSS median before / after (KiB) | Elapsed median before / after (s) |
| --- | --- | --- | --- |
| 500 single-future launch/await pairs | 0.09 / 0.09 | 48840 / 47276 | 0.08 / 0.08 |
| 5000 single-future launch/await pairs | 1.00 / 1.00 | 278508 / 281640 | 0.82 / 0.83 |
| 32 waiting tasks, default eight workers | 0.08 / 0.10 | 23012 / 24588 | 0.20 / 0.23 |
| Same waiting tasks, explicit 32 workers | 0.07 / 0.07 | 25812 / 24556 | 0.10 / 0.10 |

Small-future median RSS decreased about 3.2%, but the sustained workload used
about 1.1% more. Not all worst times improved: small-future elapsed p95 rose
0.08 to 0.09 s and explicit-wait elapsed p95 rose 0.10 to 0.11 s. Seven-sample
p95 is the observed maximum. The default-wait workload regressed in this run;
its worker policy was not changed. Explicit concurrency reduces its waiting
time by spending more concurrent workers, not by detecting I/O automatically.
There is no general RAM saving, removal of all prior overhead, desktop Live View
benefit or wider-platform result established by these tests.

## Pool ownership and default concurrency

`async.map_parallel(items, mapper)` now reuses at most eight workers, matching
the existing conservative `thread.parallel_map` default. It still processes
every item and preserves result order. The third argument explicitly chooses
concurrency, including `1` for debugging and values greater than eight when the
application has an appropriate resource budget. Empty input starts no workers.
`pool(size, tasks)` retains its explicit size contract.

This changes default scheduling: I/O-heavy tasks can take longer with fewer
workers. Tasks must not rely on all items starting simultaneously; for example,
a barrier expecting more participants than active workers can deadlock. Nested
pools are separate pools, not a shared process-wide scheduler or a global
concurrency budget. Inputs, captures, queued indices and retained results still
consume memory proportional to workload size. No new item-count cap is imposed.

Pool launch/collection lives in `async_pool_worker.sapp`, loaded on first pool
use, separately from the small single-future launcher. This avoids copying the
whole async module into each pool worker or loading pool code for `go` alone.
The owner joins every successfully created worker, including after partial
launch failure, and closes its result sender before returning. Its result queue
can hold one message per input, so joining before draining cannot deadlock on
that queue. External blocking inside a task is not covered by that guarantee.

Launch failure, failed worker join and missing result messages now throw rather
than returning an apparently successful partial array. After a launch/join
failure, both abandoned queues are drained after joining, so their payloads do
not remain in the retained channel registry. Already running tasks
can complete their external effects before that error returns. They are not
retried or forcibly cancelled. User task exceptions still map to `nil` for
backward compatibility, indistinguishable from a successful `nil` in this API;
use `go`/`all_settled` when that distinction is required. The retained host's
channel-registry reclamation limitation remains unchanged.

Example: `examples/async_pool/main.sapp`. Regression:
`tests/semantics/async_pool_ownership_test.sapp`. Failure-injection checks use
isolated copies of the internal module; production code has no injection hook.

### Pool measurements (2026-09-25)

Retained Linux host, same sources and executable, one warmup followed by seven
alternating samples per variant. The CPU workload maps 64 items in 30 rounds;
the wait workload maps 32 independent 30 ms sleeps once. CPU is user + system
time; RSS is peak process memory. A separate observation saw peak process
thread counts fall from 34 to 10 (including host threads).

| Workload | CPU median before / after (s) | RSS median before / after (KiB) | Elapsed median before / after (s) |
| --- | --- | --- | --- |
| CPU map, default workers | 0.90 / 0.21 | 302368 / 38220 | 0.28 / 0.08 |
| Same CPU map, explicit four workers | 0.17 / 0.14 | 39988 / 30192 | 0.07 / 0.08 |
| Waiting tasks, default workers | 0.04 / 0.06 | 26936 / 23052 | 0.07 / 0.17 |
| 500 single-future launch/await calls | 0.14 / 0.13 | 46980 / 47600 | 0.13 / 0.12 |

Default CPU-map median CPU fell about 77% and RSS about 87% in this workload;
this combines fewer workers and smaller worker-module snapshots. At the same
explicit worker count, CPU and RSS also decreased, but elapsed time increased
slightly. Waiting tasks took longer with the new default. These are tradeoffs,
not a universal acceleration or general memory-management fix. Single-future
RSS increased about 1.3%, and its observed p95 CPU rose 0.14 to 0.15 s. The
explicit-four-worker elapsed p95 rose 0.11 to 0.18 s. With seven samples, p95 is the observed
maximum. No installed desktop/Live View or wider-platform benefit is established.

## Self-hosted checking

`frontend_analyze(source)` now checks names, call arity and operand types using
the existing self-hosted type checker after successful parsing. It preserves
`tokens`, `tokens_count`, `stmt_count` and string `errors`, and adds structured
`diagnostics` for type failures. It does not execute source or emit bytecode.
`frontend_parse_file` also delegates to this analyzer and therefore now includes
type checking. This is a deliberate tightening: syntactically valid but type-invalid source
now has `ok:false`.

Editors requiring parse-only recovery can use `frontend_parse_safe` or
`frontend_parse_full`. Type checking does not establish that arbitrary imports,
FFI, native backend semantics, or dynamically supplied values are safe.

The existing public `pblang check source.sapp` also performs type checks when
smart diagnostics have no existing errors. Its PBLang driver throws on failure,
including direct script invocation without a status-file environment variable.
It no longer writes a status file. Warnings remain nonfatal. This route retains
the existing smart diagnostic output, which can include an additional parse.

For a smaller single-file CI interface use the PBLang-owned checker:

```text
pblang run tools/pbcheck.sapp --allow-fs --allow-env -- source.sapp
```

It exits unsuccessfully for diagnostic failures, missing/empty files or invalid
arguments. A successful check prints `pbcheck: source check passed`.

**Retained CLI limitation:** `--check --via-pblang` now displays type failures
through the improved analyzer, but the existing Rust driver still returns exit
status zero for a diagnostic-only rejection. Do not use that exit status as a
CI acceptance gate. No Rust source or retained executable was changed here.
The default Rust checker also has independent gaps documented by the audit.

Regression coverage: `tests/semantics/selfhost_analyze_types_test.sapp`; verify
the checker process exit as well as its printed diagnostics when integrating it.

## Earlier measurements (before worker isolation)

On the retained Linux host, using identical inputs, one warmup and seven
alternating measured samples per variant:

| Workload | Before median | After median |
| --- | --- | --- |
| Race return latency, 400 ms loser | 400.84 ms | 1.13 ms |
| 120 sequential create/await calls, process CPU | 0.30 s | 0.30 s |
| Same worker workload, peak RSS | 120456 KiB | 122908 KiB |
| 20000 cached awaits, process CPU | 0.05 s | 0.05 s |
| 60 source analyses, process CPU | 0.86 s | 0.88 s |
| Same analysis workload, peak RSS | 36276 KiB | 36348 KiB |

The analysis now performs additional type checks. The worker workload uses
about 2% more peak RSS. These measurements demonstrate the race behavior fix,
not general CPU/RAM savings or real-time guarantees. With only seven measured
samples, the nearest-rank p95 equals the observed maximum and is not a robust
tail-latency estimate. No GPU/NPU, Windows, macOS or deployed PB-MCP performance
claim follows from this run.

## Worker isolation follow-up measurements

The follow-up compares the previous local implementation with the isolated
worker implementation, on the same retained Linux host. Inputs and permissions
were identical; one warmup and seven alternating measured samples per variant.

| Workload | Before CPU / peak RSS medians | After CPU / peak RSS medians |
| --- | --- | --- |
| 120 create/await calls | 0.31 s / 124664 KiB | 0.04 s / 27724 KiB |
| 20000 cached awaits | 0.07 s / 23600 KiB | 0.07 s / 23260 KiB |
| Four-worker map of 200 items | 0.06 s / 26436 KiB | 0.05 s / 23284 KiB |
| 60 source analyses | 0.89 s / 37132 KiB | 0.91 s / 37044 KiB |

Worker creation used about 87% less process CPU and 78% less peak RSS in this
workload. Source-analysis CPU remained about 2.2% higher in the final comparison;
its observed nearest-rank p95 was 0.92 -> 1.08 s. The earlier exploratory run
went in the opposite direction, so no reliable analysis speedup is claimed.
This does not establish universal memory savings, performance on other OSes,
or PB-MCP/live-view gains. The public smart `check` performs additional type
validation and its complete command cost was not benchmarked here.

The existing `examples/concurrent_tasks/main.sapp` now demonstrates the new
result contract and uses a closed-capture factory for pool jobs. Its Fibonacci
algorithm/workload is unchanged; recursive declaration moved to module scope
because the retained host did not resolve the nested recursive name.

## Source-only analysis

`stdlib:compiler/source_analysis.sapp` exports `analyze(source)`, a parity-tested
structured check equivalent to `frontend_analyze`. It imports only the tokenizer,
parser and type checker plus their dependencies. It does not import the
execution VM, optimizer, code generator or module loader. `tools/pbcheck.sapp`
uses this smaller dependency set; applications needing execution still use the
full frontend. Example: `examples/source_check/main.sapp`.

This is dependency separation, not lazy loading of arbitrary language modules,
general automatic memory reclamation or a new backend. Source input is still
materialized in memory; service callers must retain their source-size and
execution budgets. Returned token/diagnostic maps are owned by each call and
are not stored in a cross-request cache.

Lexical scanning checks whether the optional `unterminated` field exists
instead of probing it through an exception on ordinary tokens. Keyword lookup
also uses map membership. Malformed numbers and unterminated strings/comments
retain their diagnostic text and prevent bytecode generation.

The tokenizer now recognizes `nil`, `true` and `false` as completed values
before a minus sign. Previously `nil-1` could become two expressions (`nil`
and `-1`) while `nil - 1` was correctly parsed as subtraction. Both spellings
now produce subtraction, including intervening comments and radix/exponent
numbers. The checker rejects scalar nil arithmetic without requiring an
explicit variable type. Unary negative literals after `return`, `throw`, an
assignment or an opening delimiter retain their previous behavior.

This does not repair the retained Rust `--check --via-pblang` exit-code route;
use the public `check` command or `tools/pbcheck.sapp` for a process-status gate.
No installed PB-MCP desktop release is changed by importing this source module.

### Source-only checker measurements (2026-09-25)

Retained Linux runtime, identical inputs, one warmup and seven alternating
measured samples per variant. CPU is user + system time for the whole process;
RSS is the process peak, not heap live bytes.

| Workload | CPU median before / after | Peak RSS median before / after | CPU observed p95 before / after |
| --- | --- | --- | --- |
| `pbcheck`, one small valid source including startup | 0.83 / 0.12 s | 36848 / 24584 KiB | 0.85 / 0.12 s |
| Full frontend, 60 small-source analyses | 0.85 / 0.86 s | 36688 / 36664 KiB | 0.90 / 0.88 s |
| Full frontend, 60 analyses with a 400-element list | 2.49 / 2.46 s | 36688 / 36756 KiB | 2.62 / 2.95 s |

The standalone checker used about 86% less CPU and 33% less peak RSS in this
startup-dominated workload. This does not imply the same saving for a long-lived
analysis server. Full-frontend medians remained close (about +1.2% and -1.2%);
one worst time increased. No general RAM saving or elimination of the older
analysis overhead is demonstrated. With seven samples, nearest-rank p95 is the
observed maximum, not a reliable tail-latency bound.

An attempted refactor making the full frontend import the smaller facade was
rejected after it increased CPU. Both entry points therefore keep small
orchestration functions over the same tokenizer/parser/type-checker libraries.
`source_analysis_parity_test.sapp` compares their complete structured outputs;
changes to the analysis contract must update both facades and retain parity.
No results establish Windows/macOS, GPU/NPU, PB-MCP or live-view acceleration.

## Shared AST for smart and type diagnostics

`stdlib:diagnostics.sapp.diagnose_checked(source)` combines smart diagnostics
with gradual type checking of the same parsed AST. The public `pblang check`
command uses this API instead of parsing again through `frontend_analyze`.
Example: `examples/checked_diagnostics/main.sapp`.

The result retains `ok`, diagnostic arrays and counts, and adds `type_checked`.
Existing parser and unresolved-name errors prevent the type pass; warnings do
not. Type diagnostics retain their code, message, available span and line.
Empty/comment-only programs are rejected. All errors contribute to CLI counts
and the failure exit status. The source is not executed.

Legacy `diagnose`, `suggest` and `autofix` keep their lint-only behavior. The
checked API is stricter: kernel/extern lint pragmas do not provide verified
foreign type signatures. Imports and dynamic values remain subject to gradual
typing limits. This is not a proof of runtime safety. Service callers must
continue enforcing source-size, time and memory budgets.

PB-MCP's source diagnostics and editor adapters now consume this checked API in
source. This does not update an installed desktop release or imply Live View
acceleration. The retained `--check --via-pblang` process-status limitation is
still separate: use public `check` or `tools/pbcheck.sapp` for exit-status gates.

### Shared-AST measurements (2026-09-25)

Retained Linux runtime, same sources and executable, one warmup and seven
alternating samples per variant. Before: smart diagnostics followed by
`frontend_analyze`; after: `diagnose_checked`. Both assert a successful verdict.

| Workload | CPU median before / after | Peak RSS median before / after | CPU observed p95 before / after |
| --- | --- | --- | --- |
| 60 checks of a small typed function | 1.08 / 1.02 s | 37148 / 37140 KiB | 1.21 / 1.06 s |
| 60 checks containing a 400-element list | 4.36 / 2.74 s | 47072 / 37208 KiB | 4.78 / 2.79 s |
| Public `check`, small file including startup | 1.01 / 0.98 s | 37248 / 37156 KiB | 1.36 / 1.05 s |

These workloads used approximately 6%, 37% and 3% less median CPU respectively.
The list workload used about 21% less peak RSS; the other medians were almost
unchanged. Seven-sample nearest-rank p95 is the observed maximum, not a tail
latency guarantee. This does not remove the earlier standalone frontend cost,
GC pauses or general runtime/FFI ownership limitations. At that checkpoint the
API still imported the full frontend; `compiler/source_analysis.sapp` was the smaller choice
for callers needing type analysis without smart lints. MCP previously ran only
smart lints, so these comparisons do not measure its added type-checking cost.

## Diagnostics without execution-frontend dependencies

The source-analysis facade now also exports `parse_safe(source)` and
`typecheck(ast)`. Diagnostics use these instead of importing the full frontend,
which would load the execution VM, code generation and other unused modules.
Parsing, smart lints and gradual type checks still run for every call. No
cross-request cache or diagnostic suppression was added.

The new parse/type results preserve the complete frontend structures, including
error messages and their historical `frontend_*` prefixes. Parity tests cover
malformed input, non-string inputs, Unicode, closures and error metadata. The
low-level `typecheck` entry expects the parser's AST schema; it is not a general
validator for arbitrary supplied objects. Parser exception conversion does not
provide a hard CPU or heap budget. The full execution frontend remains unchanged.

The checked-diagnostics regression also runs with only eight source modules
present: diagnostics, source analysis, tokenizer, parser, type checker, semantic
analysis, builtins and suggestions. No VM, codegen or full frontend module is
available in that fixture. `examples/source_check/main.sapp` demonstrates AST
reuse through the smaller facade.

### Dependency reduction measurements (2026-09-25)

Same workloads and runtime, one warmup and seven alternating measured samples.
Both variants perform the same checks, unless the row explicitly says lint-only.
CPU is whole-process user + system time; RSS is peak process memory.

| Workload | CPU median before / after | Peak RSS median before / after | CPU observed p95 before / after |
| --- | --- | --- | --- |
| 60 small combined checks | 0.72 / 0.23 s | 37212 / 27076 KiB | 1.05 / 0.33 s |
| 60 checks of a 400-element list | 2.70 / 2.02 s | 37116 / 27072 KiB | 2.75 / 2.57 s |
| Public `check`, small file with startup | 0.95 / 0.27 s | 37212 / 26976 KiB | 0.98 / 0.27 s |
| 30 direct PB-MCP checks, 400-element list | 2.50 / 2.23 s | 44860 / 41876 KiB | 2.99 / 2.34 s |
| Earlier lint-only PB-MCP versus new checked tool | 2.44 / 2.24 s | 44808 / 41828 KiB | 3.01 / 2.33 s |
| Standalone `pbcheck`, small source with startup | 0.08 / 0.07 s | 24548 / 24720 KiB | 0.08 / 0.08 s |

The checked PB-MCP workload used about 11% less CPU and 7% less peak RSS than
the previous checked version. Against the earlier lint-only tool, median CPU
was about 8% lower while preserving the new type checks. This covers the prior
6.7% added cost in this particular workload; it is not a guarantee for every
program or long-lived service. Standalone `pbcheck` times are near the timer's
resolution and its peak RSS increased slightly (about 0.7%). Absolute timings
vary between runs; compare paired variants, not isolated historical numbers.
With seven samples p95 is the observed maximum, not a tail-latency guarantee.

The installed desktop release was inspected: its inventory does not contain
diagnostics, the IDE tool module or either analysis frontend. Updating that
release would not deliver this change, so it was not replaced. No Live View,
GPU/NPU, Windows/macOS or general GC improvement is established. The retained
legacy `--check --via-pblang` exit-status issue is unchanged.

## Parser buffer lifetime and compact checking

`compiler/parser.sapp.parse(tokens)` now automatically drops its module-owned
token and error references before returning an AST, including reported grammar
errors. A new parse first discards any abandoned state from a previous call.
Returned ASTs/errors and caller-owned token arrays remain intact: cleanup
replaces the parser's references rather than clearing shared arrays in place.

The safe source-analysis and frontend parse facades also discard state after
caught parser exceptions and empty-input rejection. Their lexical-invalid
early returns release abandoned parser state too. Existing error structures,
messages and success criteria are unchanged.

`source_analysis.analyze(source, include_tokens = true)` keeps its complete
historical output by default. Passing the boolean `false` omits only `tokens`
and drops the analysis-local token reference before type checking. Counts and
diagnostics remain available. `tools/pbcheck.sapp` selects this automatically;
no CLI flag or manual cleanup is needed. Non-boolean options are rejected.
See `examples/source_check/main.sapp` and `examples/parser_lifecycle/main.sapp`.

For debugging, `parser.retained_state()` returns only token/error counts, not
source content. `parser.discard_state()` is an idempotent explicit reset after
a caught direct-parser exception. The parser still uses module-global state:
it is not reentrant and must not be reset during an active parse. Direct
uncaught parser exceptions preserve their original behavior and can retain
state until explicit reset, a later parse, or a safe facade call. This is not
general exception unwinding, cycle collection or external-resource/FFI cleanup.
Releasing references also does not guarantee memory is returned to the OS.

### Parser lifetime measurements (2026-09-25)

Same retained Linux executable and inputs, one warmup and alternating samples:
11 per variant for the small CLI, seven for each other workload. CPU is process
user + system time, RSS is peak process memory. Nearest-rank p95 is the observed
maximum with these sample counts and is not a latency guarantee.

| Workload | CPU median before / after | Peak RSS median before / after | CPU observed p95 before / after |
| --- | --- | --- | --- |
| Standalone `pbcheck`, small source with startup | 0.08 / 0.08 s | 24632 / 24616 KiB | 0.09 / 0.09 s |
| Standalone `pbcheck`, 4000-element list | 0.40 / 0.40 s | 24624 / 24496 KiB | 0.45 / 0.49 s |
| 20 parses of an 800-element list | 1.02 / 1.01 s | 24436 / 24416 KiB | 1.05 / 1.44 s |
| 30 direct PB-MCP checks, 400-element list | 2.29 / 2.25 s | 41804 / 41776 KiB | 2.83 / 2.66 s |

The earlier small-CLI RSS increase did not repeat in this paired run, but the
medians are essentially unchanged. Some observed worst times increased. These
results do not establish a general speedup, RAM reduction or elimination of
earlier regressions. The primary result is that completed parses no longer
retain their internal token/error buffers. Measurements cover direct MCP
source checks, not its installed desktop transport or Live View. Legacy CLI
status handling, wider platforms and general runtime ownership remain separate.

## Pool initialization ownership (2026-09-26)

`async.pool` and the capture-based `async.map_parallel` backend now close the
work channel if allocation of the result channel fails. If preloading job
indices fails, both channels are closed and already queued indices are drained
before throwing. No worker has started on these paths; there is no task replay.
New stage messages are `async.pool: result channel unavailable` and
`async.pool: work queue initialization failed`. The retained catch mechanism
cannot preserve the original host exception text. Initial work-channel allocation
still propagates its own error because no channel is owned yet.

The temporary fault-injection test rewrites a copy of the actual implementation,
uses real channels, and verifies first/second allocation failure plus first/third
index-send failure. Each created channel must be closed and empty. The regression
fails against the previous implementation and passes after the change. Temporary
source is removed. Existing task exceptions still yield nil; worker/result-transfer
failures still join siblings before throwing. See
`tests/semantics/async_pool_initialization_test.sapp` and
`examples/async_pool_recovery/main.sapp`.

Fifteen async/thread/channel suites, the example, lint, fixture build and portable
lock validation passed. This closes specific initialization paths, not arbitrary
OOM unwinding, retained registry reclamation, external-resource/FFI cleanup or GC.

Paired Linux measurements, identical retained executable and input, two warmups
and nine alternating measured samples (startup included; RSS in KiB):

| Workload | CPU median before/after s | Peak RSS median before/after | Wall median before/after s | Wall p95 before/after s |
| --- | --- | --- | --- | --- |
| 40 four-item maps | 0.07713 / 0.07679 | 32844 / 33656 | 0.06215 / 0.06254 | 0.07100 / 0.06805 |
| 32 waits of 10 ms | 0.05509 / 0.05395 | 23128 / 23108 | 0.09266 / 0.09201 | 0.09922 / 0.10009 |
| Ten 128-item numeric maps | 0.07471 / 0.07335 | 27736 / 27996 | 0.06117 / 0.05785 | 0.06439 / 0.06603 |

CPU/wait medians were close; small-map RSS increased about 2.5%, numeric RSS about
0.9%, and some p95 times worsened. Nine-sample nearest-rank p95 is the maximum
sample. An extra launch-module experiment reduced some RSS but added about
20–23% CPU in its paired run and was rejected. No general speedup or RAM-saving
claim follows from this correctness fix. The old retained CLI exit-status bug,
installed PB-MCP/Live View integration and broader runtime/platform work remain
separate.

## Aggregate future ownership (2026-09-26)

`async.await_all` now settles every supplied valid future before returning ordered
values or throwing `async.await: task threw`. Previously a failed task aborted
that loop before later siblings were joined or their channels closed. Task-body
and result-transfer failures now leave all supplied futures in their cached
terminal state. Successful sibling values remain available through `await`.
Empty inputs, duplicate futures, nil/false and marker-shaped results are retained.
`gather` delegates this settlement to `await_all`, removing its redundant wait pass.

Compatibility: failure can now return later, after the slowest sibling finishes.
This is deliberate ownership cleanup, not cancellation, retry or rollback.
Use `await_timeout`/`await_race` when early return is needed and retain ownership
of every unfinished future. A nonterminating task can block `await_all` forever.
Only valid futures owned by this VM are supported; malformed handles, concurrent
mutation, host aborts and arbitrary allocation failures are outside this fix.
The separate `async_await.sapp` API and native backends are unchanged.

Regression coverage: `tests/semantics/async_await_all_ownership_test.sapp`.
Example: `examples/async_await_all_cleanup/main.sapp`.

Identical Linux retained executables, two warmups and nine alternating measured
samples per version, including startup; RSS is peak process memory:

| Workload | CPU median before/after s | RSS median before/after KiB | Wall median before/after s | Wall p95 before/after s |
| --- | --- | --- | --- | --- |
| 40 gathers of four tasks | 0.12442 / 0.12227 | 29836 / 30896 | 0.09536 / 0.09481 | 0.10361 / 0.09866 |
| Batch of 128 futures | 0.13055 / 0.12843 | 29080 / 29700 | 0.10723 / 0.10456 | 0.18519 / 0.19678 |
| 32 futures waiting 10 ms | 0.08723 / 0.08632 | 25828 / 25644 | 0.09065 / 0.09274 | 0.13766 / 0.11567 |

CPU differences are small (about 1–2%); RSS increased about 3.6% for gathers and
2.1% for the batch. Batch wall p95 and wait median worsened. Nine-sample p95 is
the observed maximum, not a latency guarantee. This is a cleanup correctness
fix, not general optimization. The earlier pool RAM and compiler-tail regressions
remain unresolved. An inline pool-initialization experiment saved only about
0.5% RSS with mixed tails and was rejected. No installed PB-MCP or Live View
performance benefit, general runtime/FFI/GC completion or platform parity follows.

## Complete staged thread pipelines (2026-09-27)

`thread.pipeline(stages)` now builds a batch pipeline with parallel workers
inside each stage and a completion barrier between stages. Previously it used
nonblocking `drain` on the final queue and could return an empty/partial result
while workers and detached closer tasks were still active. The implementation
is PBLang and loads `thread_pipeline.sapp` on first construction; ordinary thread
users do not load that module. Its worker pool loads only for nonempty work.

Use `thread.pipeline_run(pipeline, input_array)` for explicit receiver dispatch:

```pblang
let T = import("stdlib:thread.sapp");
let p = T.pipeline([map{workers:2, process:fn(x){return x*2;}}]);
let values = T.pipeline_run(p, [1,2,3]);
```

**Retained VM limitation:** in a reproduced nested-loop failure case,
`p:run(data)` inside `try/catch` did not expose the expected sibling completion
signal, while `p.run(p, data)` and `T.pipeline_run(p, data)` passed. Prefer the
explicit public function, especially when catching failures in a local scope.
The colon-method form remains for compatibility; this change does not repair
general VM method-call/exception semantics. The exact VM root cause is unresolved.
The synchronous `pipeline_source/stage/collect` API is unchanged.

Stage configuration is copied and validated before any task starts. Every stage
requires a callable `process`; `workers` defaults to one and must be a finite
positive integer, clamped to the current batch length. Invalid later stages
are rejected during construction. Input must be an array; nil input items are
ordinary data. A nil *result* filters an item; false and marker-shaped maps remain
values. Empty batches start no workers; zero stages return the input unchanged.
Use one worker per stage when deterministic input order is required; multiworker
results follow completion order.

The existing owned pool joins every started worker and closes/drains queues on
ordinary task, input/result transfer and partial launch failures. A failed stage
prevents later stages from starting. Tasks are not retried; completed external
effects are not rolled back, and unstarted jobs may be abandoned after failure.
No detached closer task is needed. Nonterminating workers, runtime aborts and
arbitrary OOM remain outside catchable cleanup. Input/result bytes require caller
budgets; queue slots are bounded by batch length. Closed host registry entries
are not reclaimed by this library.

Compatibility correction: stages no longer stream concurrently into each other.
The API returns a complete batch or throws instead of returning partial work.
A request depending on overlapping stages needs a separately owned streaming API.
Validation now rejects zero/negative/nonnumeric workers, even for empty batches.
See `examples/thread_pipeline_cleanup/main.sapp` and
`tests/semantics/thread_pipeline_ownership_test.sapp`.

Paired Linux control measurements against `de0bbd9`, identical executables,
two warmups and nine alternating samples, startup included:

| Existing workload | CPU median before/after s | Peak RSS before/after KiB | Wall median before/after s | Wall p95 before/after s |
| --- | --- | --- | --- | --- |
| 40 four-item thread pools | 0.19451 / 0.19198 | 31416 / 32464 | 0.16937 / 0.16223 | 0.31321 / 0.27827 |
| 100 thread futures | 0.15707 / 0.16137 | 26524 / 26560 | 0.15960 / 0.16641 | 0.50024 / 0.23106 |
| Unchanged async control | 0.13948 / 0.13455 | 26512 / 25276 | 0.12846 / 0.12481 | 0.15725 / 0.13329 |

The pool used about 3.3% more peak RSS; thread futures used about 2.7% more CPU.
The unchanged async control also moved, so small differences are not causal
proof. Nine-sample nearest-rank p95 is the observed maximum. There is no general
RAM reduction, elimination of old async regressions or streaming speedup claim.
The previous pipeline returned incomplete work, so its time is not a valid
performance baseline for this completed-batch implementation.

## Single evaluation of computed method receivers (2026-09-27)

The PBLang-owned parser now binds a computed receiver once before colon dispatch.
Previously `make_object():read()` could call `make_object()` three times for a
plain method map (twice for a class instance), selecting the method from one
object but passing another as `self`. Calls, indexes, members and chained
receivers now use a synthetic value-block local. Its name cannot collide with a
source identifier, and existing scope/exception cleanup releases the temporary
reference. A result referring to the object remains valid.

Plain identifiers retain their existing dispatch AST, avoiding an unnecessary
scope/frame on the common `obj:read()` path. Class-table selection, left-to-right
argument evaluation, false/nil results and exception propagation are preserved.
Blocks containing user closures still use the existing closure-frame fallback;
this change is not a new collector or a general lifetime guarantee.

See `examples/method_receiver_once/main.sapp` and
`tests/semantics/selfhost_method_receiver_test.sapp`. The regression exercises
normal/bounded VM execution, safe optimization and bytecode round trips, plus
nested receivers, captured arguments, successful/failed temporary cleanup and
malformed syntax.

This is a self-hosted frontend correction. It does **not** change retained Rust
colon lowering or resolve the separate nested `pipeline:run(...)` failure above.
Use `thread.pipeline_run(pipeline, input)` on that host. The prior pool/future
performance regressions, general runtime/FFI/GC work and desktop integration are
still open. No Live View improvement follows from this parser change alone.

A minimal retained-host reproducer uses a loop-local method map: direct
`try local:run() catch 9` and `try {local.run(local)} catch 9` produce `7`,
whereas `try {local:run()} catch 9` produces `9`. The retained parser/compiler
introduces two nested functions, and its enclosing compiler snapshot stops at
one parent (`enclosing: None`). The self-hosted regression checks all three
forms produce `7`. The retained source is reference-only in this work; no Rust
source was changed and its failing path must not be advertised as repaired.

## Owned scope and aggregate-join cleanup (2026-09-27)

`thread.scope(body)` now joins its launched workers even if `body(scope)` raises
or a later launch fails. The scope closes to new launches before it joins, and
releases its handle array afterwards, including when the caller saved its launch
closure. Both `sc:launch(worker)` and explicit `thread.scope_launch(sc, worker)`
are supported. Prefer the explicit function in nested retained-host `try` blocks
because of the separately documented colon capture defect.

`thread.thread_join_all(handles)` now also attempts every handle after a native
aggregate-join failure. An invalid/already consumed handle no longer skips all
later handles. Successful calls keep native launch-order results, including
nil, false, and per-worker `{ok:false,error:...}` failure maps. Error-shaped user
values remain ordinary data. Existing native builtins are unchanged.

The normal path still uses one native `join_all`. Only its failure path retries
handles independently, ignoring already consumed ones while settling the rest.
When the scope body fails, cleanup discards each worker result rather than
retaining an aggregate array that cannot be returned. Small private helpers live
in `thread.sapp`; unused scope/join helpers start no workers and add no imports.
An experimental cached owner module added worker-copy/import overhead and was
rejected; importing it again per call was substantially slower still.

### Failure contract and limits

- The retained host cannot bind/rethrow a caught original exception. Body errors
  now become `thread.scope: body failed` **after cleanup**; body failure takes
  precedence over a concurrent join failure. The body return value is ignored,
  as before. Native per-worker error maps retain their existing behavior.
- Aggregate infrastructure failures raise `thread.scope: join failed` or
  `thread.join_all: join failed` after remaining handles are attempted. These
  stable messages replace the native aggregate error text on these paths.
- An escaped launch closure raises `thread.scope: scope is closed` after its
  body's lifetime ends. Scope objects belong to their creating VM; this is not
  a cross-VM capability or a sandbox boundary.
- Joining is not cancellation, rollback, a deadline or process termination.
  A worker that never finishes can still prevent scope completion. Native
  crashes, process exit, resource exhaustion during spawn/handle registration
  and arbitrary external resources still need runtime-level support.

See `examples/thread_scope_cleanup/main.sapp` and
`tests/semantics/thread_scope_ownership_test.sapp`. This change does not fix the
retained colon compiler, generic FFI/GC, prior async timing regressions or the
installed desktop PB-MCP.

## Race deadlines and scope success allocations

`async.await_race` waits until a supplied future is terminal. The retained
host's `select` has an internal ten-second wait limit; reaching that limit
without a terminal future now retries selection instead of inventing a `nil`
result. An empty race still returns `nil`. A successful task returning `nil`
remains distinguishable internally from an unfinished task.

`async.await_race_timeout(futures, ms)` provides explicit bounded selection:

- Returns `{ready, ok, value, index}`. `index` is zero-based in the supplied
  array. Pending and empty races return `ready:false`, `ok:false`, `value:nil`,
  `index:nil`; empty races return immediately.
- Task and transfer failures return `ready:true, ok:false`. Successful `nil`,
  `false`, and status-shaped maps remain ordinary values. Cached terminal
  futures return immediately, using the first cached entry in array order.
- `ms` must be a finite whole number from zero through 2147483647 inclusive.
  Zero polls without waiting for an unfinished result. Invalid durations are
  rejected even with an empty array or cached result.
- Selection consumes and settles only the winner; every other future remains
  caller-owned. Neither timeout nor race cancels work. Eventually use `await`
  or `all_settled` for every future.
- The selection duration is not a hard return-time guarantee: winner joining,
  VM teardown, result conversion and scheduling can add latency. These maps
  belong to the creating VM; concurrent mutation or consuming their internal
  channels is unsupported. Selection order is not a chronological ordering
  of all simultaneous completions.

Example: `examples/async_race_deadline/main.sapp`. Focused tests:
`tests/semantics/async_race_deadline_test.sapp` and
`tests/semantics/async_race_host_timeout_test.sapp`. The latter deliberately
waits 10.2 seconds to cover the real retained-host limit, then reaps the future
even when run against the broken preceding source.

Scope/aggregate joins no longer allocate a success-status map around the
native result array. Scope handles transfer to the active stack frame before
joining; the escaped launch closure is already closed and no longer retains
them on success or failure. The exceptional path still independently joins
remaining handles after an invalid/consumed handle. Worker error maps, ordered
values and stable cleanup error strings remain unchanged. No catch-bound
original exception value has been added to the retained host.

Closing a channel still does not remove its retained-host registry entry.
This change is not general handle reclamation, cycle collection, cancellation,
or runtime/FFI cleanup. It does not repair the retained nested `p:run` capture
issue or establish other-platform/installed-product support.

## Exact partitions and partial launch cleanup in thread_pool

The separate `stdlib:thread_pool.sapp` helper module now rounds chunk counts
up to integers. Previously `(n + workers - 1) / workers` used floating-point
division: mapping `[0,1,2,3,4,5]` with four workers returned
`[0,1,2,2,3,4,4,5]`. `parallel_map` now visits each element once and preserves
input order. `auto_size` returns `ceil(pending / 2)` clamped to 1..64, so an
odd pending count cannot produce an invalid fractional worker count.

`parallel_for` partitions iteration counts, not fractional array offsets.
Start, end and step must be finite integers in the exactly represented range
−9007199254740991..9007199254740991; positive spans must also fit that range.
End is exclusive. For compatibility, nil or nonpositive integer step uses 1.
Fractional/non-finite ranges now fail before launching work; use a separate
data array for fractional coordinates. Empty calls validate configuration too.

If launching a later worker fails, `parallel_map` and `parallel_for` settle
all previously launched futures before throwing `thread_pool: worker could
not start`. This is cleanup, not cancellation or rollback of external effects.
A worker that never finishes can still delay cleanup indefinitely.

`run_parallel` joins the successfully started chunks and records their task
outcomes. Tasks in unstarted chunks become failed with `worker could not
start`; they do not remain pending for implicit replay on the next run.
Compatibility change: catchable launch failures are returned in the normal
completed/failed summary rather than escaping with partially pending state.
Ordinary per-task failures retain their existing result/status behavior.
The standalone helpers' legacy handling of worker-body/transfer failures is
unchanged; this is not a new universal exception-preservation mechanism.

Regression tests: `thread_pool_partition_test.sapp` and
`thread_pool_launch_cleanup_test.sapp` under `tests/semantics`. The latter
injects first/second launch failure into a temporary copy of the real module,
uses real workers and joins, and removes its fixture. The example is
`examples/thread_pool_partition/main.sapp`. No foreign implementation code
was added. Retained channel registry entries and general runtime/FFI/GC
cleanup remain separate work.

## Owned first-success races and dependency reuse

`thread_pool.race_owned(tasks, timeout_ms=nil)` starts at most 64 zero-argument
callables and returns `{ready, value, index, status, handles}`. This helper races
**successful** outcomes; task/transfer failures do not win. A successful nil or
false is a ready value. `index` is the zero-based first observed success, not a
strict chronology of simultaneous completions. Status is `ready`, `timeout`,
`failed` (all terminal without a success), or `empty`.

All task kinds and the finite whole duration (nil or 0..2147483647 ms) are
validated before starting any worker. Zero performs one nonblocking selection
pass; worker creation happens before the selection period. A partial launch or
catchable selection failure settles every started worker before throwing a
stable library error. This does not preserve an original host exception value.

**Ownership:** every handle remains with the returned result. Eventually call
`thread_pool.settle_race(result)` in the creating VM, including after timeout;
it joins all workers and returns the same result. Repeating settlement is safe
for unmodified results. Do not transfer these future maps across VMs, concurrently
mutate them, or consume their channels directly. Cleanup waits for effects; it
does not cancel workers, roll back work, or stop a nonterminating task.

**Alpha compatibility correction:** the value-only `thread_pool.race` still
returns the first successful value (or nil), but now settles all workers before
returning. Consequently loser cleanup may exceed the selection timeout. Previously
it lost the unreturned worker handles. Call `race_owned` for early return and
explicit ownership. Races over 64 tasks, fractional/non-finite durations and
malformed task lists now fail before launch. Empty input still returns immediately.
The timer uses the retained wall clock, so clock adjustments can change selection
duration; scheduling and VM teardown also prevent a hard return-time guarantee.

Each selection pass polls all unfinished futures with zero wait, then sleeps
once if necessary. It no longer waits one millisecond separately for each
candidate. The minimal future backend described below is lazily loaded once per
owning module/VM, shared by map, range, pool and race helpers. A failed import is not
cached. Empty map/range/pool operations still avoid loading the dependency.
A new execution is required to pick up dependency changes; this is not live
module reload or a new runtime-wide cache. Serial `run_all` does not load it.

Tests: `thread_pool_race_ownership_test.sapp` covers success, timeout, malformed
later task and first/second launch failure with observed real futures, reaping
workers even against the broken baseline. `thread_pool_race_owned_test.sapp`
covers ownership transfer, nil/false, failure, empty input and input limits.
Example: `examples/thread_pool_race_owned/main.sapp`.

## Minimal pool future dependency

`thread_pool` now loads the internal `thread_pool_future.sapp` subset instead
of importing the whole `thread.sapp` API. It still uses `async_worker.sapp` for
launching, copied captures and terminal result/transfer envelopes. Serial or empty
pool operations do not load this backend; repeated parallel calls reuse it in
the same owning VM. This reduces dependency work without changing concurrency,
result order, failure-as-nil compatibility, or partial-launch cleanup.

The subset accepts only the valid, unmodified go-futures created by the pool or
`thread.go`; it is not a replacement public API for eager promises. It preserves
`__go`, `__ch`, `__handle`, `__value` and `__resolved`. Futures returned by
`race_owned` therefore remain compatible with `thread.await`, readiness checks
and repeated settlement. A timeout leaves the live future owned by the caller.
The success message is recovered after joining if the retained receive wait
limit was reached. A task/transfer/join failure retains the existing nil result
contract; `ok` in the timeout result means terminal, not task success.

`tests/semantics/thread_pool_future_test.sapp` checks both directions of
interoperability, nil/false/status-shaped data, task and transfer errors, result
queue draining, channel closure, exactly-once joining, timeout ownership and
late-envelope recovery. The existing pool/race launch-failure tests run against
the subset too. The partition example exercises repeated reuse.

This does not replace the retained host, reclaim its closed-channel registry,
preserve original caught exception objects, cancel workers, or enable general
GC/FFI ownership. The full `thread` API and the installed desktop PB-MCP are
unchanged. Startup/RAM improvements are workload-dependent, not guaranteed.
