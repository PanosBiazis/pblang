# Literal table grouping and PBTally

This slice combines the small-table/category-analysis direction in the existing `vehicle_emissions_lab` and `file_catalog_dashboard` migration inventory with a new reusable cross-tab workflow. The original product idea is a local category worksheet shared by PBTally and PBDataLab. No new private-data scan, model training or imported third-party implementation was performed.

## Shared semantics

`table_group.group_rows(doc,column,header=false)` validates a Sheet, returns exact labels, detached original row-index groups and per-record membership. Columns are zero-based. Labels keep first appearance order. Empty strings and all literal forms remain categories; no normalization, numeric coercion or formula evaluation.

`counts` returns Group/Count rows; `cross` returns a header of Row / Column plus exact column categories and one row per exact row category. Every source record increments one pair. Missing pairs are zero. Axes can be equal, yielding a diagonal table. No totals row or numeric-sum interpretation is implied. Header mode must be boolean and is never guessed. Invalid input/output fails before file writes.

Sheet budgets remain 32×8, 256 bytes per cell and 8192 total cell bytes. Thus count output supports 31 categories; cross-tab supports 31×7 categories. Output byte validation is independent of source validity. Header-only source yields an empty derived header; blank data remains a real category. No silent truncation or source mutation. Internally each axis is indexed once; counting traverses source memberships once, then emits the bounded output grid.

`table_cell_view.create(doc,row=0)` makes a portable reader for one exact row, with all up-to-eight cells selectable. Text wrapping preserves the existing display cluster model and clears unused detail lines when switching from long to short cells. Values are embedded in generated HTML/JVM data. It is not a full browser/mobile table editor.

## Product integration

PBTally provides native/CLI category worksheets with explicit source axes, header/delimiter choice, complete result text pagination and exclusive new-copy CSV/TSV/Sheets exports. Changed native axes/mode require Calculate before export. Failure retains the previous source/result. PBDataLab adds a derived count export, preserving source table, undo history and dirty baseline. Existing transformations and table profiling retain their semantics.

New-copy writes may leave a partial new file on failure; they do not promise atomic visibility/crash durability. Product CLIs expose structured status and callers must inspect `ok`. Native fixed dimensions: PBTally 1100×1050; PBDataLab retains 1100×1000. Linux checks, live Web reader checks, generated Android classes and installed-device evidence must be distinguished. Windows/macOS/mobile installation remains unverified.

## Validation and measurement

Nine focused/regression suites: table groups, table-cell view, existing table profile/transform; PBDataLab workbench/column-selection/group export; PBTally app/store. Cases include exact Greek labels, blanks/spaces, malformed axes, stable order, header-only input, same-axis diagonals, output limits, 31×7 grids, conservation of record count, detached state, long-cell clearing, stale settings, dirty-state preservation and real file round-trips. Builds/manifests pass strict checks; lint passes with warnings.

`bench/table_cross.sapp` compares the real indexed cross-tab with a reference that resolves groups and scans all records again for every category pair. Both validate input/output and assert equal results. This reference is not a previously released PBLang implementation. Workload: 31 records, 31 row categories, 7 column categories, two warmups, seven wall-clock samples. Median 5.836 → 1.327 ms (4.40×); p95 7.344 → 1.700 ms (nearest-rank, maximum of seven). Concurrent host load was present. The result describes this bounded operation, not general application/platform performance.
