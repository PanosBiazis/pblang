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
