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
input-preservation checks, one cold run, two warmups and five samples. Run with
`PB_SORT_MODE=legacy` or `PB_SORT_MODE=stable` under an external process limit.
Comparator counting is a separate untimed execution. The local report retains
baseline and post-change data; it is not evidence of universal speed superiority.
