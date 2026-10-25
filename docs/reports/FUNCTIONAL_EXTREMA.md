# Functional extrema: callback evaluation

`functional.max_by(values, key)` and `functional.min_by(values, key)` return
the input element with the greatest/least numeric key, or nil for empty input.

```sapp
let F = import("stdlib:functional.sapp");
let rows = [map{ name: "first", score: 4 }, map{ name: "second", score: 9 }];
let winner = F.max_by(rows, fn(row) { return row.score; });
assert(winner.name == "second");
```

For a non-mutating traversal of n elements, `key` runs exactly n times, in
input order, unless a callback/comparison throws. Empty input makes no calls;
a singleton makes one. The first key initializes the selection and is not
recomputed. Each later key is evaluated once before comparison. Equal keys
retain the first element. Exceptions propagate immediately with no retry or
visits to remaining elements; prior callback effects are not rolled back.

The selected value is not cloned: mutable objects retain their original
references. The library does not structurally mutate the input array.
Callbacks must not add/remove/reorder elements in that array during traversal;
snapshot semantics, concurrent mutation and purity enforcement are not provided.

Use comparable finite numeric keys. Existing runtime comparison semantics
remain in force: there is no new comparator, eager key-type validator, NaN
policy or generic total-order promise. For example, a singleton performs no
comparison, so a nonnumeric key is not necessarily rejected there. To select
strings by length, pass `len`, not `identity` (string ordering is unsupported
by the reviewed numeric comparison path).

## Compatibility and evidence

The old implementation evaluated the first key twice. Pure deterministic
numeric callbacks keep their selected values and tie behavior; effectful
callbacks deliberately lose that duplicate call and may now select a different,
correct element. Code relying on the extra callback must migrate that effect
explicitly. Evaluation remains linear with constant selection state, excluding
callback allocations/work; this is not a hard heap/time guarantee or measured
optimization speedup.

`functional_extrema` checks exact callback order/counts, empty/singleton inputs,
ties, reference preservation, negative keys, numeric text projections,
unsupported comparisons, throwing callbacks and object-bound generic validation.
`selfhost_extrema` imports the actual library through the bounded PBLang
frontend/VM and checks exact results and effects. These are focused retained-host
and PBLang VM checks, not qualification of all native backends or devices.
