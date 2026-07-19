# Serialization

`stdlib:serialization.sapp` is the shared format facade. It delegates CSV to
`stdlib:csv.sapp` and JSON pretty printing to `stdlib:json.sapp`; it does not
duplicate those implementations.

## Untrusted input

Use the structured APIs for files, network input, MCP arguments, and user data:

```sapp
let ser = import("stdlib:serialization.sapp");
let result = ser.parse_safe(text, "yaml", map{
    max_input_bytes: 1048576,
    max_output_bytes: 2097152,
    max_lines: 100000,
    max_line_bytes: 65536,
    max_depth: 64,
    max_items: 100000,
});
if not result.ok { print result.diagnostics[0].code + ": " + result.error; }
```

`parse_safe`, `stringify_safe`, and `convert_safe` return
`pblang.serialization.result.v1` maps with `ok`, `format`, `value`, `error`,
and `diagnostics`. Caller limits are capped at hard ceilings (16 MiB input,
32 MiB output, one million lines/items, 1 MiB per line/scalar, depth 256), so
an accidental option cannot create an unbounded operation.

`validate(text, format, options)` and the JSON validation helpers use this
bounded path. Unknown formats fail closed. `detect_format` is a convenience
heuristic, not a content-type security decision; pass an explicit format at
trust boundaries.

## Format boundaries

| Format | Supported safe behavior |
|---|---|
| JSON | Strict builtin parse/stringify plus pre-parse depth/item/size guards |
| TOML | Tables/dotted table names with plain keys, quoted strings, numbers, booleans, and flat scalar arrays |
| YAML | Indented maps/sequences with plain keys, quoted/plain scalars, booleans/null/numbers, and flat flow arrays |
| CSV | Quote-aware `csv.sapp` map parsing/stringifying under facade limits |
| XML | Entity escape/unescape and text passthrough only; no DOM, DTD, entity expansion, or schema claim |
| text | Bounded passthrough/string conversion |

The strict TOML/YAML paths reject duplicate keys, malformed syntax, ambiguous
indentation, and constructs the native subset does not implement. In
particular, TOML arrays of tables and inline tables, and YAML anchors, aliases,
tags, merge keys, block scalars, flow maps, and multi-document markers are not
silently approximated. Use a dedicated future standards-complete parser when
those features are required.

## Compatibility

`toml_parse`, `yaml_parse`, `parse_format`, and their stringify counterparts
remain available. Direct TOML/YAML parsers preserve the older lenient subset
for trusted data but now enforce absolute input, line, recursion, and output
ceilings. `parse_safe(..., map{ strict: false })` provides the same compatibility
mode while retaining configurable resource limits.

`convert` now uses strict bounded conversion and throws on a structured failure.
Code that must report errors without exceptions should use `convert_safe`.

## Verification

```bash
./target/release/pblang tests/semantics/serialization_hardening_test.sapp
./target/release/pblang tests/semantics/serialization_enrich_test.sapp
./target/release/pblang examples/safe_serialization/main.sapp
./target/release/pblang bench/serialization_pipeline_stages.sapp
```

The benchmark has checksums for every phase. Run it in several fresh processes
and report median/p95 plus process elapsed time/RSS; do not compare a strict
double-validation path directly with an unchecked parser as if they had the
same security contract.

The 2026-07-18 canonical regression after this slice passed 683/683 semantic
tests with zero failures, ignored tests, or filtered tests. Reproducible local
benchmark evidence is recorded in
`bench/results/2026-07-18-serialization/REPORT.md`.
