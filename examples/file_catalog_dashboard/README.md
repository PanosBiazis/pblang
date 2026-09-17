# File Catalog Dashboard

This PBLang-only example migrates an earlier Python folder-analysis script into
a deterministic metadata classifier and responsive dashboard. It needs no
NumPy, Matplotlib or foreign runtime package.

```text
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
