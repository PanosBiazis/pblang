# Set queries, partitioning and callback mutation

`stdlib:set.sapp` adds `intersection_size`, `is_proper_subset`,
`is_proper_superset` and `partition`. Size now reads map length directly, and
an impossible subset is rejected by cardinality before constructing a key list.

## Compatibility and identity

This remains the legacy string-keyed set: `str(value)` determines membership.
For example, number `1` and string `"1"` share one key, and the later `add`
replaces the stored representative. Nil is a valid member. These semantics
are deliberately unchanged, including the representative chosen by existing
algebra functions. Typed scalar identity in `collection_keys.sapp` is a separate
contract, not silently substituted into this module.

`to_array` follows runtime `map_keys` order. The retained host sorts string keys;
it does not preserve insertion order. Old comments claiming insertion order and
ignored duplicate representatives were corrected. Mutable payloads remain shared,
and mutating their display text does not automatically rekey existing entries.
Arbitrary/cyclic payload stringification is not hardened by this change.

## New functions

- `intersection_size(a,b)` counts shared keys without constructing a result set.
  It scans the smaller set, which must have at most 65,536 members. The larger
  set may exceed that limit. Empty intersections return zero.
- `is_proper_subset(a,b)` requires both subset membership and strictly smaller
  cardinality; `is_proper_superset` reverses the arguments. These use the existing
  subset contract without an added size cap.
- `partition(s,predicate)` returns `{matched: set, rejected: set}`, using at most
  one predicate call per visited member. The input must have at most 65,536 members,
  checked before callbacks or key-snapshot allocation. Both outputs own fresh
  storage but share payload references. Original keys/representatives are retained.

The scan limits bound snapshot member counts, not key byte lengths, payload size,
callback time or callback allocations. Existing constructors and other set
operations remain uncapped. Callbacks are trusted code, not sandboxed computations.

## Callback behavior

`map`, `filter`, `each`, `retain` and `partition` snapshot keys at entry, then read
live membership and the current representative before each callback. A removed
future key is skipped, clearing skips all remaining keys, and new keys absent
from the snapshot are deferred. A replaced or removed-and-readded snapshot key
can still be visited with its current value. This is a key snapshot, not an
immutable value snapshot or an ABA guard. Concurrent access is not guaranteed.

`filter` and `partition` classify the representative read before their callback.
`retain` removes the key after a false predicate, including if the callback
replaced that same key. Callback exceptions propagate immediately; earlier
callback side effects and earlier `retain` mutations are not rolled back. No
partial partition result is returned on error.

## Verification

Four suites passed using the retained PBLang 0.6.0 Linux bootstrap:

```sh
pblang run tests/semantics/set_hotpath_test.sapp --allow-all
pblang run tests/semantics/set_queries_test.sapp --allow-all
pblang run tests/semantics/selfhost_set_queries_test.sapp --allow-all
pblang run tests/semantics/collections_test.sapp --allow-all
pblang run examples/set_queries.sapp --allow-all
```

The query suite exhausts all 1,024 pairs of subsets of a five-element universe,
with independent overlap/cardinality oracles, proper-subset/superset checks and
partition coverage/disjointness. It also covers representative collisions, nil,
payload aliasing, output independence, callback removal/addition/replacement,
clear, callback exceptions, empty sets, accepted 65,536/rejected 65,537 scans,
and a small scan against a larger set. All four old callback invalidation failures
are reproduced using the unchanged baseline fixture. The bounded self-hosted VM
executes the actual module's queries, partition and removal-during-iteration.
Source checking of six new/touched PBLang files reports zero errors/warnings.
Build, manifest validation and the runnable example pass; lock paths are portable.

A separate retained-runtime limitation was reproduced with both old and new
modules: a nested callback created inside another closure can fail to capture a
loop-local variable used only by the innermost callback (`undefined global
'state'`). The callback-error regression creates the callback before invoking the
operation, avoiding that unrelated runtime issue. This change does not fix it.
Minimal failing shape:

```sapp
let S=import("stdlib:set.sapp");
fn invoke(f){return f();}
for operation in [S.each] {
    let source=S.from_array([1,2]);let state=map{n:0};
    invoke(fn(){return operation(source,fn(v){state.n=state.n+1;});});
}
```

## Measurements

```sh
pblang run bench/set_queries.sapp --allow-all
```

The unchanged baseline module comes from `8dbe1c9b9839c3ef9af9eb3511f9ab7d1daecc18`.
There are 28 workloads, two warmups, seven alternating-order samples and result
parity before timing. Calls/sample are 20 for size, three for subset/disjoint/count
queries and one for partition/each. Times are milliseconds per call. All samples,
medians and interpolated p95 are retained in `bench/set_queries_results.json`.
Partition is compared to two old filters with a pure predicate. Intersection
counts are compared to old intersection construction followed by old size.
These are local workload measurements, not cross-platform guarantees.

