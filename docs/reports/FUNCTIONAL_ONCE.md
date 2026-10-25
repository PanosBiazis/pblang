# Once: one attempt versus first success

`functional.once(f)` and the new `pipe.once_attempt(f)` share the PBLang-only
implementation in `functional_once.sapp`. They return a unary callable that
starts its callback at most once per live closure instance under sequential
execution. The first input is used; later inputs cannot restart the callback.

```sapp
let F = import("stdlib:functional.sapp");
let P = import("stdlib:pipe.sapp");
let seen = [];
let initialize = F.once(fn(x) { push(seen,x); return nil; });
initialize("first");
initialize("ignored");
// seen == ["first"]; nil is a successfully cached result.
let attempt = P.once_attempt(fn(x) { throw "initialization failed"; });
let first = try attempt(0) catch "original failure";
let second = try attempt(1) catch "incomplete attempt rejected";
```

## Lifecycle

| State | Call behavior |
| --- | --- |
| Not started | Mark started, invoke callback with the first input. |
| Started, not completed | Throw `once: call started without a completed result`; do not invoke callback. |
| Completed | Return the cached value, including nil/false/zero/empty values. |

Completion is published only after the callback returns. There is no catch or
defer around it, preserving the original first ordinary exception. A subsequent
call on the same surviving incomplete closure raises the constant diagnostic;
it does not retain/replay the original error value. Recursive re-entry before
completion is rejected by the same guard. If the callback catches that nested
error and then returns normally, its result is successfully cached.

The previous functional.once marked the closure called before invoking f, but
returned nil on later calls after failure. This false-success behavior is fixed.
Applications relying on that nil must explicitly handle failure. Successful
caching and first-input behavior retain the public API shape.
The shared helper adds one transitive module to relevant imports; callers with
exact module/source budgets should account for it rather than assume unchanged
loader cost. Existing tested bounded imports still fit their stated limits.

## Legacy pipe.once and migration

`pipe.once(f)` is intentionally unchanged: an ordinary failure permits a later
attempt, and the first successful result is then cached. It does not reject
recursive re-entry. This can repeat partial callback effects; use it only when
those semantics are intentional. Choose `pipe.once_attempt` when failure must
consume the attempt. Replacing pipe.once is an application policy decision, not
a silent change to existing code. Signal/event/concurrency APIs named once are
separate and were not changed by this slice.

## Limits

The flags are not atomic. There is no thread-safe/global once, inter-process or
cross-VM persistence, reset API, distributed exactly-once guarantee or unforgeable
capability. Spawn may copy closure state into another VM. Callback effects before
an exception are not rolled back. Cached mutable values remain shared references
for the wrapper's lifetime, without snapshots, eviction or secret scrubbing.

These wrappers do not bound arbitrary callback time/allocations or sandbox it.
Recovery after sticky VM quota failures, engine faults or process termination is
not established; such failures may prevent further invocation or destroy state.
The implementation does not catch/clear quotas. Returned runtime error values
retain the runtime's ordinary semantics; this slice does not redesign errors.

Tests cover initial failure followed by rejection, throw nil, nil/false success,
first-input effects, borrowed results, caught/uncaught re-entry, malformed callback,
and preserved legacy retry behavior. The actual libraries are also imported and
run through the PBLang-written frontend/VM with explicit step/frame/module/source
limits. An uncaught fixture checks original first-error text separately because
the current catch expression does not expose an error binding. A second uncaught
fixture checks the later constant diagnostic without echoing the original payload.
These are `tests/fixtures/once_attempt_first_error.sapp` and
`tests/fixtures/once_attempt_incomplete_error.sapp`; both intentionally exit with
failure when run directly. They are not ordinary passing semantic suites.
The standalone trusted helper source also passes plain/optimized/in-memory
serialized VM execution with matching plain/restored instruction counts; its
test file is read before the size assertion, not a bounded file-read primitive.
No Rust source,
opcode-format, full-suite or universal-platform qualification is included.
