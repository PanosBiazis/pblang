# Functional chunk and window size contract

`functional.chunk(array,size)` partitions an array into non-overlapping groups;
the final short group is retained. `functional.windows(array,size)` returns every
full sliding window at step one; short tails are omitted. Both return independent
outer/inner arrays with shallow, borrowed payload references.

```sapp
let F=import("stdlib:functional.sapp");
let batches=F.chunk([1,2,3,4,5],2); // [[1,2],[3,4],[5]]
let sums=F.map(F.windows([1,2,3,4],2),F.sum); // [3,5,7]
```

Both require an array and an ordinary finite integer `number` size, validated
before iteration even for an empty input. Nil, bool, text, containers, functions,
exact-number variants, fractional numbers, NaN and infinities throw constant
diagnostics that do not stringify the offending value.

- Chunk additionally requires size > 0; size > length returns one shallow group
  for nonempty input. Valid empty input returns an empty array.
- Windows preserves the legacy empty result for finite integer size <= 0 or
  size > length. Valid empty input likewise returns an empty array. This differs
  intentionally from `array.windows`, which rejects nonpositive size.
- Valid positive integer results and reference semantics are unchanged. A large
  finite integer size does not itself drive iteration: chunk visits the input,
  and windows returns early when size exceeds input length.

## Compatibility correction

Baseline `9e2e930` silently accepted fractional chunk sizes, producing groups
rounded up in length. The regression test fails before the guard is added.
Other invalid values could produce inconsistent empty/single-group results or
enter range conversion. Such inputs now fail explicitly, including on empty
arrays; callers must validate/convert sizes intentionally. Chunk's previous
zero/negative-size singleton behavior is no longer accepted. Windows keeps its
existing nonpositive-integer sentinel behavior but rejects negative fractions.
Accidental iterable/string inputs are now rejected instead of treated as arrays.

Guards do not inspect or deep-copy array elements. Mutating one result array
does not change sibling window slots or input slots; mutating a borrowed map/
array payload is still observable through every reference to that payload.
There is no concurrent snapshot/ABA guarantee.

## Remaining limits

This correction validates shape, not total resource usage. Chunk produces n
payload slots plus its groups. For 1 <= size <= n, windows produces
`(n-size+1)*size` payload slots plus outer entries: this can be quadratic in n.
Inputs/outputs must be budgeted by the caller; no new global item/output cap,
hard heap bound, preemption or hostile-code sandbox is established. There are
no callback invocations in these two helpers, but concurrent mutation remains
unsupported. Very large arrays are not qualified by these focused tests.

`array.chunk`/`array.windows`, `compute.chunk`, raw native builtins, and generator
counts in repeat/iterate are separate implementations and are not repaired by
this patch. No Rust/compiler/runtime implementation or import graph is changed.
Focused retained-host and actual PBLang-written frontend/VM tests do not prove
all-backend, emulator or physical-platform parity. The separate numeric session
benchmarks do not measure window construction or claim a speedup from validation.