| Operation | Members | Before median ms | After median ms | Before p95 ms | After p95 ms | Time change |
|---|---:|---:|---:|---:|---:|---:|
| size | 0 | 0.001156 | 0.000846 | 0.001349 | 0.000858 | -26.8% |
| impossible_subset | 0 | 0.001272 | 0.002305 | 0.001327 | 0.002384 | +81.2% |
| disjoint_small | 0 | 0.003020 | 0.002384 | 0.004792 | 0.002495 | -21.1% |
| intersection_size | 0 | 0.005086 | 0.003258 | 0.005738 | 0.003338 | -35.9% |
| intersection_size_equal | 0 | 0.004927 | 0.003497 | 0.008384 | 0.003632 | -29.0% |
| partition_vs_two_filters | 0 | 0.007629 | 0.007868 | 0.007868 | 0.008273 | +3.1% |
| each | 0 | 0.001907 | 0.001907 | 0.002313 | 0.002146 | +0.0% |
| size | 8 | 0.004256 | 0.000834 | 0.004555 | 0.000858 | -80.4% |
| impossible_subset | 8 | 0.004927 | 0.001510 | 0.005007 | 0.001589 | -69.4% |
| disjoint_small | 8 | 0.007312 | 0.005007 | 0.011349 | 0.010347 | -31.5% |
| intersection_size | 8 | 0.013510 | 0.005802 | 0.014313 | 0.005992 | -57.1% |
| intersection_size_equal | 8 | 0.041326 | 0.017405 | 0.047096 | 0.019209 | -57.9% |
| partition_vs_two_filters | 8 | 0.061512 | 0.048876 | 0.063848 | 0.051975 | -20.5% |
| each | 8 | 0.015974 | 0.020981 | 0.016212 | 0.021148 | +31.3% |
| size | 128 | 0.220418 | 0.001538 | 0.236099 | 0.001814 | -99.3% |
| impossible_subset | 128 | 0.210285 | 0.002225 | 0.228365 | 0.002527 | -98.9% |
| disjoint_small | 128 | 0.216961 | 0.005086 | 0.222739 | 0.005746 | -97.7% |
| intersection_size | 128 | 0.238498 | 0.006517 | 0.243147 | 0.007025 | -97.3% |
| intersection_size_equal | 128 | 1.228015 | 0.402530 | 1.489369 | 0.477044 | -67.2% |
| partition_vs_two_filters | 128 | 0.851393 | 0.620127 | 1.117516 | 0.643611 | -27.2% |
| each | 128 | 0.244856 | 0.293255 | 0.399971 | 0.496221 | +19.8% |
| size | 1024 | 2.214181 | 0.001597 | 2.907149 | 0.001920 | -99.9% |
| impossible_subset | 1024 | 2.804120 | 0.002782 | 3.063583 | 0.004141 | -99.9% |
| disjoint_small | 1024 | 1.720826 | 0.004371 | 2.715421 | 0.005468 | -99.7% |
| intersection_size | 1024 | 1.650651 | 0.007312 | 2.647074 | 0.009441 | -99.6% |
| intersection_size_equal | 1024 | 9.837389 | 2.563159 | 12.094514 | 3.183881 | -73.9% |
| partition_vs_two_filters | 1024 | 9.062767 | 6.553173 | 9.354258 | 8.855176 | -27.7% |
| each | 1024 | 3.232479 | 3.755331 | 4.671073 | 4.971528 | +16.2% |

Nonempty size queries took about 80–99.9% less time, and partition took about
20–28% less than two filters. Equal-set intersection counts took about 58–74%
less. Skewed/disjoint cases benefited further from eliminating full-set size
snapshots. The membership guard in `each` increased time about 16–31%; the empty
subset case increased about 81% at microsecond scale. Report these regressions
alongside the improvements.

Direct size is O(1). Key scans still allocate snapshots and inherit the retained
host's O(k log k) key sorting. `intersection_size` avoids an output set, not all
allocation. No claim of allocation-free or universally linear set algebra is made.

## Security/privacy scope

Review covered the complete owning module, repository references, intended
publication files, callback effects, bounds and existing key semantics. There
are no new network/filesystem/credential/telemetry operations or dependencies.
Fixtures use synthetic data. No production probe, full Rust/cross-platform suite,
dependency advisory scan or comprehensive review of legacy stringification was run.
