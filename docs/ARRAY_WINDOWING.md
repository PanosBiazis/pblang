# Array chunk and stepped windows

`array.chunk(input,size)` keeps a final short chunk. `array.windows(input,size,
step=1)` returns only full windows, starting at 0, step, 2*step, and so on.
Both require array input and positive finite ordinary integer counts before
work, even on empty arrays. Fractional/nonfinite/exact-number values, bool,
text and containers are rejected without rendering the offending value.

```sapp
let A=import("stdlib:array.sapp");
let chunks=A.chunk([1,2,3,4,5],2); // [[1,2],[3,4],[5]]
let windows=A.windows([1,2,3,4,5],3,2); // [[1,2,3],[3,4,5]]
```

Chunk counts are clamped to remaining input length before computing end indices.
Windows returns empty for size > length, and stops before forming an unused next
index when step exceeds the remaining valid start interval. Huge finite integer
counts therefore never enter raw array_chunk or cause an unused overflowing
advance. Zero/negative counts reject, unlike functional.windows' legacy empty
sentinel. Fractional truncation and accidental iterable inputs are not supported.

Outer/inner arrays are independent shallow copies; nested maps/arrays remain
borrowed references. Slot mutation is isolated, payload mutation is not. No
callbacks are invoked; concurrent mutation and snapshot/ABA consistency are
not qualified.

## Known default-argument divergence

The retained interpreter rejects an explicit nil step. The PBLang-written
frontend currently substitutes the default 1 for explicit nil before library
entry, so that path accepts it and behaves like an omitted step. The selfhost
test records this separately from invalid-count conformance; it does not assert
parity that does not exist. Required nil size is rejected on both tested paths.
The language-level distinction between omitted and nil default arguments remains
open; this library patch does not repair the frontend.

The bytecode serializer separately rejects literal numeric constants outside
[-9000000000000000,9000000000000000]. The roundtrip test verifies successful
plain/optimized/restored execution with supported constants and the expected
rejection of a 1e20 literal; it does not bypass that existing artifact limit.

## Scope and outstanding boundaries

The algorithms and validation are PBLang in `array_windowing.sapp`; only validated
indices are passed to the existing native bulk `slice` primitive, which creates shallow
copies. Direct helper imports perform the same validation. This adds one
transitive module to array imports and can affect exact module/source budgets.
The existing public array.slice implementation remains unchanged.
Raw array_chunk is no longer called by
this wrapper, but direct raw calls remain available. Its retained VM fractional/
infinite conversion and C99 nonfinite/out-of-range size_t conversion/allocation
risks are not repaired. No new Rust source is added or modified.
Direct raw C99 slice calls also retain malformed-number conversion/type-parity
risks; this helper does not expose caller-supplied bounds to that primitive.

Chunk output is linear in input length. Stepped windows allocate
`(floor((n-size)/step)+1)*size` payload slots when size <= n, which can still be
quadratic. Caller resource budgets remain necessary: no hard heap/wall bound or
hostile-code sandbox is provided. Native allocation failure, concurrent mutation,
all-platform parity and general 1.0.0 readiness remain unproved.

The portable fixture passes on the retained host. The attempted retained CLI
C99 path emitted an unresolved import symbol and its C compilation failed;
no C99 execution qualification is claimed. Linking via other compiler paths is
a separate test, not inferred from numeric C99 benchmark success. The first
attempt using CLI build with an output option was also rejected as unsupported.
Only generated temporary C artifacts were removed after inspection.
