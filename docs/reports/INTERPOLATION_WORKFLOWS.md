# Piecewise linear sample table workflows

Based on the `sample_series.interpolate` recipe in `DISCOVERY_RECIPES.md`, this slice creates a usable file workflow shared by PBDataLab and the new PBInterpolate product. No discovered private data or vendor source is copied.

- `interpolation_table.create(samples,queries)` returns a bounded detached result document and sample/query counts.
- `interpolation_files.open`, `save`, `run` provide explicit local table inputs and new-copy output.
- `interpolation_view.create(samples,queries)` displays all results, four per portable page, through existing modern_ui actions.
- `sample_series.interpolate` now preserves exact stored knot values, including tiny terminal values next to large samples. Moving means are unchanged.

Samples require exactly `X,Value` headers and 2–31 data rows. Positions must be strictly increasing; duplicate or descending positions reject rather than sort. Queries require exactly `Query` and 1–31 rows, all within the closed sample range. Query order, duplicates and original query spelling are preserved in the detached `Query,Value` result. Both source tables remain unchanged. CSV, explicit semicolon/tab delimited input and literal `.pbsheet` tables reuse the existing bounded table adapters. The same selected delimiter applies to both text inputs and text output.

All numeric values use strict ASCII decimal/exponent spelling, at most 64 characters, with no whitespace, suffix, formula, nonfinite name or nonzero underflow. Samples and positions stay within +/-1e9. Existing Sheet limits also apply: 32 rows, 8 columns, 256 UTF-8 bytes per cell and 8192 total. Headers are case-sensitive. No dates, timestamps, units, sensor access or calibration are inferred. The caller must use consistent coordinates and value units.

Interpolation is piecewise linear, using the existing binary-search segment lookup. Exact sample positions return their stored values directly. The previous endpoint expression could lose a small terminal value by cancellation: `[0,1]`, `[1e9,1e-9]`, query `1` returned `0`; it now returns `1e-9`. Other interior calculations retain the existing floating-point formula. This is a correctness fix, not an exact-decimal or measured-speedup claim. No extrapolation, curve fitting or uncertainty bounds are implemented.

Native PBInterpolate supports explicit input paths, delimiter choice, calculation, pages and saving. Both empty input paths use the documented synthetic example. Failed calculation retains the visibly marked previous report and disables saving; edited input settings also require recalculation. An already calculated result is a snapshot: external file changes are not watched, and the caller must Calculate again to reread them.

New output filenames are required. Existing files and symlinks are rejected by the shared adapters. Failed output attempts can leave an incomplete new file; atomic visibility/crash durability is not promised. Generic failure text omits raw contents and paths. Web exports explicitly include numerical results, replace generated build output, and retain page navigation only. The caller controls report retention/deletion.

Validation includes the endpoint regression, 2400 independent affine oracle values, non-affine segments, shuffled/repeated queries, malformed input, both source preservation, overwrite/symlink rejection, shared CLI parity and native/portable views. Linux is the local runtime target; Web interaction and Android generation are separately checked. Windows/macOS execution and installed Android are unverified.
