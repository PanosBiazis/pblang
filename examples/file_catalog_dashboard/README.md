# File Catalog Dashboard

This PBLang-only example migrates an earlier Python folder-analysis script into
a deterministic metadata classifier and responsive dashboard. It needs no
NumPy, Matplotlib or foreign runtime package.

```text
pblang run tools/pblock_install.sapp --allow-all -- examples/file_catalog_dashboard
pblang run examples/file_catalog_dashboard/main.sapp
pblang run examples/file_catalog_dashboard/benchmark.sapp
```

`stdlib:file_catalog.sapp` recognizes source, document, data, configuration,
database, image, audio, video, archive, package, executable, binary, font, 3D
model and disk-image families. Specific and multipart suffixes are evaluated
before generic suffixes. Ambiguous `.h` and `.mod` files receive shared labels
instead of a false language claim.

Directory scans reuse the bounded metadata-only `project_inventory` boundary,
inspect one level and read no file content. Reports contain aggregate counts by
default. Basename disclosure requires explicit `include_names: true`; directory
paths are never returned by the catalog summary.

The visualization contract provides accessible rows plus portable bar, donut
and radial-column plans. The decorative 3D Matplotlib chart from the source is
represented as deterministic radial geometry rather than claiming a live 3D
renderer on every platform. One shared UI tree compiles to Web, native desktop
and Android records.

The original source is represented by SHA-256
`5cf64c844e513426f224ebffc284bce8d63e9d7274d98195fe864c24a9824240`
without retaining its personal filesystem path.

Suffix classification does not inspect magic bytes or prove file contents. It
is non-recursive and capped at 4,096 immediate items. Deep content detection,
archive traversal and recursive scans remain explicitly outside this example.

## Portable metadata names and indexed lookup

Classification treats both `/` and `\` as separators on every host. A leading
single ASCII letter plus colon in a separator-free input is a Windows
relative-drive prefix (`C:main.py` becomes `main.py`). Empty basenames, trailing
separators, `.` and `..` are rejected. This deliberately changes Linux handling
of literal backslashes and drive-like prefixes, and rejects formerly normalized
trailing separators, to uphold basename-only disclosure for imported metadata.
It is not filesystem canonicalization or permission checking.

`classify` and `summarize` no longer require filesystem capability for basename
parsing; loading their source modules still uses the separate import capability.
`scan_directory` still needs filesystem access. Classification uses a private
index of ordered final-extension buckets; multipart/special-name precedence and
all existing labels are unchanged. Basename extraction starts with a short
trailing window and doubles it until the final separator is included. Longer
names are retained in full without splitting every preceding directory. A long
name after a single directory prefix is returned without repeated expansion. No foreign runtime or new scanner limit is
introduced. See [validation and measurements](../../docs/FILE_CATALOG_INDEX_VALIDATION_2026-09-16.md).


## Self-hosted aggregation

`summarize` and `visualization_plan` are checked through the self-hosted frontend
as well as the retained host. Missing options receive defaults; explicit invalid
values (including `nil` inside an options map) are rejected. Sorted histogram
rows preserve the retained host order. Directory scanning still uses the host
filesystem boundary and is not established as a self-hosted guest-OS capability.

See [current validation and resource measurements](../../docs/FILE_CATALOG_SUMMARY_VALIDATION_2026-09-17.md).

## Inventory scope in the dashboard

The scope card consumes the existing `scan_directory` fields; rendering never
starts a scan. The default ten-name sample says **Supplied list / sample; no
scan evidence**. `summarize` output alone establishes no filesystem coverage.

A valid report with `scan_scope = "one directory level, metadata only"`, boolean
`inventory_truncated`/`inventory_timed_out` and an integer `scanned_entries`
produces one of these labels:

- One-level scan report; no truncation reported. This is not a promise of
  complete inventory: exclusions, skipped entries and snapshot consistency
  are not established by these four fields.
- Partial scan report; inventory truncated.
- Timed out; partial scan report (with a truncation note when both flags apply).

Missing/invalid fields, unrecognized scope, nonfinite counts and visited-entry
counts below the classified-file count display **coverage unknown**. Unknown
scope strings are not rendered; they may contain private paths. Classification
counters must be nonnegative integers up to 4096 with known + unknown = total;
invalid classification counters reject before rendering. Visited entries may
include directories and are displayed separately from classified files.

The progress bar is explicitly **recognized suffixes among listed files**, not
scan completion, file-content verification or an estimate of unvisited files.
Empty input has an unavailable rate (zero-position bar), not a completion claim.
Family Inspect buttons preserve all scope and interruption indications.
Caller-supplied flags are report metadata, not authenticated scan provenance.

Run `pblang run examples/file_catalog_dashboard/test.sapp` after the local lock
setup above. `scan_fixture.sapp` is a test adapter for a newly created owned
temporary directory with three immediate files and a nested directory. It
checks actual one-level/cap-limited metadata results through the existing
scanner; do not point this fixture at personal volumes or company paths.
Timeout states are synthetic tests, not a claim of reproducing a real timeout.

The retained runtime resolves nested local locks against its process cwd. The
existing `pblock_install` helper materializes this example's isolated local lock.
Before publishing/moving the checkout, restore its portable lock using
`pblang run tools/pblock.sapp --allow-all -- examples/file_catalog_dashboard`.
Do not publish a host-bound lock. The Discovery kit uses its shared root
manifest/lock snapshot and omits this nested manifest/lock.
