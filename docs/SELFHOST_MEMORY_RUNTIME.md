# Memory ownership in the PBLang VM

The bytecode VM in `stdlib/compiler/vm.sapp` implements scalar ownership and
lifetime management through `stdlib/compiler/vm_memory.sapp`, written in
PBLang. Its memory builtins do not forward to the retained host's Rust ownership
tracker. This is an implemented part of runtime migration, not a translation or
replacement of the entire native runtime.

## Execution routes

`compiler/selfhost_frontend.sapp`'s `frontend_run` and `frontend_run_bounded`
share one private ownership context across module initialization and the main
program. `compiler/vm.sapp`'s public run helpers also create this context.
The ordinary retained `pblang run program.sapp` still uses the host runtime;
merely rebuilding a compiler does not change that execution route. C99, WASM,
native backends and FFI do not inherit this engine automatically.

The orchestration context is never inserted into guest globals, exports,
closures or the import cache. Host callers that explicitly supply a context to
`run_seeded`, `run_seeded_bounded` or `exec` own its closing obligation via
`close_execution`. These APIs are internal orchestration mechanisms, not guest
capabilities.

## Automatic reclamation and safety

Use `memory.scope(fn() { ... })` or `memory.scope_result(fn() { ... })` from
`stdlib:memory.sapp`; the equivalent wrappers in `memory_safety.sapp` also work.
For supported allocations, closing the scope releases its borrows first, then
removes its owner/value entries and lifetime indexes. Explicit free remains
available and removes the same indexes, so later scope cleanup does not double
free. No history of dead allocation records is retained.

```pblang
let M = import("stdlib:memory.sapp");
let value = M.scope(fn() {
    let handle = M.box(42);
    return M.read(handle);
});
print value; // 42
print M.stats().total_locations; // 0
```

The example above is guest source. To run guest source on this VM from a
retained-host driver, use the explicit frontend API (not the tree-interpreter
`pbi` route):

```pblang
let F = import("stdlib:compiler/selfhost_frontend.sapp");
let result = F.frontend_run("push_lifetime(); memory_alloc(42); pop_lifetime(); print memory_stats().total_locations;");
for value in result.output { print value; } // 0
```

A handle returned from its own scope is expired and cannot be read. Return a
scalar value when it must outlive that scope. `SafeRef`, `Box`, `Rc` and arena
maps do not gain lexical destructors; scalar owners allocated through their
helpers are reclaimed by the surrounding lifetime. User-held tracking maps and
arrays have their ordinary lifetimes and may still retain numeric handle IDs.

Shared borrows coexist, mutable borrows are exclusive, immutable owners reject
writes and unique moves, and a unique move permanently consumes the owner.
Free while borrowed fails. Legacy release by owner/kind selects the oldest
matching active borrow; release by exact borrow ID avoids that search.

Both ordinary and resource-bounded VM execution enforce these rules. No safety
off switch or project-size limit is added. Resource budgets remain a separate
choice of execution profile. A future source-local unsafe facility needs its
own explicit language contract, checker and backend tests; none is introduced
here.

## Value and failure boundaries

Supported owned values are numbers, strings, booleans and nil. Arrays, maps,
functions, builtin descriptors and foreign/reference values are rejected before
allocation or replacement. The retained host supports additional value categories, but the PBLang VM
cannot enforce alias-safe ownership of those objects without an identity-aware
representation. These memory builtins were previously unimplemented in the
PBLang VM. Deep copying would silently
change program semantics and is not used as a substitute.

Balanced `scope_result` callbacks reclaim on return and on caught callback
failure. Raw `push_lifetime` / `pop_lifetime` must remain balanced. Unmatched
manual pushes inside a callback are not automatically repaired; generic
try/catch does not add an implicit lifetime boundary. The root cannot be popped.

Normal whole-execution completion explicitly clears root allocations and any
unfinished scopes. The self-hosted loader (`run_pbck*`, byte-array, seeded/options,
pbrun and PBSELF routes) now owns its context and closes it after either success
or a caught runtime failure, before returning its existing structured result.
See [loader cleanup validation](LOADER_CLEANUP_VALIDATION_2026-09-16.md).

Android emulator `run_pbcs` and `compile_and_run` also close their private
context after success or caught failure, before recording a success event.
See [Android cleanup validation](ANDROID_CLEANUP_VALIDATION_2026-09-16.md).

Direct VM/frontend execution now closes its private scalar ownership context
before an escaping guest `throw` or `error(value)`, while propagating the original
value/message. Active guest try handlers prevent premature whole-run cleanup;
the selected handler is removed before its catch body executes. A nested catch
that rethrows is therefore protected only by its remaining outer handlers.
Normal success, nil/false values and handled failures retain their behavior.

