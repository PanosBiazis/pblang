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
