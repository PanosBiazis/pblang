# Signal lifecycle and reentrant emissions

`stdlib:signal.sapp` supplies synchronous in-process signals. This change adds
`disconnect(sig)`, fixes two callback-mutation cases, and removes a temporary
key array from `count(sig)`. Existing signatures and `clear` behavior remain.

## Lifecycle

Every result of `map_signal`, `filter_signal`, `scan_signal`, `distinct_signal`,
and `merge_signals` retains private `_upstream` unsubscribe callbacks. Call
`disconnect(derived)` to remove its registrations from its immediate sources.
This is idempotent, returns the signal, and keeps its local subscribers intact.
Direct `emit` and `subscribe` still work afterwards. Plain signals are unchanged
by `disconnect`; no automatic reconnect is provided.

`clear(sig)` only removes local subscribers. To tear down both sides, call
`disconnect(sig)` followed by `clear(sig)`. Disconnect is shallow: in a chain,
disconnect each intermediate derived signal when it is no longer needed.
Disconnecting one branch never disconnects other consumers of a shared source.
The merge operator removes both links even when both inputs are the same signal.
Callers must treat `handlers`, `next_id`, and `_upstream` as internal state.

## Emission semantics

`emit` snapshots subscription ids, then checks whether each subscription still
exists before calling it. A callback removed before its turn is skipped;
new subscriptions wait for the next emission. Clearing during emission skips
remaining callbacks. Callback errors remain isolated, and the return value counts
attempted callbacks, including those that throw.

`distinct_signal` records the current value before invoking downstream callbacks.
An immediate recursive emission of the same value is suppressed, and a recursive
changed value remains the latest value after the outer callback returns. `nil`
is a valid payload. Equality semantics and mutable payload aliasing are unchanged.

These guarantees cover synchronous callbacks. `emit_async` retains its existing
thread snapshot behavior and error handling; disconnect does not cancel a worker
already spawned. No new concurrency or thread-safety guarantee is claimed.
Arbitrary callback recursion and subscription growth remain caller-controlled;
this change does not impose a global subscriber or recursion limit. Disconnect
removes registrations, but is not a measured guarantee of garbage collection.

## Verification

Run from the package root with a compatible PBLang 0.6.0 executable:

```sh
pblang run tests/semantics/signal_stdlib_test.sapp --allow-all
pblang run tests/semantics/signal_enrich_test.sapp --allow-all
pblang run tests/semantics/signal_lifecycle_test.sapp --allow-all
pblang run tests/semantics/selfhost_signal_lifecycle_test.sapp --allow-all
pblang run examples/signal_lifecycle.sapp --allow-all
pblang run bench/signal_lifecycle.sapp --allow-all
```

All four suites passed on the retained Linux bootstrap, including the bounded
self-hosted VM. The lifecycle suite covers recursive duplicate/changed values,
nil payloads, every derived constructor, idempotency, unrelated listeners, same-
source merge, shallow chains, callback-time disconnect, clear and subscribe.
Both corrected bugs were independently reproduced using the unchanged old fixture.
Six changed/new PBLang sources passed source checking with zero errors/warnings.
Package build and manifest validation passed; the published lock is portable.

## Measurements

The fixture is the unmodified module at base commit
`127d5748abf85fe063cedf1ac3b7012275c7ca11`. The benchmark checks return-value parity,
uses two warmups, seven alternating-order samples, 100 calls/sample for `count`
and three for `emit`. Times are milliseconds per call from `clock()`; p95 uses
linear interpolation. These local measurements are not cross-platform promises.

| Operation | Subscribers | Before median ms | After median ms | Before p95 ms | After p95 ms | Time change |
|---|---:|---:|---:|---:|---:|---:|
| count | 0 | 0.001745 | 0.000980 | 0.001781 | 0.001093 | -43.85% |
| emit | 0 | 0.002305 | 0.002464 | 0.003044 | 0.002543 | +6.90% |
| count | 8 | 0.007353 | 0.000966 | 0.008013 | 0.001085 | -86.87% |
| emit | 8 | 0.022968 | 0.028769 | 0.023476 | 0.030589 | +25.26% |
| count | 128 | 0.282378 | 0.001080 | 0.305772 | 0.001236 | -99.62% |
| emit | 128 | 0.524521 | 0.651598 | 0.564822 | 0.682330 | +24.23% |
| count | 1024 | 2.169511 | 0.000746 | 2.513868 | 0.000842 | -99.97% |
| emit | 1024 | 4.269918 | 5.222321 | 4.799636 | 5.419008 | +22.30% |

`count` now reads map length directly, independent of subscriber count, instead
of allocating/enumerating keys. The existence check in `emit` increases steady-
state dispatch cost by about 22–25% in these nonempty workloads, while correcting
callback-time removal. Empty dispatch increased about 7% at microsecond scale.
Do not describe the whole signal library as uniformly faster.

## Security and privacy scope

The change adds no filesystem, network, credential, telemetry, dependency, or
external data operation. Tests and benchmarks use synthetic values. Review covered
the complete owning module, its repository callers, intended publication files,
retention through source registrations, callback mutation and reentrancy. There
was no production probe, dependency advisory scan, or cross-platform runtime run.
