# Avoiding obsolete state updates

`stdlib:latest_updates.sapp` is a PBLang-only cooperative buffer for updates whose
contract explicitly allows intermediate states to be replaced. It keeps the
latest pending payload per string key, then calls a consumer once per key at a
flush boundary chosen by the application. It contains no platform calls, timers,
background threads, polling, disk writes or hardware-specific code.

```pblang
let L=import("stdlib:latest_updates.sapp");
let updates=L.new_buffer();
updates.submit("preview.width",640);
updates.submit("preview.width",800);
let result=updates.flush(fn(key,value){print key+"="+str(value);return true;});
// Only preview.width=800 is consumed. result.coalesced is 1.
```

`submit_lazy(key, producer)` additionally postpones a zero-argument producer until
flush. Replaced producers are never called. This can avoid preparing an obsolete
preview or other replaceable computation, rather than only skipping its final
delivery. Producers must be pure or have explicitly replaceable effects.

## API and behavior

- `new_buffer()` creates an independent buffer.
- `submit(key,value)` and `submit_lazy(key,producer)` return true for a newly
  queued key, false for replacement. Replacements preserve first-key order.
- `pending()` returns the number of queued keys, not the number of submissions.
- `discard()` drops pending payload/producer references without invoking them.
  Call it when abandoning the buffer. It is rejected during a flush.
- `flush(consumer)` detaches one finite batch. The consumer receives `(key,value)`
  and must return boolean true to acknowledge application. New submissions from
  callbacks, including the same key, belong to the next flush. Recursive flush
  is rejected before changing state. An empty flush does no producer/consumer
  work; it still creates a small result report.

The flush report contains `ok`, `attempted`, `applied`, `coalesced` and `failures`.
Failures contain only the caller's key and a stage: `producer_exception`,
`consumer_exception` or `not_acknowledged`. Payloads are not copied into reports.
No reports are persisted. Keys should identify work, not contain secrets.

A failed producer/consumer does not stop unrelated updates. Failed entries are
consumed, **not automatically retried**, because a consumer may have performed a
partial external effect before failing. The application must inspect the report
and decide how to recover. `applied` means acknowledged, not independently proven
device delivery. Catch expressions currently cannot retrieve original host error
messages; the stage labels do not purport to be those messages.

Payloads are retained by reference, not deep copied. Mutating a submitted object
before flush changes what the consumer observes. Lazy closures likewise retain
their normal captures. There is no recursive value comparison or implicit
traversal of cycles. Work between flushes consumes memory proportional to pending
unique keys/payloads; flushing/discarding releases the buffer's references. It
does not close external handles or implement a cycle collector.

## Application and device coverage

The common algorithm is independent of application type and can be reused by
current and future runtimes supporting the required PBLang primitives. It does
not itself add a missing runtime, backend, driver or application framework.

| Area | Potential replaceable work | Work that must retain its own semantics |
|---|---|---|
| CLI, GUI, web, mobile | Progress/status display, next preview state | Commands, transactions, user actions |
| CPU and AI preprocessing | Preparing only the latest requested preview | Required pipeline stages and training steps |
| GPU, NPU, AI accelerators | Latest display/inference request before dispatch | Submitted jobs, dependencies, synchronization |
| HDD/SSD and network | An application-defined replaceable snapshot | Logs, append writes, commits, durable records |
| Internal/external peripherals | Latest supported indicator/display state | Motion, safety controls, every-event protocols |

These are integration opportunities, not claims of implemented device adapters.
The mechanism neither cancels already-submitted hardware work nor changes OS
power management. The host application chooses the flush boundary and guarantees
that replacement is semantically allowed. No inference is made from a variable
name, device type or elapsed time. Ordinary PBLang code remains unchanged.

Callbacks are trusted and synchronous, not sandboxed. No hidden maximum project
size, timer, retry or workload limit is introduced. Consequently, an unbounded
producer, process termination, memory exhaustion or exhausted host budget can
interrupt a flush. This is not an emergency-stop or exactly-once external-effects
protocol. Realtime/embedded deadlines and async cancellation need their own
validated integration.

## Evidence and benchmarks

The contract tests run directly, through the self-hosted module loader and after
compiling the complete library/test source to native bytecode. They cover latest
values, first-key order, nil/false, invalid keys/acknowledgements, failure isolation,
discard, reentrancy, same-key submissions during flush, independent buffers and
eager/lazy replacement. In a burst of 1,000 submissions over 16 keys, exactly 16
producers and consumers run; 984 obsolete computations are skipped.

