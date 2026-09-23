# Scoped cleanup of external resources

`stdlib:resource_scope.sapp` provides synchronous cleanup of explicitly
registered resources, written entirely in PBLang. Cleanup runs automatically
when the scope's body finishes, returns an error result or throws a catchable
exception. Register each resource's release callback immediately after acquiring
it. No resource is discovered automatically from an arbitrary numeric handle.

```pblang
let R=import("stdlib:resource_scope.sapp");
let outcome=R.with_scope(fn(scope){
    let handle=memory_alloc(42,true);
    scope.defer(fn(){memory_free(handle);return map{ok:true,value:nil};});
    return map{ok:true,value:7};
});
print outcome.ok;
```

The [SQLite example](../examples/resource_scope.sapp) opens an in-memory database,
registers its original handle for closing and returns query data. Return copied
data/results from a scope, not a live handle whose resource will already be
closed when the caller receives it.

## Callback and outcome contracts

The body and every cleanup callback must return either `{ok:true,value:...}` or
`{ok:false,error:...}`. `ok` must be boolean; the corresponding field must exist.
Both successful nil values and explicit nil errors are supported.

`scope.defer(callback)` registers a zero-argument cleanup and returns its
zero-based registration ID. Cleanup runs in reverse registration order so a
dependent resource can be released before its dependency. Each registration is
removed before invocation. An exception, an invalid result or a failure result
is recorded, and remaining callbacks are still attempted. No callback is retried.
Registering while cleanup is running, or through an escaped facade after the
scope has closed, fails. Nested scopes maintain separate ledgers.

The outcome is a scoped-operation report, not a drop-in `Result`:

| Field | Meaning |
|---|---|
| `ok` | Valid successful body result and no reported cleanup failure |
| `value` | Body success value, also retained if cleanup subsequently fails |
| `error` | Original structured body error, or nil when unavailable/not applicable |
| `body_status` | `returned`, `exception`, or `invalid_result` |
| `body_result` | Snapshot of the valid body result's standard fields, otherwise nil |
| `original_error_available` | Body explicitly returned an error result, including a nil error |
| `cleanup_ok` | Every registered cleanup returned a valid success result |
| `cleanup_errors` | Registration ID, failure kind and available error-result snapshot |
| `cleanups_attempted` | Number of callbacks attempted |

Failure kinds are `exception`, `invalid_result` and `error_result`. Result
envelopes are snapshotted before later cleanup code can replace their fields.
Error/value objects themselves retain their existing aliases; this is not a deep
copy or immutable error representation. Additional custom envelope fields are
not copied. No contents are logged, serialized or persisted by this library.

## Important limits

- The current `try ... catch` language expression does not expose the original
  host exception message. For such exceptions the report deliberately uses
  `body_status:"exception"`, `error:nil`, `original_error_available:false`.
  No fabricated message is presented as the original error. Use explicit error
  results where preserving the original application error is required.
- A successful release callback means it reported success; it is not independent
  proof that an OS/driver released a resource. A failed callback may leave its
  resource open. The caller must inspect the report and choose an appropriate
  recovery action; blind retries can be unsafe.
- Callbacks are trusted and synchronous. There is no sandbox, thread safety,
  async cancellation protocol, forced termination or ownership proof for FFI.
  An infinite callback, process termination, out-of-memory failure or exhausted
  host budget can prevent cleanup from completing. No limits are bypassed.
- Registration itself requires memory. Acquisition before failed registration
  remains a caller/runtime responsibility. Registering the same resource twice
  creates two independent cleanup obligations; handle identity is not inferred.
- This is automatic cleanup **within this API**, not universal compiler-inserted
  cleanup of all resources. Other modules do not automatically import this optional library. The PB-MCP
  GTK viewer explicitly uses it; the current installed integration and measured
  scope are recorded in [allocation cost](RESOURCE_SCOPE_ALLOCATION_COST.md).
- Cyclic references, implicit-error diagnostic capture in the general runtime,
  general destructor/FFI ownership and independent backends remain open work.
  General retained-thread cycle collection cannot be treated as owner-isolated:
  complete roots across suspended callers cannot currently be proven. The
  optional `runtime_gc` facade refuses that operation; existing desktop
  maintenance uses a separate retained-host collection path, unchanged here.

