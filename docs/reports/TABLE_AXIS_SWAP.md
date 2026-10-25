# Cross-tab axis swapping and same-axis reuse

PBTally's native **Swap axes and calculate** operates on the current axis selectors
in cross-tab mode. It derives the candidate result from the loaded source with
those selectors reversed. Only after that succeeds does it update both selectors,
result, export settings and preview page. Count mode is refused. The source and
its loaded header mode never change; the header selector still applies to the
next Open. Pending axis selections are applied by a successful swap.

Failure leaves selectors, prior result, pagination and export-settings baseline
intact. The limits are asymmetric: up to 31 row categories but only 7 column
categories. A valid 9-by-1 report cannot become a 1-by-9 report. Invalid source
columns and output byte limits also reject. No truncation is introduced.

CLI already supports this operation: use `cross` with the source-column arguments
in reverse order and a new output filename. Exported Web/Android readers display
the resulting table using their existing row selection contract; they do not
edit axes. No file format changed.

`table_group.cross` validates both column indices as before. When the indices
are equal, it now reuses the first in-memory axis index for the second axis.
That index is read-only during result construction and is never returned, cached
or written to the source. Distinct source columns still get independent indices,
even if their labels happen to be identical. Public `group_rows` and `counts`
contracts are unchanged. No persistent cache or additional state is introduced.

Tests compare exact old/new results or rejection over 160 combinations of header
mode, source size, category count and axis choices; they also check record totals,
diagonal zeroes, detached output, malformed data, literal and long labels. Product
tests cover double swap, current selections, count-mode refusal, failed axis swap,
loaded-header semantics, layout bounds and native/Web/Android reader generation.

`bench/table_same_axis.sapp` compares the exact pre-change module in
`bench/fixtures/table_group_before_axis_reuse.sapp`. The workload is 31 records
with 1, 3 or 7 categories, same-axis and distinct-axis cases. Both versions have
identical input/output validation and output assertions. Three warmup batches,
nine alternating-order samples, twenty calls per sample; output is wall-clock
milliseconds per cross-tab call. Median and nearest-rank p95 (max of nine) describe
only that run, not end-to-end application speed or other platforms.
