# Async lifecycle and self-hosted source checking

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
