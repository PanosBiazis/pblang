# Unary memoization

`functional.memoize(f)` and `pipe.memoize(f)` share the PBLang implementation
in `stdlib/functional_memo.sapp`. Both return a unary callable with the same
calling convention as before. Use this optimization for pure callbacks whose
result depends on the argument, not changing external state.

## Cache identity and retention

- Keys carry a type tag: number `1` cannot alias string `"1"`, nor boolean/nil
  their textual spellings.
- The original scalar is checked on every bucket hit; lossy number formatting
  cannot return another numeric input's result. A colliding bucket may be
  replaced, affecting hit rate rather than correctness.
- Strings of at most 4096 language characters, booleans, nil and finite nonzero
  numbers are eligible. Zero is deliberately bypassed to avoid conflating
  signed-zero behavior. Non-finite numbers are bypassed.
- Arrays, maps, objects, functions and other non-scalars execute the callback
  without caching. No recursive traversal, string conversion or identity
  guessing is performed for these inputs.
- Each memoized callable retains at most 1024 entries. At capacity, existing
  hits still work; new distinct keys execute without retention. There is no
  background worker, eviction timer or external persistence.
- Only successful results are stored, including nil. Exceptions propagate and
  subsequent calls retry. Returned mutable results remain shared references,
  not defensive copies; callers must not mutate them if they require purity.

The limits bound entry count and eligible string-key length, not callback work,
arbitrary result size, aggregate VM heap, concurrency or process lifetime.
There is no exactly-once/concurrent deduplication guarantee. Cache state is
local to the callable and there is no logging/export interface.

## Compatibility

Previously both APIs cached by `str(argument)`, which could silently conflate
types and composite values. Composite, zero, oversized-string and capacity-miss
calls may now execute more often. Code relying on suppression of side effects
was not a pure memoization use case; use explicit application state for that
requirement. No new arguments, Rust source or bytecode-format change.

`memoize_identity` covers both public APIs, exact values and callback counts,
mixed scalar types, repeated/nil results, retry after exceptions, mutable input
and callable bypass, zero, and 1024/1025-entry and 4096/4097-character boundaries.
`selfhost_memoize_identity` compiles the reviewed module source as one unit and
compares plain/optimized/in-memory serialized execution. That test is not
independent module-loader, foreign-backend or device qualification.