`execution_context()` still defaults to caller-owned state. Internal orchestrators
can opt into escaping-guest-error cleanup with `execution_context(true)` when an
uncaught guest error ends the entire run. Ordinary VM run helpers and the two
frontend run routes opt in automatically, including module initialization. This
flag is not a guest capability. Explicitly supplied default contexts must still
be closed by the caller. The context and active-handler count must never be
mutated concurrently, shared by simultaneous executions, or exposed to guest code.

Controlled ownership failures (invalid handles, conflicting borrows, unsupported
values, arity, exhausted IDs and lifetime boundaries) now also close the private
engine state before escaping an owned execution. The original `self-host memory:`
reason is retained, including when cleanup itself fails. A guest handler prevents
whole-execution cleanup; once that handler is removed, a failure from its catch
body closes the state unless another outer handler remains. Aliased builtins and
nested guest calls use the same dispatch policy.

The private engine's `dispatch(state, name, args, floor, close_on_failure=false)`
keeps its four-argument caller-owned behavior. The VM supplies the fifth argument
from its ownership flag and current handler count. Each synchronous dispatch
replaces the private policy; explicit `unwind` resets it to caller-owned behavior.
No callback or reference to the VM context is stored in the engine, avoiding a new
context/engine reference cycle. This is not a synchronization primitive: the
existing rule against concurrent sharing of an execution context still applies.
After a controlled escape, an explicitly retained context may reference the
**closed, emptied state** until `close_execution` clears that reference. Do not
resume an execution that has ended; create a fresh context. Closing again is
idempotent. Implicit host failure before reaching the controlled `fail` function
is outside this guarantee.

Runtime quota failures now also close an owned scalar context. Limits remain
sticky and cannot be caught to resume the same budget: step, frame, stack,
local-slot, output, value-size and allocation/growth limits terminate the run.
Consequently quota cleanup applies even while guest handlers are active. It
never closes caller-owned contexts unless the orchestrator explicitly opted in.
The execution context is passed privately to the guard helpers; it is not stored
in or returned with the public runtime counters. Limits, step accounting and
counter shape remain unchanged.

Explicit VM admission errors (invalid call counts, declared function/method
arity, unresolved imports, invalid overload targets, builtin arities,
negative slots and explicit byte-conversion type checks) use the same ownership
rule as ordinary guest exceptions: close only if no guest handler can continue.
Previously explicit error messages are retained; newly checked builtin counts
report the builtin name and expected/actual count without argument values. Stack
traces can include a cleanup helper frame. This does not make every host builtin failure a controlled VM
error.

Other implicit host failures and arbitrary FFI exceptions are not covered by this path.
Predictable collection type failures have the narrower coverage described below. A host failure during cleanup is suppressed to retain the
original guest exception; complete reclamation in that case is not guaranteed.
General deterministic exceptional cleanup remains open. Neither loader nor Android cleanup
retries execution, rolls back mutations or reclaims arbitrary host/native
resources. Android copies top-level seeded globals, but nested maps/arrays
can remain aliased and retain guest mutations. Removing ownership entries does
not guarantee lower OS RSS.

Handles are positive integer IDs, never reused within one execution, with
checked exhaustion at 2^53-1. They are predictable IDs, not unforgeable security
capabilities. This ownership engine is not a hostile-code isolation boundary
or a complete bound on aggregate heap usage. Existing bounded-value admission
also checks the value being allocated/stored, independently of other arguments.
Module initialization currently receives separate runtime budgets: shared
ownership state does not create a cumulative step or aggregate-memory budget
for the entire import graph. That resource-accounting gap remains open.

## Compiler metadata and packaging

`compiler/vm_builtins.sapp` owns the memory builtin name table through
`memory_registry()`. Code generation imports this metadata without loading
`vm_memory.sapp`. The engine retains its existing `registry()` callable as an
alias of the same function; both APIs return fresh maps. Mutating one returned
registry cannot change future lookups or another compiler instance.

The VM still imports its engine statically. This preserves source linking and
freezing: the current inliner rejects nested runtime imports. Consequently this
change avoids engine loading for metadata/codegen-only consumers, but does not
promise lazy engine loading for the full frontend, VM or packaged applications.
It neither changes scalar cleanup rules nor enables cycle collection or FFI
cleanup. Actual OS memory/time savings depend on the importing application.

Coverage: `tests/semantics/selfhost_memory_metadata_test.sapp`; example:
`examples/selfhost_throw_cleanup/memory_metadata.sapp`.

## Observability and cost