Tests: [contract](../tests/semantics/latest_updates_test.sapp) and
[self-hosted](../tests/semantics/latest_updates_selfhost_test.sapp).
Example: [latest updates](../examples/latest_updates.sapp).

Measurements on 2026-09-20 use the same retained executable and 10,000 submissions
per process. Direct and buffered variants produce byte-identical arrays of the
final state, checked by SHA-256 on every run. Each variant receives two warmups
and seven alternating measured runs, without concurrent test/analyzer work.
GNU time records process CPU, elapsed time and peak RSS. P95 is the maximum of
seven observations; CPU resolution is 0.01 seconds, so a measured zero is below
resolution, not zero computation.

| Workload | Variant | CPU median / P95, s | Wall median / P95, s | Peak RSS median / P95, KiB |
|---|---|---:|---:|---:|
| 10,000 unique keys, cheap producer | direct | 0.01 / 0.01 | 0.01 / 0.01 | 12068 / 12108 |
| Same | buffered | 0.06 / 0.07 | 0.07 / 0.07 | 25744 / 25816 |
| 64 repeated keys, cheap producer | direct | <0.01 / 0.01 | 0.01 / 0.01 | 10476 / 10664 |
| Same | buffered | 0.01 / 0.01 | 0.02 / 0.02 | 10576 / 10636 |
| 64 repeated keys, 64 arithmetic iterations | direct | 0.12 / 0.12 | 0.12 / 0.12 | 10496 / 10628 |
| Same | buffered | 0.01 / 0.02 | 0.02 / 0.02 | 10512 / 10632 |

The repeated computation case reduces median process CPU about 92%; cheap
and unique work instead pays buffering overhead. No general RAM savings or
hardware power/IO-wear savings are established. PB-MCP itself has not yet been
integrated or benchmarked with this library. No GPU/NPU or physical peripheral
measurement is claimed, and execution on every platform is not verified.

Reproduce with `calculate(v)` starting at `total=v` and performing either zero
or 64 iterations of `total=total+num_sqrt(v+j+1)`. For submission `i`, use key
`str(num_mod(i,key_count))`. The direct variant calculates and assigns immediately;
the buffered variant queues a one-level closure factory `producer(i)` and flushes
once. Use key counts 10,000 and 64. Print final values in numeric key order and
compare complete outputs. Include all calculation, registration, reporting and
output costs; exclude build/lock publication. Measure with
`/usr/bin/time -f '%U %S %e %M'`.

## Polyglot and non-programming formats

The queue can sit before the existing polyglot boundary. Submit a request value
containing language, source/input and the caller's execution options. At flush,
the consumer delegates the latest request to the existing facade, or use a lazy
producer wrapping that call. This skips obsolete foreign-runtime launches before
they happen. It does not analyze or optimize the interior of arbitrary foreign
code, intercept every existing inline polyglot block or modify its execution policy.

No language whitelist is built into the queue: identifiers for programming
languages, SQL, markup, shaders or future formats can be carried as data. Actual
interpretation remains the responsibility of an available, authorized adapter.
Unknown/unsupported formats are not turned into executable support by buffering.
Do not treat side-effecting SQL, shell commands or training steps as replaceable.

The [polyglot example](../examples/latest_polyglot.sapp) keeps execution options
unset and therefore demonstrates the existing refusal. It never generates trust
or unsafe-execution acknowledgements. Callers using authorized dispatch pass
their policy options unchanged and remain subject to all existing hash binding,
permissions and execution limits. Neither buffering nor an acknowledgement of
handling a refusal proves that foreign work ran successfully.

The [boundary test](../tests/semantics/latest_updates_polyglot_test.sapp) passes
two pending versions for each of seven language/format labels through the real
facade. Exactly the latest versions reach dispatch and all remain unexecuted
without authorization. This verifies coalescing plus preserved refusal; it is
not successful execution coverage of those languages or formats.

Final validation: 47 local suites across the combined resource-scope and latest
update changes, compiled-native contract runs, examples, isolated build and
portable-lock publication passed. Twelve latest-update lint/syntax checks passed;
lint reports one used result variable as unused in the polyglot refusal fixture.
Review covered detached-batch ownership, finite reentrant submission, explicit
acknowledgements, no automatic retries and unchanged polyglot authorization.
No Specisoft server was used; other local modifications were preserved.
Graphify refresh exceeded its 35-second process limit and was terminated;
refreshed semantic coverage is not claimed.
