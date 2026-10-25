# Exact table joins as product workflows

The existing discovery recipes explicitly identified order/course enrichment and inventory lookups as reusable workflows. This slice turns the existing `table_lookup` recipe foundation into PBJoin and an additive PBDataLab CLI command. No new filesystem discovery, external data import or copied vendor implementation was needed. Source inspiration: `docs/DISCOVERY_RECIPES.md` in the active workspace (recipe evidence, not a claim that earlier examples were full products).

## Libraries

- Existing `table_lookup.join` retains its literal many-to-one contract: both tables have headers, nonblank keys, unique reference keys, repeated left keys allowed, explicit zero-based selected columns, left/inner mode. Detached results retain match flags and one-based original data-row positions, including after inner filtering. Unmatched left rows get blank appended fields. Repeated header labels are preserved. Sheet limits apply after expansion.
- New `table_lookup_files.open` reads two bounded local tables and returns the same report; `save` writes a new values-only table; `run` composes open/save and returns counts without echoing cell text. Failures return `ok:false`. Reads/calculations happen before writing. Existing table file semantics apply, including possible partial new files on failure. No atomic or durable-save promise.
- New `table_columns.parse` is the original PBDataLab parser moved unchanged into a reusable library. The product module forwards to it, preserving existing imports and syntax. Distinct one-digit columns 1–8, comma-separated, optional whitespace, maximum 64 UTF-8 bytes; no ranges, decimals or exponents.
- `table_cell_view.create(doc,row=0,title="PBTally table reader")` adds an optional bounded heading. Existing callers retain the same default. All row cells remain available with full text wrapping and long-to-short clearing. PBJoin uses its own heading.

## Product behavior and coverage

PBJoin provides synthetic demo, native two-path join workspace, CLI, new-copy exports and portable value-only row previews. Native result pages show source row/match flags. Saving is blocked while any source path or join setting differs from the last successful result. Invalid operations retain that result. PBDataLab adds the same CLI command and common column parsing; it does not gain a second native join form in this slice.

A source index of 1 means the first data row immediately after row 0 headers. Report totals always describe all left input data rows, including unmatched rows omitted by inner mode. Selecting a reference key column for output is permitted. Exported table files intentionally contain only values; consumers needing match provenance must use the shared report, not infer it from blanks.

## Validation plan and limits

Targeted suites cover existing lookup semantics and budgets, shared selection grammar, portable heading compatibility, prior cell reader and tally behavior, prior PBDataLab workbench/selection/group export, PBJoin model/stale-state behavior and real file/native callback roundtrips. Strict builds/manifests, lint, CLI parity, Linux event-loop capture and Web/Android generation are checked separately. Exact run evidence is in the accompanying task report; generated Android output is not installation evidence. Windows/macOS physical operation is unverified. No measured speedup or use of every PBLang capability is claimed; this integration reuses the existing indexed lookup algorithm.
