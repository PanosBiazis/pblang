# Owned table pipelines and product column selection

`table_transform.apply(document, steps, header=false)` preserves its public
contract: 0–16 exact-schema operations, a detached output, literal strings,
explicit header handling, stable sorting and no caller mutation on failure.

Previously every operation copied and validated the intermediate table, in
addition to the initial copy. The pipeline now validates and copies the input
once, then passes its private intermediate rows to unexported helpers. Trimming
only removes characters, sorting and deduplication only rearrange/select rows,
and column projection creates new row arrays. These operations preserve the
validated Sheet size/UTF-8 limits. Filtering still uses the existing profile
validator, including its column/query checks. Nothing bypasses input validation.
Standalone exported transforms still detach their outputs from the input.

No intermediate state is returned on failure. The empty pipeline also returns
a detached copy. Aliased input row arrays become distinct output row arrays;
header and data mutations on the output cannot reach the caller's document.

PBDataLab now exposes the existing `select_columns` function through its native
workbench and `select` CLI. The shared product parser accepts 1–8 distinct,
one-based numbers separated by commas within 64 UTF-8 bytes. Whitespace around
numbers is allowed, ranges and numeric coercions are not. Indices are checked
against the current table by the shared transform. Header strings and data are
reordered together. GUI failures retain history/data, identity selections do
not consume Undo, and dropping a column remains unsaved until native export or
Undo. Native files roundtrip through PBOffice's existing Sheet reader.

## Verification

- Existing `table_transform_test.sapp`: standalone and pipeline behavior.
- `table_pipeline_ownership_test.sapp`: combined operations, post-mutation
  failures, detached empty pipelines, repeated operations, aliased rows,
  header-only input, malformed tables and maximum UTF-8 input.
- PBDataLab `column_selection_test.sapp`: parser, GUI button routing, controls
  inside the window, Undo, no-op history, invalid current columns and close state.
- PBDataLab `column_interchange_test.sapp`: CSV to selected PBOffice document,
  TSV roundtrip, literal formulas/leading zeroes and original-file retention.

Reproduce the performance comparison with
`pblang run bench/table_pipeline.sapp --allow-all`. It imports the exact
pre-change implementation from `bench/fixtures/table_transform_before_pipeline.sapp`,
uses a 32x8 synthetic table, compares outputs before timing, performs three
warmup batches, nine alternating samples and twenty calls per sample, and emits
wall-clock milliseconds per call. Report median and nearest-rank p95 (the
maximum with nine samples), including regressions if present. Measurements do
not establish larger-table, memory/RSS, isolated CPU-time or cross-platform guarantees.

The Sheet limits remain 32 rows, 8 columns and 8192 total UTF-8 cell bytes.
Web/Android views are still aggregate snapshots and have no projection editor.
