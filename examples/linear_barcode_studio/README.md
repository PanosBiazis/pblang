# Linear Barcode Studio

This PBLang-only example replaces a small Python barcode script and its external
image dependency with a bounded reusable library. It implements Code 128 Set B,
EAN-13 and UPC-A validation, check-digit generation, module encoding, portable
bar geometry, local SVG, bounded PNG and a text preview.

The source accepted arbitrary data, delegated correctness to an installed
package, overwrote a fixed output basename and printed a misleading duplicated
`.png` suffix. The migration validates inputs before rendering and performs no
default file write. Its shared UI compiles to Web, native desktop and Android
records without remote assets or trackers.

```text
pblang run examples/linear_barcode_studio/main.sapp
pblang run examples/linear_barcode_studio/benchmark.sapp
```

Code 128 support is deliberately limited to Set B printable ASCII. GS1-128,
FNC1, Code Sets A/C, scanner certification, assigned GS1 identifiers and print
quality verification are not claimed. Do not encode passwords, tokens or other
secrets: a barcode is a machine-readable representation of its payload.