## Verification

The same contract tests pass on the retained host, through the self-hosted module
loader, and after compiling the library plus tests to native bytecode. A real
SQLite test triggers an out-of-bounds error after opening a connection, verifies
exactly one close callback, and confirms that the handle is subsequently unusable.
Tests cover reverse order, nested scopes, errors in both phases, non-callable
cleanup, malformed results, nil/false values, closed-scope registration and
mutation of shared result envelopes by later callbacks.

Tests:
[contract](../tests/semantics/resource_scope_test.sapp),
[self-hosted](../tests/semantics/resource_scope_selfhost_test.sapp),
[SQLite](../tests/semantics/resource_scope_sqlite_test.sapp).

The final validation passed 44 local suites, an additional compiled-native
contract run, the SQLite example, isolated build and portable-lock publication.
Ten lint/self-hosted syntax checks passed; lint retained three advisory messages
in test fixtures (shadowing, explicit boolean comparison and a used variable
reported as unused). Focused review checked one-attempt cleanup, result-envelope
mutation, registration after closure and separation of body/cleanup failures.
No external capabilities or runtime quotas were changed.

A separate retained-bootstrap limitation was reproduced during testing:
`fn(s){s.defer(fn(){return value;});return value;}` can fail to resolve `value`
through two enclosing function scopes. A one-level callback factory avoids
that bootstrap defect; the self-hosted compiler is tested separately. This
library does not repair that retained Rust compiler.

## Performance methodology

The benchmark compares manual successful cleanup against scope registration and
reporting, with the same retained binary and same two callbacks per iteration.
Each callback increments a counter and returns success. Both paths import the
library, run 50,000 iterations and produce exactly 100,000 increments. The manual
baseline does not provide the scope's failure-reporting guarantees; the difference
measures the cost of that additional behavior, not a speedup claim.

Two warmups and seven alternating measured processes per variant; GNU time records
process CPU (user + system), wall time and peak RSS. P95 is the largest of seven
samples. The initial and optimized library revisions are also compared on the
same workload. The earlier optimized implementation stores callbacks directly in the
pending array, derives IDs from their positions, and uses private tuples for
capturing callback outcomes instead of temporary maps. Public reports stay maps.
The original measurement baseline is preserved in
[resource_scope_initial.sapp](../bench/fixtures/resource_scope_initial.sapp);
production callers import `stdlib:resource_scope.sapp` instead.

The benchmark runner uses `for i 50000` and either calls the two cleanup functions
directly or calls `with_scope` with two `scope.defer(close)` registrations. Each
body returns `{ok:true,value:7}`; each cleanup returns `{ok:true,value:nil}`.
Assert every outcome is successful and print only the final counter. Measure
`/usr/bin/time -f '%U %S %e %M'` outside build/lock publication and other tests.

Measurements on 2026-09-20:

| Variant | CPU median / P95, s | Wall median / P95, s | Peak RSS median / P95, KiB |
|---|---:|---:|---:|
| Manual success path | 0.14 / 0.15 | 0.15 / 0.16 | 25356 / 25640 |
| Initial scope implementation | 1.32 / 1.52 | 1.33 / 1.52 | 82664 / 82820 |
| Optimized scope implementation | 1.01 / 1.11 | 1.01 / 1.12 | 71516 / 71660 |

The private representation optimization reduced median CPU about 23% and peak
RSS about 13% compared with the initial scope implementation. Against direct
manual calls, scope registration/reporting still costs 0.87 additional CPU
seconds across 50,000 scopes and uses more peak RAM. There is no zero-overhead
or general RAM-saving claim. This synthetic workload uses exceptionally cheap
callbacks; actual file/database/device workloads and PB-MCP must be measured
separately. Retained-host weak allocation registries can also grow during long
runs even after resource cleanup; this library does not repair host heap GC.

## Current allocation follow-up

[Scoped child-process ownership](PROCESS_SCOPES.md) adds process-specific exit
policies, a default cancellation path, and explicit recovery ownership. The public
process backend still fails closed until its FFI contract is verified.

[Callback allocation cost and installed desktop integration](RESOURCE_SCOPE_ALLOCATION_COST.md)
replaces the earlier private tuples with exception state on the existing ledger,
including mixed-failure measurements and current compatibility limits.
