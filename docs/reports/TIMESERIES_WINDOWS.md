# Time-series rolling windows

`stdlib:timeseries.sapp` now supplies `rolling_min(data, window)` and
`rolling_max(data, window)`, validates window sizes for the existing rolling
functions, and accelerates provably exact integer sums and means.

## Contract

- `sma`, `rolling_sum`, `rolling_std`, `rolling_min` and `rolling_max` require an
  ordinary finite integer window. Fractional, NaN, infinite and non-number windows
  throw, including with empty input. Previously fractional windows were accepted
  and could produce a mean with a denominator different from the sample count.
- Nonpositive integer windows and windows larger than the input still return an
  empty array. `rolling_std` also preserves its empty result for window 1.
- New extrema accept arrays of at most 65,536 ordinary finite numbers. Oversized
  input is rejected before processing; values are checked when there is a full
  window to return. Empty-result cases do not inspect unused elements.
- Each result has `n-window+1` elements for valid full windows. Output is fresh;
  input is unchanged. Equal extrema retain the earliest value in the window,
  including the sign of zero. Malformed input never returns a partial result.
- Existing numeric data contracts for sums, means and sample standard deviation
  are unchanged. Only window validation is intentionally stricter. No new size
  limit is imposed on existing functions.

## Implementation

Sums/means use a shared implementation. For window >= 8 and at least eight output
windows, it checks that each input is an ordinary finite integer and the sum of
all absolute values is <= 2^53-1. The budget is decremented rather than accumulated,
so the proof cannot round above its boundary. Every intermediate subset sum is
then representable exactly. The outgoing element is subtracted before the incoming
one is added; this preserves the subset-sum bound. This path is O(n), with O(1)
auxiliary storage besides the output.

All other inputs keep left-to-right recomputation per window, O(n*window), including
decimals, larger integers and exact-number types. This preserves existing rounding
and allows later windows to recover after NaN/infinity has left the window. The
eligibility scan adds overhead and may conservatively decline safe local windows.
`rolling_std` is unchanged apart from window validation and remains O(n*window).

Extrema use a monotone queue of source indices. Expired indices are skipped at the
head; dominated indices are removed at the tail. Each index is added/removed at
most once, giving O(n) time. The simple queue retains an expired prefix, so auxiliary
storage is O(n), not O(window); the 65,536-item cap bounds it and the output. Window
1 validates and copies directly. No mutation, callback, external access or recursion
is involved.

## Verification

Five suites passed on the retained PBLang 0.6.0 Linux bootstrap:

```sh
pblang run tests/semantics/timeseries_stdlib_test.sapp --allow-all
pblang run tests/semantics/timeseries_enrich_test.sapp --allow-all
pblang run tests/semantics/indicators_stdlib_test.sapp --allow-all
pblang run tests/semantics/timeseries_windows_test.sapp --allow-all
pblang run tests/semantics/selfhost_timeseries_windows_test.sapp --allow-all
pblang run examples/timeseries_windows.sapp --allow-all
```

The new regression suite covers 810 size/window/data-pattern combinations, with
full exact old-version parity for sums/means and an independent naive oracle for
extrema. Additional cases cover the exactness budget, cancellation, decimal fallback,
NaN recovery, malformed/empty input, signed zeros, fresh results, and accepted/rejected
input limits. The former fractional-window result is explicitly reproduced with
the unchanged fixture. The self-hosted test exercises both the fast integer path
and the new extrema in the bounded VM. Existing downstream indicator tests pass.

## Reproducible local measurements

```sh
pblang run bench/timeseries_windows.sapp --allow-all
```

The old fixture is the module at base commit `c7f99c5efd46b24dd1e27ca1db3e192d6b12e420`.
Two warmups and seven alternating-order samples are used, with one full call per
sample and full output parity before timing. Results include every raw sample,
median and interpolated p95 in `bench/timeseries_windows_results.json`.

For new extrema, the baseline is a naive scan, not a previously released API; it
does not perform the new bounded-input validation. Performance is for these finite
synthetic workloads only. No speed claim is made for all libraries or platforms.

