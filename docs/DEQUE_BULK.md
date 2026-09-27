# Deque bulk insertion and exact indices

`stdlib:deque.sapp` adds `extend_back`, `extend_front` and `compact`, avoids
per-element public calls in `from_array`, and rebases sparse indices before they
leave the exactly representable integer range.

## Public behavior

```text
q = from_array([a,b])
extend_back(q, [c,d])   -> [a,b,c,d]
extend_front(q, [x,y])  -> [x,y,a,b,c,d]
compact(q)             -> same values, head=0, tail=size(q)
```

All three new functions mutate and return the same deque. Bulk input must be an
array with at most 65,536 elements; invalid type/size is rejected before changing
or migrating the deque. This is a per-call cap, not a total queue capacity limit.
Empty batches are valid. `extend_front` preserves the input order, unlike repeated
front pushes in forward iteration order. `from_array` retains its original
uncapped constructor contract.

Legacy `{data: [...]}` queues still migrate on first use. A legacy data array
can also be passed as the batch input: migration replaces queue storage without
modifying the source array. Input arrays and `to_array` snapshots own independent
slots, while contained maps/arrays remain shared payloads. `nil`, false and zero
are valid values. Existing empty pops still return nil; inspect size/is_empty
when it matters to distinguish an empty queue from a stored nil.

## Precision and compaction

The ordinary-number index limit is 2^53-1. Single pushes check the affected end;
bulk calls check the whole requested interval. Near the limit, the active slots
are copied to indices `[0,size)` before insertion. The new slot map is constructed
before replacing the old map. A missing-slot error during explicit compaction
therefore leaves the original indexed queue intact. Invalid, nonfinite,
fractional, reversed or out-of-range compaction indices are rejected.

This prevents future rounding-induced overwrite/nonadvancing indices. It is not
a recovery tool for queues already corrupted by an older implementation. Queue
fields are internal state; callers should use the module functions rather than
forge or modify them. Regression tests construct tiny valid boundary states to
exercise the limit without trillions of operations.

Normal end operations perform a constant number of map actions. Rare automatic
rebasing and explicit `compact` cost O(n) time and temporary O(n) slots. Bulk
insertion costs O(batch size), plus O(queue size) if rebasing is needed. Queue
capacity remains caller-managed; there is no new global memory quota or thread-
safety guarantee. `clear` and emptying a queue continue resetting both indices.

## Verification

On the retained PBLang 0.6.0 Linux bootstrap, these four suites passed:

```sh
pblang run tests/semantics/deque_hotpath_test.sapp --allow-all
pblang run tests/semantics/deque_bulk_test.sapp --allow-all
pblang run tests/semantics/selfhost_deque_bulk_test.sapp --allow-all
pblang run tests/semantics/collections_test.sapp --allow-all
pblang run examples/deque_bulk.sapp --allow-all
```

The new suite compares 2,000 deterministic mixed operations to a plain-array model,
checks nil and shared payloads, legacy migration/aliasing, empty batches, malformed
inputs, accepted 65,536/rejected 65,537 batches, both precision boundaries, repeated
compaction and failure without partial publication. The two precision defects
are reproduced with the unchanged old implementation. The bounded self-hosted VM
exercises bulk calls, explicit compaction and automatic high-index rebasing.
Six touched/new PBLang sources pass source checking without errors or warnings.
Package build and manifest validation pass; the published lock uses relative paths.

## Performance evidence

```sh
pblang run bench/deque_bulk.sapp --allow-all
```

The unchanged old module is captured from `7b033278e7740980a9eab7ddaa9847539fe687e5`.
There are 20 workloads, two warmups and seven alternating-order samples, one full
operation per sample. Final arrays are checked for equality before timing; the
regression suite separately verifies operation results. New bulk methods are
compared to ordered repeated pushes. Raw samples, medians and interpolated p95
are in `bench/deque_bulk_results.json`. These are local timings, not universal
performance guarantees.

| Operation | Items | Before median ms | After median ms | Before p95 ms | After p95 ms | Time change |
|---|---:|---:|---:|---:|---:|---:|
| from_array | 0 | 0.002146 | 0.002384 | 0.003767 | 0.002718 | +11.1% |
| extend_back | 0 | 0.005484 | 0.005960 | 0.006390 | 0.006938 | +8.7% |
| extend_front | 0 | 0.005484 | 0.005722 | 0.005889 | 0.007463 | +4.3% |
| push_pop_back | 0 | 0.002146 | 0.002146 | 0.002384 | 0.009227 | +0.0% |
| push_pop_front | 0 | 0.002146 | 0.002146 | 0.002313 | 0.002146 | +0.0% |
| from_array | 8 | 0.020266 | 0.010252 | 0.020742 | 0.010657 | -49.4% |
| extend_back | 8 | 0.024557 | 0.016212 | 0.025272 | 0.016856 | -34.0% |
| extend_front | 8 | 0.025988 | 0.016689 | 0.028229 | 0.017166 | -35.8% |
| push_pop_back | 8 | 0.050783 | 0.051737 | 0.051451 | 0.059819 | +1.9% |
| push_pop_front | 8 | 0.050545 | 0.051975 | 0.063062 | 0.052547 | +2.8% |
| from_array | 128 | 0.297308 | 0.132799 | 0.304794 | 0.134587 | -55.3% |
| extend_back | 128 | 0.442505 | 0.154018 | 0.495601 | 0.259328 | -65.2% |
| extend_front | 128 | 0.306129 | 0.131369 | 0.314879 | 0.133038 | -57.1% |
| push_pop_back | 128 | 0.742435 | 0.803471 | 1.123905 | 1.077318 | +8.2% |
| push_pop_front | 128 | 0.744581 | 0.796318 | 1.367521 | 1.147532 | +6.9% |
| from_array | 1024 | 2.339840 | 1.026392 | 3.302932 | 1.492906 | -56.1% |
| extend_back | 1024 | 2.680540 | 1.062393 | 3.774047 | 1.718783 | -60.4% |
| extend_front | 1024 | 2.755165 | 1.304865 | 3.353572 | 1.625943 | -52.6% |
| push_pop_back | 1024 | 7.869005 | 7.427216 | 8.595300 | 7.727957 | -5.6% |
| push_pop_front | 1024 | 6.610870 | 6.676912 | 8.486605 | 9.452105 | +1.0% |

Nonempty construction used about 49–56% less time; bulk insertion used about
34–65% less. Individual push/pop workloads ranged from about 6% faster to 8%
slower. Empty construction/bulk calls had up to about 11% relative overhead at
microsecond scale. The index guard is a correctness cost, not a claimed speedup.

## Review scope

The change uses no network, filesystem, credentials, telemetry, callbacks or new
dependencies. Fixtures use synthetic data. Scoped review covered the full deque
module, repository references, intended publication files, bounds, retention,
compatibility and failure behavior. No production operation, dependency advisory
scan, full Rust suite or cross-platform runtime matrix was performed.
