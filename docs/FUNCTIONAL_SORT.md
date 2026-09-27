# Opt-in stable functional sorting

`functional.stable_sort_by(array, comparator)` is a PBLang-only, bottom-up merge
sort. It returns a new shallow array, keeps equal-key elements in their original
order, and does not write to the caller's array itself. Existing `sort_by`,
`sort_asc` and `sort_desc` retain their implementation and callback sequence.
Opt in when comparator call order/count is not part of application behavior.

```sapp
let F = import("stdlib:functional.sapp");
let rows = [map{name:"first",score:2},map{name:"second",score:1}];
let ordered = F.stable_sort_by(rows,fn(a,b) {
    if a.score < b.score { return -1; }
    if a.score > b.score { return 1; }
    return 0;
});
// ordered[0].name == "second"; rows[0].name == "first"
```

## Contract

- Input must be an array of at most 4,096 elements, checked before copying.
- Comparator returns a finite ordinary `number`: negative for before, zero for
  equivalent, positive for after. Nil, bool, text, containers, functions, exact
  numeric variants, NaN and infinities are rejected with a constant diagnostic.
  Subtracting two finite extremes may overflow; use relational comparisons as
  above when needed. This API does not itself compare or stringify payloads.
- Each merge decision calls the comparator once. Ties select the left element.
  Ordering/stability requires a deterministic, consistent total preorder;
  inconsistent results cannot be checked generally and may yield unsorted output.
- Empty/singleton input returns a new array without invoking or validating the
  comparator. Otherwise non-callables fail through normal invocation semantics.
- Callback errors propagate without retries. Side effects are not rolled back.
  Payload objects are borrowed references, not immutable/deep snapshots. Do not
  mutate ordering keys during sorting. A callback can mutate captured input or
  payloads, but cannot obtain the private slot arrays directly from this API.

## Cost and qualification limits

The algorithm uses two n-element shallow arrays (one for n<2), no recursion,
O(n log n) slot work and O(n) auxiliary slots. At n=4,096 it invokes at most
49,152 comparisons. These are algorithmic bounds, not a VM-wide heap limit:
payload size, callback allocations, callback runtime and external effects are
not limited by this helper. The comparator is trusted and must terminate.

