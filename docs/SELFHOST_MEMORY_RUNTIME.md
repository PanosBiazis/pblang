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

Direct VM/frontend execution still preserves uncaught exceptions without an
explicit failure close; final reclamation on that route depends on host GC.
Deterministic cleanup there remains open. Neither loader nor Android cleanup
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