`memory_stats()` returns current owner/borrow counts and lifetime depth, plus
`allocations`, `reclaimed` and `peak_locations`. It returns aggregate values,
not the private ownership maps. Removing a scope visits only IDs indexed to
that scope, not historical allocations. Allocation adds owner and scope index
entries; each borrow adds owner, scope and borrow index entries. Scope cleanup
uses temporary snapshots proportional to that scope's active IDs. Exact map
operation cost depends on the host implementation.

Programs that never invoke memory builtins leave the ownership engine state
unallocated. VM calls still carry a shared execution-context reference; measured
cost must therefore be checked even for programs without explicit ownership.
See [validation and migration gaps](SELFHOST_MEMORY_VALIDATION_2026-09-15.md).

Regression coverage: `tests/semantics/selfhost_throw_cleanup_test.sapp`. The
`examples/selfhost_throw_cleanup/main.sapp` driver shows automatic frontend
behavior and the explicit orchestration option.

Controlled-memory regression coverage:
`tests/semantics/selfhost_memory_failure_cleanup_test.sapp`; runnable example:
`examples/selfhost_throw_cleanup/memory_failure.sapp`. The suite covers bounded
and ordinary routes, nested handlers, caller-owned contexts, ID exhaustion and
idempotent closing. This does not activate the PBLang VM inside applications
that still use the retained host directly.

Quota/admission regression coverage:
`tests/semantics/selfhost_admission_cleanup_test.sapp`; runnable example:
`examples/selfhost_throw_cleanup/quota_failure.sapp`. The tests include nested
handlers, borrowed contexts, pre-mutation rejection and unchanged public runtime
counter fields. See also the existing bounded VM, tick-accounting and UTF-8
budget suites. A malformed host value or arbitrary builtin exception that occurs
before an explicit VM rejection can still escape without this cleanup.


### Dynamic builtin argument counts

The VM checks wrong builtin argument counts before indexing an argument list or
calling a primitive. This also covers source programs whose callee is a function
parameter, and direct VM/PBCS chunks. For example:

```pblang
fn call(f, values) { return f(values, 99); }
let values = [7];
call(array_pop, values);
```

The call now fails and `values` stays `[7]`. Previously the extra argument was
ignored and the array became empty. A missing argument to `array_push` previously
escaped through a host index error without closing an opted-in scalar context.
Both failures now use the existing controlled-exception cleanup path. A guest
`catch` retains usable allocations; an escaping error closes owned contexts;
borrowed contexts remain the caller's responsibility. Arguments are still
evaluated once, in normal language order, before argument-count rejection.
Those evaluation side effects are not rolled back.

`compiler/vm_builtins.sapp::fixed_arities()` returns fresh fixed-count metadata.
The VM caches its own copy once and consults it only in generic/fallback dispatch.
Valid scalar fast paths still avoid temporary argument arrays and this lookup.
`print` remains variadic, `rational` accepts one or two arguments, `import` requires
one path, and memory builtin contracts remain in the ownership engine. Existing
explicit clock/sort/byte-conversion arity diagnostics are preserved. Code relying
on silently ignored extra arguments must be corrected.

Coverage: `tests/semantics/selfhost_builtin_arity_cleanup_test.sapp` and
`examples/selfhost_throw_cleanup/builtin_arity.sapp`. This checks argument counts,
not every builtin argument type or host failure. General FFI/resource cleanup,
composite/cyclic ownership and other backends are outside this guarantee. No host
runtime change or general performance improvement is claimed.


### Collection type failures and exact numeric size checks

The PBLang VM now closes eligible owned state before a guaranteed collection
receiver/key type rejection, then invokes the original host primitive once to
preserve its message. It covers array/map operations, length and membership,
including the length instruction used by for-in loops. Handled errors,
caller-owned contexts, valid nil/false results and successful mutations retain
their behavior. Invalid bounded push calls now reach the original array-type
error rather than an earlier length error.

Bounded value-size guards also distinguish actual strings from boxed exact
numbers. This restores valid bigint/rational/decimal execution across stack,
local, output and return paths without treating numeric objects as collections.
Numeric digit/bit quotas and arbitrary host exceptions remain outside this
change. See [contracts, parity tests and limits](SELFHOST_COLLECTION_BOUNDARY.md)
and `examples/selfhost_throw_cleanup/collection_types.sapp`.

### String allocation admission failures

Known argument rejections for repeat, replace and padding now skip size
arithmetic, close eligible owned scalar state, then invoke the original primitive
once. This preserves its first diagnostic and guest argument evaluation, while
handled and caller-owned contexts stay live. See [string admission and its
limits](SELFHOST_STRING_ADMISSION.md), including the separate existing
empty-search replacement estimate gap. Arbitrary host/FFI failures remain outside
this guarantee.
