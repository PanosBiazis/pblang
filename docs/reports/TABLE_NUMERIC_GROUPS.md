# Exact decimal groups and PBGroupStats

The portfolio roadmap identifies numeric grouping as the next step after category
counts. `table_group_stats.summarize(document, category, value, header=false)`
reuses `table_group.group_rows`, the existing exact decimal runtime and Sheet
validation. PBGroupStats supplies a focused native/CLI workflow; PBTally exposes
identical semantics through its additional numeric mode. Existing statistics APIs
and count/cross-tab behavior are unchanged.

The exported `parse_value` validates bounded literal syntax before decimal
construction: optional minus, 1–12 integer digits, optional dot plus 1–6 fractional
digits. No whitespace, exponent, plus, separator or implicit conversion. All
source numeric cells must parse; no partial result escapes. Arithmetic is exact
base10, and output strings use decimal normalization. Negative and zero values
participate normally. Source labels are literal and stable. Each group is traversed
once, with no persistent cache and no measured speedup claim.

Output columns are Group/Count/Sum/Min/Max. Header-only input returns the header.
At most31 groups fit with an output header. The full32×8 Sheet,256-byte-cell and
8192-total-byte budgets apply, including headings and derived values. Header and
column selection are explicit. Results detach from the input. Use compatible
units; no unit/currency inference, mean/rounding policy, totals or synchronization.

`table_stats_view` provides a shared native workspace plus the existing portable
complete selected-row reader. Source files use `table_files`: bounded no-follow
reads, exclusive new writes, generic path-free errors. Pending numeric selectors
block stale export. Failed Open/Calculate retain previous state. The input field
names the next Open; Calculate uses the loaded document. No network or telemetry.
Exports contain original category text and numeric aggregates. Portable cell
selection does not hide embedded values. Third-party CSV formula interpretation
is outside the literal PBLang model. Failed writes may leave partial new copies.

Verification: semantic positive/negative cases, exact32-row large sums, malformed
values/columns, output count/byte limits, source independence; PBTally numeric
integration and existing modes; PBGroupStats app/file suites. Linux native and
Web interaction evidence are separate from Android generation. Windows/macOS,
installed Android, signed releases and large streaming tables are unverified.
