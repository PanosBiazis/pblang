# pblang memory model (v0.2)

## What is “heap allocated” today

pblang currently uses Rust `Rc`-based heap objects for runtime values:

- `Obj::String(String)` via `Rc<Obj>`
- `Function`, `Closure` via `Rc<...>`
- `Module` via `Rc<RefCell<Module>>`
- `Upvalue` via `Rc<RefCell<Upvalue>>`

This means “ownership” is shared and lifetime is managed by reference counting.

## What holds references

- **VM stack**: `Vec<Value>` holds `Value::Obj/Function/Closure/Module` references.
- **Chunk constants**: `Chunk.constants: Vec<Value>` keeps constants alive for the duration of the chunk.
- **Globals**: `Module.globals: HashMap<String, Value>` keeps referenced values alive across calls.
- **Closures / upvalues**:
  - `Closure.upvalues` holds `Upvalue` references.
  - `Upvalue::Closed(Value)` holds captured values alive even after stack slots are gone.
- **Module cache** (runtime import): the VM keeps a cache of imported modules so multiple `import(path)` calls return the same module object.

## Current consequences

- This is **usable and simple**, but:
  - reference cycles are possible (e.g. module → closure → module),
  - so a pure RC design can leak memory in cyclic graphs.
  - native resource handles (tcp/ffi/sqlite) must be closed, otherwise the process can hold OS resources.

## Safety notes (buffer overflows, UBs)

- The pblang runtime is written in **safe Rust** for the core VM/value stack, so classic buffer overflows
  are not expected in normal execution.
- The main safety “escape hatches” are:
  - **FFI** (`ffi_call`): calling the wrong symbol/signature can crash the process (this is inherent to C ABI interop).
  - **External libraries** (SQLite, TLS, etc): safety depends on their correctness too.

## Debug stats (today)

We expose a small builtin `gc_stats()` that reports allocation counters (starting with string allocations).
This is **not GC**; it’s just instrumentation to guide the next step.

Current counters include:
- `string_allocs`
- `array_allocs`
- `map_allocs`
- `module_allocs`

## Resource collection helper (today)

We expose `gc_collect()` as a pragmatic “cleanup” builtin:

- **Native resources**: force-closes the runtime’s native handle tables (tcp/ffi/sqlite) by dropping all entries.
- **RC cycle collection (mark/sweep over containers)**: runs a tracing pass from VM roots, then clears
  unreachable `array` / `map` / `module` containers to break reference cycles (e.g. `a[0] = a`).

This is not a full moving GC heap; it’s a safe, incremental way to eliminate the most common RC-cycle leaks
while pblang still uses `Rc` for object lifetimes.

## Bounded pblang allocation policies

`stdlib:memory_safety.sapp` supplies runtime-owned boxes, borrow guards,
checked slices, and bounded arenas. `bounded_arena(name, max_allocations,
max_units)` rejects work before allocation when either quota is exhausted.
Callers define units as bytes, tokens, nodes, pixels, packets, or another
quantity because the language cannot infer the retained deep byte size of an
arbitrary object graph. `arena_checkpoint` and `arena_rewind` release
speculative allocations in reverse order, while `arena_drop_all` removes the
arena's retained owner records as it deallocates them.

`stdlib:resource_budget.sapp` adds overflow-safe multi-resource admission and
mutable ledgers. `ledger_reserve` returns complete diagnostics;
`ledger_try_reserve` is the lower-allocation boolean hot path. Both reject
unknown metrics and leave ledger state unchanged after rejection.

These are runtime policies, not a proof of memory safety for every device.
FFI/native libraries, generated syscalls, kernel drivers, DMA, firmware, and
physical hardware remain external trust boundaries. Device operations should
also use capability isolation and the fail-closed policies in
`stdlib:device.sapp`.

## Next step options

### Option A: RC-first, cycle breaker later

- Keep RC for everything
- Add a “cycle breaker” strategy (weak registries + periodic graph traversal from roots) to break RC cycles
- Pros: quickest path to a usable language
- Cons: cycles require discipline and careful design

### Option B: Mark-sweep GC

- Introduce a GC heap (mark roots from stack/constants/globals/frames/upvalues/modules)
- Pros: best long-term for language-grade runtime, handles cycles
- Cons: bigger refactor (need object headers, tracing, and GC-safe APIs)

## Decision for v0.x

For pblang v0.x, we stick with **RC-first** (what the runtime already uses via `Rc`), while
continuing to add allocation stats and invariants. We also ship an RC-cycle collector (`gc_collect()`)
to break cycles in container graphs. Longer-term, we can move to a true **mark-sweep GC heap** once the
object model grows further and needs predictable memory behavior under heavy workloads.