The helper adds one transitive module to functional imports, which may affect
applications with exact module/source budgets. Host limits can be stricter than
the library cap (the pure PBCK evaluator's collection cap is 256). No compiler,
Rust, runtime or legacy sort implementation is changed. Focused host and
PBLang-written frontend/VM tests do not establish all-backend, C99, emulator,
physical-device or 1.0.0 readiness. This is not a hostile-code sandbox.

`bench/functional_sort_compare.sapp` compares unchanged legacy and opt-in APIs
using the same sorted/reversed/duplicate numeric arrays, exact reference results,
input-preservation checks, one first run (no cache flush), two warmups and five samples. Run with
`PB_SORT_MODE=legacy` or `PB_SORT_MODE=stable` under an external process limit.
Comparator counting is a separate untimed execution. The local report retains
baseline and post-change data; it is not evidence of universal speed superiority.

## Cached-key ordering and stable indices

Two additive APIs are available from both `functional.sapp` and
`functional_sort.sapp`:

```sapp
let sorted = F.stable_sort_by_key(rows, keyfn, compare_keys);
let order = F.stable_argsort_by_key(rows, keyfn, compare_keys);
```

The first returns the original payloads in stable key order. The second returns
zero-based indices into the input slot snapshot, preserving source index order
for ties. Indices are useful for applying one ordering to parallel arrays. If a
callback structurally mutates the caller's array, indices still refer to the
original snapshot; they are not remapped to the mutated array.

Input type and the 4,096-element cap are checked before any callback. A shallow
slot snapshot is then taken. The key function runs exactly once per input element,
left to right, before any comparison. Empty input invokes neither callback;
singleton input invokes the key function once and never invokes/validates the
comparator. An error in projection stops immediately without starting comparisons.
Comparator errors propagate after all successful projections; callback side effects
are not rolled back, and no partial sorted result is returned.

Keys may be arbitrary values because the supplied comparator owns ordering. It
must return a finite ordinary number and satisfy the same total-preorder contract
as `stable_sort_by`. Keys and payloads are borrowed references, not deep/frozen
snapshots. Do not mutate key objects while comparing them. Caching scalar keys
prevents later changes to source slots from changing the comparison inputs, but
does not freeze mutable objects returned as keys.

Implementation decorates each snapshot slot with a private two-element array:
`[key, payload]` for values or `[key, original_index]` for indices. It then
reuses the existing stable merge sorter. No extra module or
alternate sorting algorithm is introduced. Source slots, decorated records,
merge buffers and final output require O(n) extra storage with a larger constant
than the plain comparator API. Projections require exactly n calls; comparisons
remain bounded by 49,152 at n=4,096. Callback runtime, returned-key size and payload
size are not bounded by these counts. All legacy callback traces remain unchanged.
The existing finite-comparator test was rewritten as `order*0 != 0`, preserving
NaN/infinity rejection without a self-subtraction lint diagnostic.

## Cached-key verification

The following six suites passed on the retained PBLang 0.6.0 Linux bootstrap:

```sh
pblang run tests/semantics/functional_key_sort_test.sapp --allow-all
pblang run tests/semantics/selfhost_key_sort_test.sapp --allow-all
pblang run tests/semantics/functional_stable_sort_test.sapp --allow-all
pblang run tests/semantics/functional_stable_sort_bounds_test.sapp --allow-all
pblang run tests/semantics/selfhost_stable_sort_test.sapp --allow-all
pblang run tests/semantics/functional_combinators_test.sapp --allow-all
pblang run examples/functional_key_sort.sapp --allow-all
```

The new suite uses an independent value-bucket/index oracle for all 243 five-item
ternary arrays. It covers ties, descending order, exact projection count/order,
phase ordering, opaque keys, empty/singleton behavior, input snapshot mutation,
borrowed payloads, fresh output slots, invalid inputs and comparator results,
projection errors, 4,096 accepted / 4,097 rejected and every boundary index. The
helper and public facade agree. The new bounded self-hosted test executes both
actual imported APIs. Existing stable-sort tests additionally verify legacy
comparison traces and plain/optimized/serialized self-hosted execution.

Source checking reports zero errors. The helper/new tests/example/benchmark have
no warnings. Two pre-existing self-subtraction warnings remain in the unchanged
functional window validator and the exact baseline fixture. The former was
confirmed by checking the original functional source separately. No warnings
were suppressed and the baseline fixture was not altered.

## Cached-key measurements

```sh
pblang run bench/functional_key_sort.sapp --allow-all
```

The unchanged merge-sort fixture is from `1bd97b24180a46386dfdd00cac990b2f01c00419`.
Twenty-four workloads compare repeated projection inside the old comparator with
the new cached-key APIs. For indices the baseline composes decorated indices with
the old stable sorter. There are two warmups, seven alternating-order samples and
one full call/sample. Full result parity is checked before timing. Projection-call
counts are collected separately, outside timed work. Raw samples, medians and
interpolated p95 are in `bench/functional_key_sort_results.json`.

The cheap key reads one field. The computed key applies 24 deterministic arithmetic
steps, identically in both variants. No claim is made that every real projection
has that cost. These are local results, not cross-platform guarantees.

| Output | n | Pattern | Key | Old key calls | New key calls | Before median ms | After median ms | Before p95 ms | After p95 ms | Time change |
|---|---:|---|---|---:|---:|---:|---:|---:|---:|---:|
| sort_values | 8 | reversed | cheap | 24 | 8 | 0.07176 | 0.10228 | 0.07563 | 0.11044 | +42.5% |
| sort_indices | 8 | reversed | cheap | 24 | 8 | 0.05722 | 0.05579 | 0.10092 | 0.10843 | -2.5% |
| sort_values | 8 | reversed | computed | 30 | 8 | 0.29087 | 0.12398 | 0.39811 | 0.16036 | -57.4% |
| sort_indices | 8 | reversed | computed | 30 | 8 | 0.33998 | 0.12755 | 0.46558 | 0.19269 | -62.5% |
| sort_values | 8 | duplicates | cheap | 28 | 8 | 0.04220 | 0.06223 | 0.07639 | 0.08798 | +47.5% |
| sort_indices | 8 | duplicates | cheap | 28 | 8 | 0.05960 | 0.05603 | 0.06056 | 0.05643 | -6.0% |
| sort_values | 8 | duplicates | computed | 34 | 8 | 0.30994 | 0.13208 | 0.33460 | 0.18592 | -57.4% |
| sort_indices | 8 | duplicates | computed | 34 | 8 | 0.34094 | 0.12374 | 0.38223 | 0.13828 | -63.7% |
| sort_values | 128 | reversed | cheap | 896 | 128 | 1.19472 | 1.27292 | 1.62997 | 1.63844 | +6.5% |
| sort_indices | 128 | reversed | cheap | 896 | 128 | 1.47772 | 1.22881 | 1.67348 | 1.47014 | -16.8% |
| sort_values | 128 | reversed | computed | 1486 | 128 | 13.89360 | 3.36599 | 15.43763 | 3.58744 | -75.8% |
| sort_indices | 128 | reversed | computed | 1486 | 128 | 15.17487 | 3.13902 | 19.52207 | 4.63555 | -79.3% |
| sort_values | 128 | duplicates | cheap | 1418 | 128 | 1.88923 | 1.89042 | 2.81184 | 3.06108 | +0.1% |
| sort_indices | 128 | duplicates | cheap | 1418 | 128 | 2.21539 | 1.60623 | 2.72141 | 2.25663 | -27.5% |
| sort_values | 128 | duplicates | computed | 1480 | 128 | 13.82422 | 2.77972 | 15.09297 | 2.89152 | -79.9% |
| sort_indices | 128 | duplicates | computed | 1480 | 128 | 13.94105 | 2.80023 | 15.22603 | 3.21584 | -79.9% |
| sort_values | 512 | reversed | cheap | 4608 | 512 | 7.53927 | 7.23839 | 10.83965 | 8.45819 | -4.0% |
| sort_indices | 512 | reversed | cheap | 4608 | 512 | 9.10544 | 7.51948 | 10.93287 | 8.94692 | -17.4% |
| sort_values | 512 | reversed | computed | 7988 | 512 | 112.93030 | 24.40763 | 131.86314 | 25.66054 | -78.4% |
| sort_indices | 512 | reversed | computed | 7988 | 512 | 141.11304 | 24.55139 | 169.76261 | 25.43316 | -82.6% |
| sort_values | 512 | duplicates | cheap | 7586 | 512 | 17.62080 | 17.53688 | 17.98363 | 20.87986 | -0.5% |
| sort_indices | 512 | duplicates | cheap | 7586 | 512 | 24.62459 | 19.63043 | 35.29003 | 35.50527 | -20.3% |
| sort_values | 512 | duplicates | computed | 7812 | 512 | 128.36075 | 22.79449 | 227.15642 | 25.45006 | -82.2% |
| sort_indices | 512 | duplicates | computed | 7812 | 512 | 105.82685 | 16.58010 | 139.55238 | 23.98760 | -84.3% |

Computed-key workloads used about 57–84% less time. At 512 reversed records,
projections dropped from 7,988 to 512. Cheap-key value sorting ranged from about
4% faster to 48% slower, reflecting decoration/allocation cost; cheap-key index
sorting was about 3–28% faster than the composed index baseline. The existing
plain comparator API remains available when projection is trivial.

Review covered the owning helper, functional facade, callers/tests, bounds,
callback phases, shared references and intended publication files. No filesystem,
network, telemetry, credentials, dependencies or compiler changes were introduced.
Fixtures use synthetic data. No full Rust suite, dependency advisory scan or
cross-platform runtime matrix was run; existing retained-runtime nested-closure
capture limitations are not fixed by this change.

## Compact key records (follow-up)

The cached-key implementation originally allocated a three-field map per item,
retaining both payload and index even though each API returns only one. Private
pairs now retain just the projected key and the requested output. They are not
exposed to callbacks: the comparator still receives only the projected keys.
This removes named field lookups in the comparison wrapper and output extraction.
It does not remove the input snapshot, alter the merge algorithm, reduce its
comparison count or change the public API. No heap-byte reduction was measured.

The original source at `8ae53b6ddd0346ad7c0a8fead88c2df2362c5caf` is retained byte
for byte as `bench/fixtures/functional_sort_before_pairs.sapp`. The trace regression
compares both APIs with that baseline on 243 ternary arrays each (486 cases),
including the entire projection/comparison sequence and original source slots.
Additional negative cases stop comparators at calls 1/2/3 or return an invalid
map, checking immediate failure, all prior projections and no partial output.
Comparator-side source mutation and array-valued keys are checked against the
baseline. Existing input-bound, projection-failure, aliasing, NaN/infinity and
legacy-sort tests remain in effect.

`selfhost_key_sort_test.sapp` now also checks both cached-key APIs with actual
helper source through plain, optimized and serialized bytecode, with invalid
comparator rejection and matching comparison traces. Plain/restored instruction
counts must match. The helper source-size cap is a post-read assertion on a trusted
repository fixture, not a hostile-file pre-allocation bound.

Seven suites passed: the six listed above plus
`tests/semantics/functional_key_sort_trace_test.sapp`. Checks on the five changed
PBLang files report zero errors and zero warnings. Existing unrelated functional
window and older fixture warnings were not suppressed. Build, portable lock,
manifest validation and whitespace checks are required for delivery.

### Comparable measurements

Run `pblang run bench/functional_key_pairs.sapp --allow-all`.
Both sides call the same cached-key API on the same synthetic arrays and key
functions, including identical n projection calls. This is a different comparison
from the earlier table, which compared cached and repeated projections. Each run
uses two warmups, seven alternating-order samples and five complete calls/sample;
reported sample times are milliseconds per call. Full result parity and projection
counts are checked outside timing. Two independent runs are retained, including
all raw samples, medians and interpolated p95, in
`bench/functional_key_pairs_results.json`. No workload or sample was discarded.

| Output | n | Pattern | Key | Initial change | Confirmation before median ms | Confirmation after median ms | Confirmation before p95 ms | Confirmation after p95 ms | Confirmation change |
|---|---:|---|---|---:|---:|---:|---:|---:|---:|
| sort_values | 8 | reversed | cheap | -7.7% | 0.04549 | 0.04225 | 0.05616 | 0.04643 | -7.1% |
| sort_indices | 8 | reversed | cheap | -12.9% | 0.04478 | 0.04101 | 0.05313 | 0.06774 | -8.4% |
| sort_values | 8 | reversed | computed | -3.2% | 0.11139 | 0.11210 | 0.16989 | 0.13738 | +0.6% |
| sort_indices | 8 | reversed | computed | -7.4% | 0.11168 | 0.10462 | 0.11950 | 0.10952 | -6.3% |
| sort_values | 8 | duplicates | cheap | -7.8% | 0.04954 | 0.04501 | 0.05224 | 0.04559 | -9.1% |
| sort_indices | 8 | duplicates | cheap | -8.1% | 0.04864 | 0.04611 | 0.05585 | 0.04830 | -5.2% |
| sort_values | 8 | duplicates | computed | -2.1% | 0.11816 | 0.11148 | 0.15170 | 0.13432 | -5.6% |
| sort_indices | 8 | duplicates | computed | -2.8% | 0.11821 | 0.11082 | 0.16110 | 0.14736 | -6.3% |
| sort_values | 128 | reversed | cheap | -6.9% | 1.21746 | 1.04494 | 1.44069 | 1.10538 | -14.2% |
| sort_indices | 128 | reversed | cheap | -8.7% | 1.21579 | 1.03922 | 1.31357 | 1.15303 | -14.5% |
| sort_values | 128 | reversed | computed | -4.9% | 2.55494 | 2.33407 | 2.66141 | 2.42686 | -8.6% |
| sort_indices | 128 | reversed | computed | -5.3% | 2.55036 | 2.35381 | 2.71561 | 2.46694 | -7.7% |
| sort_values | 128 | duplicates | cheap | -13.8% | 1.47004 | 1.38073 | 1.55336 | 1.49439 | -6.1% |
| sort_indices | 128 | duplicates | cheap | -13.5% | 1.52941 | 1.32580 | 1.68538 | 1.37411 | -13.3% |
| sort_values | 128 | duplicates | computed | -1.7% | 2.54760 | 2.46444 | 2.70457 | 2.51468 | -3.3% |
| sort_indices | 128 | duplicates | computed | -7.3% | 2.61126 | 2.45218 | 2.68065 | 2.55300 | -6.1% |
| sort_values | 512 | reversed | cheap | -11.4% | 6.08282 | 5.24640 | 6.62640 | 5.79338 | -13.8% |
| sort_indices | 512 | reversed | cheap | -12.7% | 6.51259 | 5.20692 | 6.76521 | 5.72925 | -20.0% |
| sort_values | 512 | reversed | computed | -7.4% | 12.98146 | 11.65910 | 13.62980 | 12.09665 | -10.2% |
| sort_indices | 512 | reversed | computed | -6.7% | 12.85105 | 11.43260 | 13.70345 | 12.29918 | -11.0% |
| sort_values | 512 | duplicates | cheap | -10.8% | 7.79123 | 7.09071 | 8.09900 | 7.28026 | -9.0% |
| sort_indices | 512 | duplicates | cheap | -13.2% | 8.09588 | 6.97742 | 8.66215 | 7.16621 | -13.8% |
| sort_values | 512 | duplicates | computed | -5.7% | 12.51330 | 11.09061 | 12.74299 | 11.51680 | -11.4% |
| sort_indices | 512 | duplicates | computed | -8.9% | 11.95483 | 10.96578 | 12.92850 | 11.48576 | -8.3% |

Cheap-key medians improved in all 12 workloads in both runs: about 7–14% in
the initial run and 5–20% in confirmation. Computed-key improvements were smaller;
one eight-item value workload was 0.6% slower in confirmation. This supports the
compact representation for this retained Linux runtime, not a universal speed
claim. O(n) auxiliary space and O(n log n) comparisons remain unchanged. This
follow-up does not establish that cached sorting beats plain comparator sorting
for every cheap projection.

The scoped source/privacy review covers private-pair exposure, borrowed references,
callback effects and bounds. Only synthetic fixtures are introduced; no network,
filesystem behavior, telemetry, dependencies, compiler or runtime changes are
added to the production library. Cross-platform timing and a full dependency or
runtime security audit remain outside this slice.
