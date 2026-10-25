# Optional automatic cleanup of one scalar owner

Import `stdlib:memory_owned.sapp` to use `with_box`, `with_box_result`,
`with_box_immutable` or `with_box_immutable_result`. This separate library is
loaded only when requested; ordinary `memory.sapp` users do not load its helper
implementation. It runs on the retained host and the PBLang bytecode VM.

```pblang
let M = import("stdlib:memory.sapp");
let Owned = import("stdlib:memory_owned.sapp");
let answer = Owned.with_box(40, fn(handle) {
    M.write(handle, 42);
    M.borrow(handle);
    return M.read(handle);
});
print answer; // 42; this helper's owner and scoped borrow have been removed
```

The helper allocates its owner before opening a nested borrow lifetime. It runs
the callback with that numeric handle, closes its borrow lifetime, then attempts
explicit free once. Balanced success and caught callback failures therefore
remove the known owner on both execution routes, including the retained host
whose ordinary scope exit only invalidates owner entries. Nested helpers retain
outer owners and borrows until their own callback exits. Manual early free is
idempotent. Returning a handle does not extend its lifetime.

## Results and failure handling

The result variants always return these fields:

| Field | Meaning |
| --- | --- |
| `ok` | Callback succeeded and cleanup was verified |
| `value` | Callback result, including nil/false, only when `ok` is true |
| `error` | Fixed value-free error string on failure; nil on success |
| `allocation_ok` | An owner was acquired |
| `body_ok` | Callback returned successfully; false if allocation/setup failed |
| `cleanup_ok` | Helper boundary matched and free succeeded; true when no owner was acquired |

Cleanup failure overrides callback success/failure and clears the returned
value. Errors do not embed callback content or handles. The current language
cannot recover the original caught exception payload; the throwing variants
raise the fixed result error. These helpers do not promise cleanup after process
termination, fatal native failure, exhaustion that prevents error handling, or
an uncatchable VM budget abort.

The helper snapshots lifetime depth and its pushed lifetime token. It pops
only at the expected depth and verifies the returned token and final depth.
An extra push, premature pop, or pop/re-push replacement reports unsuccessful
cleanup. It does not blindly pop caller scopes or speculate about repair.
The callback must use raw lifetime operations in strict nested balance.
Concurrent mutation of the same lifetime state is outside this synchronous API.

## Supported values and explicit boundaries

Supported initial values are primitive numbers, strings, booleans and nil.
Arrays, maps, functions and other reference/foreign values are rejected before
allocation and before callback execution. This is the same initial-value
contract on both execution routes and is a limit of this opt-in helper, not a
new source-size, project-size or general language restriction.

Callbacks are trusted ordinary program code. On the retained host they must
also keep replacements scalar: a direct raw `memory_set(handle, composite)`
can bypass this helper's initial admission. The PBLang VM itself rejects such
composite replacement. Numeric IDs and raw memory builtins are public; this API
is not callback isolation or an opaque capability boundary.

Retained-host composite ownership is not certified here. Source review found
non-transactional object-to-owner bookkeeping around failed store/free and a
single-owner mapping for aliased objects. No Rust source was changed. Supporting
those values requires a separately tested runtime fix or an identity-aware
PBLang representation; initial type checks alone cannot repair native aliases.

Only the helper's known owner is explicitly freed. Other allocations, open
files, sockets, transactions or user-held references created by the callback
remain its responsibility. The helper does not install lexical destructors,
change garbage collection, or guarantee an immediate OS RSS reduction.

## SafeRef correction

`memory.SafeRef` now starts inactive, permits idempotent fresh/repeated release,
rejects reacquisition while active, reads the correct owner directly, and rejects
access after explicit release. The historical `tracker` parameter remains for
source compatibility and is not consulted. Pass the original mutable guard map
as explicit `self`, not a frozen or fabricated replacement.

The old get/set calls omitted the tracker's explicit receiver. Actual baseline
probes on both retained and self-host execution rejected set at the arity check;
no cross-owner mutation was reproduced. The new setter rejects immutable or
inactive guards and reports active mutable write-through as unsupported. Neither
runtime exposes a token-aware borrowed-write operation, and temporarily
releasing/reacquiring would weaken the borrow contract.

A guard must not escape its acquisition lifetime. Its public `released` field
cannot detect automatic release by a scope while an outer owner survives; there
is no non-consuming borrow-token introspection. It is a convenience object,
not a destructor, an unforgeable capability or a stale-token validator.

## Cost

Automatic ownership adds checks and result bookkeeping; it is not free. Lifetime
depth currently comes from `memory_stats()`. On the retained host that also sums
borrow/reference metadata, so its cost can grow with ambient tracked state.
The PBLang VM uses direct indexed counts. A dedicated depth primitive is a
future optimization, not an implemented guarantee. No existing allocator or
normal compiler path is switched to the helper automatically.

See [measurements and current validation](OWNED_CALLBACK_VALIDATION_2026-09-15.md)
and the [broader runtime contract](SELFHOST_MEMORY_RUNTIME.md).
