# SQLite callback cleanup and placeholder generation

## Optional one-connection cleanup

Import `stdlib:db_owned.sapp` to use `with_sqlite(path, callback)` or
`with_sqlite_result(path, callback)`. Existing `db.sapp` users do not load the
helper automatically. The callback receives the ordinary SQLite connection
map. See `examples/db_owned.sapp` for an in-memory example.

The helper opens once, captures the original numeric handle, runs one trusted
synchronous callback and attempts `sqlite_close` on that captured handle after
normal return or a catchable callback error. Changing the connection's `handle`,
`kind` or `close` fields cannot redirect the helper's cleanup to another
connection. A callback may close it early; the retained runtime treats a later
close of the missing handle as an idempotent operation. Extra connections
opened by the callback remain the callback's responsibility.

`with_sqlite` returns the callback value, including `nil`, `false`, arrays and
maps, or throws a fixed error. `with_sqlite_result` returns:

| Field | Meaning |
| --- | --- |
| `ok` | Open, callback and close call all completed successfully |
| `value` | Callback result only when `ok`; otherwise `nil` |
| `error` | `nil` or a fixed message without path, SQL, handle or exception content |
| `open_ok` | Connection acquired |
| `body_ok` | Callback returned normally |
| `close_call_ok` | Close builtin returned without throwing; true when nothing was acquired |

The fixed messages are `db: SQLite open failed`, `db: SQLite callback failed`
and `db: SQLite close call failed`. A close-call failure overrides a callback
result/error and clears `value`. The callback is not called after open failure.
A non-callable callback is caught after opening and receives normal cleanup.

This is cleanup, not a transaction. Previously committed writes are not rolled
back after a callback exception. Use the existing `c.tx`/`c.savepoint` APIs when
transaction behavior is needed. Returning a connection, its raw handle or a
`prepare` closure does not extend its lifetime: further database operations
through that closed original handle fail on the tested retained runtime.

## Runtime boundaries

Only trusted synchronous callbacks are supported. Do not spawn database work
that outlives the callback. Current native exec/query removes a handle from the
table during work and reinserts it afterward, so a concurrent close can miss it.
The native close implementation discards table-lock errors and returns nil;
therefore `close_call_ok` is not independent proof that a resource was removed.
Hard termination, out-of-memory abort, poisoned native locks, counter exhaustion
and uncatchable failures are not repaired by this library.

SQLite handles remain process-global, predictable numeric IDs without
per-VM/realm authorization. This helper is not a sandbox, opaque ownership
system or cross-tenant isolation fix. Runtime/FFI work remains necessary for
those guarantees. No native Rust implementation or retained executable was
modified for this slice. SQLite execution is validated on the local retained
Linux runtime, not every backend or guest operating system.

## Faster `db.sql_placeholders(n)`

Exact nonnegative integer counts use doubling of identical `,?` chunks. The
algorithm uses logarithmically many concatenations and linear total copied
bytes instead of repeatedly copying each growing prefix. It returns identical
SQL text: zero produces an empty string, three produces `?,?,?`.

Legacy behavior for negative counts, fractions and non-numeric inputs is
preserved. Positive counts beyond the exact integer domain of the current
number representation, NaN and positive infinity now throw
`db: placeholder count is not exactly representable` before allocating a
result. This is a representation guard, not a source/project-size limit.

There is no new result-size cap and no dependency on `str_repeat`'s retained
10 MiB cap. A 5,300,000-placeholder / 10,599,999-byte case is tested. This API
materializes the complete string and needs memory proportional to the count.
Doubling accelerates huge allocations too: tools that accept untrusted counts
must apply their own explicit resource policy before calling it. It does not
make arbitrary counts safe or guarantee allocation success on all devices.
SQL engines may impose their own parameter-count limits independently.

Benchmark methodology, measurements and current validation are in
[DB lifecycle validation](DB_LIFECYCLE_VALIDATION_2026-09-15.md).
