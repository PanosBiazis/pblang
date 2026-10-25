# Moving means and cancellation

`stdlib:sample_series.sapp` keeps its existing `moving_mean(values,window)` API:
1–4096 finite samples within ±1,000,000,000, integer width in 1..N, N-width+1
complete trailing means, `first_source_index=width-1`, detached output and
`pblang.moving-mean.v1` metadata. Inputs and accepted ranges are unchanged.

The first complete window is built bottom-up once. Later identical finite replacements
reuse the existing root; changed samples still recompute exactly the same ancestors
and use the same expansion merge. Every complete window still emits one detached
result, including repeated values and non-power-of-two widths. Width-one identity
is unchanged.

Each window is represented by a bounded binary tree of nonoverlapping floating-point
expansions. The next sample replaces the expired sample at a ring-buffer leaf; only
its ancestors are recomputed from their current children. The previous
add-new/subtract-old rolling sum lost small contributions beside large samples,
and that lost contribution persisted even after the large sample expired. For
example `[1000000000,0.000000001]` at width one returned a zero second value;
width two on `[1000000000,0.000000001,0.000000001]` also lost the final tiny mean.
Replacing leaves removes cross-window contamination without subtracting a rounded
expired contribution, while retained residual components also preserve
the small residual of `[1000000000,0.000000001,-1000000000]` within one window.

Width one copies each stored binary64 sample, including signed zero and
subnormals. Wider means remain binary64 computations: compensation and the
final division can round, and division may underflow. Exact decimal arithmetic,
universally correctly rounded results and preservation of subnormal means below
the representable range are not promised. No restrictions were added to hide
these numerical limitations. Interpolation is unchanged.

For widths above one there are at most size-1 initial node recomputations,
where size is the next power of two, then at most ceil(log2(width)) per changed
sample. The previous conservative bound of 49,152 at N=4096 still holds. Merging a node grows a nonoverlapping expansion with
error-free FastTwoSum additions; a discarded rounding residual is retained as
another component. With K components in a node, this simple merge costs O(K²),
so total work is O(N*ceil(log2(width))*K²), not a constant-cost tree update.
K is bounded by the number of samples contributing to that node and never
exceeds width. An unexpected component count beyond width throws a fixed
resource-limit error rather than silently dropping precision or samples.

The tree contains at most 8,192 array slots. Across its levels the stored
numeric components total at most width*(ceil(log2(width))+1), or 53,248,
plus the bounded temporary merge array and N-width+1 result slots. These are
logical data bounds, not measured allocator/GC memory limits. Width one is
O(N) and allocates no tree. PBSmooth retains its 31-sample limit: at most 155
node recomputations and 186 stored components. No universal speedup over the
original rolling sum is claimed. The implementation avoids recomputing every
complete window and preserves more than two scales through cancellation;
final projection to binary64 and division still round.

`sample_series_numerical_test.sapp` checks signed and scaled cancellations,
bitwise width-one identity, detached results, maximum sample/full-window bounds,
three-scale expired-contribution regression, and 1,984 output means against an
independent exact fixed-point reference.
`smoothing_table_test.sapp` checks that the tiny trailing value survives the
product-facing table serialization. These are scoped regression cases, not a
proof of numerical accuracy for every binary64 input.

## Repeated samples and initial-window cost

Run `pblang run examples/sample_series_repeated.sapp --allow-import` for a
bounded repeated cancellation example. The optimization adds no dependencies,
I/O, approximation or new accepted input range. The baseline and candidate
merge primitive are identical; only redundant ancestor recomputations are skipped.

A paired local check (2 warmups, 11 measured fresh processes per variant, alternating
order, identical CLI/flags/inputs) compared the previous expansion implementation
with this version. Median whole-process wall time was 0.1000 → 0.0223 s for
4096 alternating ±1e9 samples/width 2048, 0.2386 → 0.1541 s for nonperiodic
fixed-point samples at the same size/width, and 0.2204 → 0.0398 s for a full
4096-sample window. The 31-sample/width-7 repeated workload was 0.0747 →
0.0697 s. Width-one identity was 0.01330 → 0.01354 s (mixed small-process
variation, not an improvement). These are scoped shared-host measurements,
not universal speed or memory guarantees and not a comparison with the older
inaccurate rolling-sum algorithm. All 4,984 checked means in the independent
Fraction reference corpus remained within the prior two-ULP test tolerance
(maximum observed one ULP); exact fixed-point regressions remain exact.
