# Bend-Inspired Features In pblang

pblang now has a Bend-inspired standard-library layer in
`stdlib/bend.sapp`.

This is not a copy of Bend or HVM. It ports the practical programming model:
write pure tree-shaped work, split it into independent branches, and let the
runtime evaluate those branches concurrently.

## Added APIs

- `fork_pair(left_fn, right_fn)`
- `bend_tree(...)`
- `bend_tree_parallel(...)`
- `fold_tree(...)`
- `fold_tree_parallel(...)`
- `map_tree(...)`
- `tree_sum(...)`
- `tree_count(...)`
- `divide_reduce(...)`
- `divide_reduce_parallel(...)`
- `range_sum(...)`
- `range_sum_parallel(...)`
- `parallel_map(...)`
- `parallel_filter(...)`
- `divide_sort(...)`
- `runtime_model()`

## Why This Shape

Bend exposes massive parallelism through pure recursive, tree-shaped programs.
pblang already has `spawn`, `join`, channels, atomics, locks, async helpers, and
a transparent JIT path. The safest first step is therefore a library API that
uses those primitives with explicit `parallel_depth` and `grain` controls.

That gives pblang users the useful Bend-style pattern now, while leaving deeper
compiler work for measured follow-up tasks:

- IR pass for recognizing divide-and-conquer recursion.
- Work-stealing scheduler optimization.
- Optional HVM/interaction-net backend experiment.
- GPU backend only after IR and benchmark evidence.

## Example

```pblang
let bend = import("stdlib:bend.sapp");

let total = bend.range_sum_parallel(1, 100001, 4);
print total;

let sorted = bend.divide_sort([5, 1, 4, 2, 3], fn(a, b) { return a < b; }, 2);
print sorted;
```

## Verification

```sh
pblang run tests/semantics/bend_stdlib_test.sapp --allow-all
```
