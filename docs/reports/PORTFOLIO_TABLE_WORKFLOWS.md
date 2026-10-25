# Literal table workflows

PBDataLab now has a native table workbench; PBReconcile is a new, local PBLang-only product for comparing table versions. Both reuse the existing Sheet and CSV contracts rather than introducing separate data models. This is a bounded alpha, not a general spreadsheet engine or database merger.

## Shared libraries

- `sheet_csv.encode(document, delimiter=",")`: validates the existing Sheet model, always quotes fields, doubles quotes and uses CRLF records. CSV, semicolon CSV and TSV roundtrip through `decode`, including embedded CR/LF, Unicode, empty cells and a data BOM. Output stays below the existing 64 KiB transport limit. Literal formulas remain literal in PBLang; other spreadsheet programs may interpret them on import. Export does not alter cell values to accommodate another application's formula policy.
- `table_transform.trim_cells(document, header=false)`: detached copy with the runtime's `str_trim` applied to data cells only. No case folding, numeric conversion or Unicode normalization.
- `table_transform.deduplicate(document, header=false)`: stable first complete-row occurrence; exact string equality through unambiguous JSON row keys. A header is excluded from duplicate detection.
- `table_transform.filter_rows(document, column, query, header=false)`: zero-based column, case-sensitive literal substring, empty query retains all data. A header-only result is allowed. A headerless zero-row result is rejected because the Sheet model requires at least one row; no blank sentinel row is invented.
- `table_transform.apply(document, steps, header=false)`: detached transaction, 0–16 steps. Exact schemas are `{op:"trim"}`, `{op:"deduplicate"}` and `{op:"filter",column,query}`. Invalid steps reject without modifying the input, even if preceding steps succeeded.
- `table_compare.compare(before, after, column=0, header=false)`: equal column count, matching literal headers when enabled, unique nonblank keys in each table. Keys are compared exactly: `001` and `1` differ, as do case and surrounding spaces. Whitespace-only keys are rejected. Row movement is ignored. A changed key becomes a removal plus an addition. All non-key cells are compared literally; no fuzzy pairing, merge or conflict resolution occurs.
- `table_files.open(path, delimiter=",")` and `save(path, document, kind="csv", delimiter=",")`: shared local file adapters. `.pbsheet` uses strict native decoding; other inputs use explicit delimited decoding. CSV export requires `.csv`, tab export `.tsv`, native export `.pbsheet`. Save uses new-file exclusive creation and never overwrites or retries via a less strict writer. It does not promise atomic visibility or durability; failed copies may be incomplete. Fixed errors do not include input contents or paths. Trusted local CLI file commands on the retained runtime require `--allow-all`.

The inherited bounds are 32 rows including an optional header, 8 columns, 256 UTF-8 bytes per cell, 8192 cell bytes total and 64 KiB input. Comparison produces at most 64 differences. Header-only inputs have zero data rows. Changes follow before-table order, then additions follow after-table order; `columns` lists zero-based changed cell indices only for changed rows. Added/removed rows have an empty `columns` list and one null side. Reports include detached source values and `contains_cell_text:true,safe_to_log:false`. Aggregate snapshots omit keys and row values, but totals may still be private.

## Product behavior and editions

PBDataLab's CLI `clean` performs trim followed by complete-row deduplication. Its native workbench also offers literal filtering, eight undo states, read-only table preview, explicit input header mode, new-copy CSV/TSV/Sheets exports and retention of unsaved results on close. Changing the header selector applies only to the next successful open. Export delimiter uses the current selector. Opening another table is blocked while results are unsaved; undo or save first. The existing profiler and aggregate snapshot functions remain available.

PBReconcile supplies a native comparison workspace and CLI, with every difference accessible using previous/next controls. Failure retains the previous result. Native report export creates a new JSON file containing keys and row values. There is no imported report parser or report-to-table mutation. The source tables are never changed.

Portable Web and Android views currently show aggregate results only. They do not import files, execute transformations or comparisons, save results or provide native workbench parity. Generated HTML and JVM classes are evidence of renderer generation, not an installed mobile release. Physical Windows/macOS/mobile runtime operation, signed packages, large streaming tables and typed schemas remain unverified/unimplemented.

## Idea provenance and tests

This slice uses the existing `PORTFOLIO_FOUNDATIONS.md` data-analysis direction, `PRODUCT_PORTFOLIO_ROADMAP.md` selected transformations/export backlog and `IDEA_INTAKE_CATALOG_2026-09-17.md` lesson of separating data processing from presentation. PBReconcile is a newly authored composition for comparing inventory/task/catalog snapshots; no additional disk scan, private-file ingestion or training was performed. Existing scan coverage remains partial.

Focused suites cover exact Unicode/quoted transport, maximum dimensions/quote expansion, malformed steps, failure without mutation, stable deduplication, missing/duplicate keys, row reorder, all change categories, maximum disjoint reports, GUI undo/close/error state and PBOffice native-sheet interchange. Comparison indexes each table by key and does bounded per-cell equality work; no measured speedup over another implementation is claimed.

## Explicit ordering and header alignment

`table_transform.select_columns(doc, columns)` returns a detached table with
1–8 unique, valid zero-based columns in requested order, including any header.
`sort_rows(doc, column, descending=false, header=false)` performs a stable
literal sort using the existing bottom-up merge-sort helper. Equal keys retain
source order in either direction. Empty strings sort before nonempty values in
ascending order; there is no locale, numeric parsing, formula evaluation or
implicit trim. Header-only documents stay unchanged. A header is kept first.

`apply` additionally accepts exact `{op:"select",columns:[...]}` and
`{op:"sort",column:0,descending:false}` steps. Indices always refer to the current
intermediate table. A failed later step leaves the caller's input unchanged.
Deduplication/filtering now return rows already copied into their validated
private source rather than copying and validating that subset a second time.
The detached-output contract remains unchanged; performance evidence must use
the identical workload and is not a larger-dataset or platform guarantee.

`table_compare.compare_named(before, after, key_name)` opts into exact unique
header matching. Both header sets must be equal and all headers nonblank. It
reorders a detached after-table with `select_columns`, then reuses the existing
keyed comparison. The report format is `pblang.table-comparison.named.v1`, with
`column_order:"before"` and `after_source_columns` (zero-based original after
indices). All row values and changed column indices are in before order. The
original positional `compare` and its v1 format are unchanged.

PBDataLab exposes sorting through native controls and a `sort` CLI. PBReconcile
exposes optional alignment through native controls and `compare-aligned` CLI.
Their shared Web/Android snapshots still do not execute these operations.