| Operation | Data | n | Window | Before median ms | After median ms | Before p95 ms | After p95 ms | Time change |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| rolling_sum | integer | 128 | 1 | 0.13232 | 0.14591 | 0.14105 | 0.15616 | +10.3% |
| sma | integer | 128 | 1 | 0.15020 | 0.16308 | 0.15175 | 0.17011 | +8.6% |
| rolling_min_vs_naive | integer | 128 | 1 | 0.14305 | 0.19717 | 0.14482 | 0.20571 | +37.8% |
| rolling_max_vs_naive | integer | 128 | 1 | 0.14377 | 0.20194 | 0.15159 | 0.20363 | +40.5% |
| rolling_sum | integer | 128 | 8 | 0.42176 | 0.34976 | 0.44796 | 0.43943 | -17.1% |
| sma | integer | 128 | 8 | 0.45967 | 0.42391 | 0.46923 | 0.52660 | -7.8% |
| rolling_min_vs_naive | integer | 128 | 8 | 0.61393 | 0.66352 | 0.62203 | 0.73152 | +8.1% |
| rolling_max_vs_naive | integer | 128 | 8 | 0.51951 | 0.64850 | 0.64421 | 0.69661 | +24.8% |
| rolling_sum | integer | 128 | 64 | 1.10316 | 0.22769 | 1.42343 | 0.36309 | -79.4% |
| sma | integer | 128 | 64 | 1.12057 | 0.19670 | 1.19348 | 0.29783 | -82.4% |
| rolling_min_vs_naive | integer | 128 | 64 | 2.22921 | 0.69165 | 2.29828 | 0.71180 | -69.0% |
| rolling_max_vs_naive | integer | 128 | 64 | 2.06399 | 0.58842 | 2.26855 | 0.64926 | -71.5% |
| rolling_sum | decimal | 128 | 1 | 0.12279 | 0.13208 | 0.14198 | 0.14372 | +7.6% |
| sma | decimal | 128 | 1 | 0.14925 | 0.15020 | 0.16828 | 0.16079 | +0.6% |
| rolling_sum | decimal | 128 | 8 | 0.38004 | 0.42439 | 0.44487 | 0.46542 | +11.7% |
| sma | decimal | 128 | 8 | 0.41556 | 0.41318 | 0.44837 | 0.46847 | -0.6% |
| rolling_sum | decimal | 128 | 64 | 1.35517 | 1.42980 | 1.40240 | 1.49360 | +5.5% |
| sma | decimal | 128 | 64 | 1.38783 | 1.46794 | 1.50352 | 1.54667 | +5.8% |
| rolling_sum | integer | 1024 | 1 | 1.07646 | 1.17660 | 1.12574 | 1.46987 | +9.3% |
| sma | integer | 1024 | 1 | 0.80395 | 0.85640 | 1.10340 | 1.01199 | +6.5% |
| rolling_min_vs_naive | integer | 1024 | 1 | 0.93555 | 1.13368 | 1.00651 | 1.37842 | +21.2% |
| rolling_max_vs_naive | integer | 1024 | 1 | 0.71645 | 0.88334 | 0.98369 | 1.10950 | +23.3% |
| rolling_sum | integer | 1024 | 8 | 3.80254 | 3.11732 | 4.28352 | 3.63896 | -18.0% |
| sma | integer | 1024 | 8 | 3.75414 | 3.69644 | 4.01433 | 3.81663 | -1.5% |
| rolling_min_vs_naive | integer | 1024 | 8 | 5.22470 | 5.53179 | 5.32391 | 5.78370 | +5.9% |
| rolling_max_vs_naive | integer | 1024 | 8 | 5.31125 | 5.66292 | 5.50387 | 5.74827 | +6.6% |
| rolling_sum | integer | 1024 | 64 | 23.09680 | 2.99788 | 25.41862 | 3.53460 | -87.0% |
| sma | integer | 1024 | 64 | 25.93088 | 3.15833 | 26.57712 | 3.93171 | -87.8% |
| rolling_min_vs_naive | integer | 1024 | 64 | 33.33831 | 5.63526 | 35.02517 | 6.04980 | -83.1% |
| rolling_max_vs_naive | integer | 1024 | 64 | 35.56204 | 5.98311 | 37.93516 | 6.89447 | -83.2% |
| rolling_sum | decimal | 1024 | 1 | 0.73934 | 0.80633 | 1.32239 | 1.30517 | +9.1% |
| sma | decimal | 1024 | 1 | 0.82541 | 0.83113 | 1.00367 | 1.10729 | +0.7% |
| rolling_sum | decimal | 1024 | 8 | 2.71344 | 2.71630 | 3.64580 | 3.50368 | +0.1% |
| sma | decimal | 1024 | 8 | 2.53916 | 2.57945 | 3.37696 | 3.51193 | +1.6% |
| rolling_sum | decimal | 1024 | 64 | 14.92524 | 15.18798 | 17.83473 | 17.14902 | +1.8% |
| sma | decimal | 1024 | 64 | 15.32030 | 15.31267 | 16.74602 | 17.69273 | -0.0% |

At window 64, integer sums/means used about 79–88% less time, and extrema about
69–83% less than the naive baseline. Small windows and decimal fallback can be
slower: existing operations were up to about 12% slower; new extrema with window
1 were about 21–41% slower than the unchecked naive baseline. Report these costs
alongside the larger-window improvements.

## Scope and limits

The scoped review covered the owning module, direct indicator callers, published
files, bounded allocation, numerical equivalence and failure behavior. No network,
filesystem, telemetry, credential, dependency or CI capability was added. Tests
use synthetic inputs. No dependency advisory scan, production probe, exhaustive
floating-point proof for unchanged functions, or cross-platform execution was run.
Other time-series functions, asynchronous execution and the rest of the library
upgrade remain separate work.
