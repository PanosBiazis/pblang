# Local code-map refresh

The installed Graphify AST extractor does not include `.sapp` in its supported
extensions. A successful Graphify command is therefore not evidence that PBLang
source relationships are current. Semantic/LLM extraction was not run.

For the repository's supported code and document formats, run locally:

```sh
GRAPHIFY_MAX_WORKERS=2 graphify update . --no-cluster
```

This skips community detection and visualization. It keeps AST extraction local
and does not require an API key. The command must finish successfully before its
output is treated as refreshed; elapsed time alone is not completion. The
September 15 isolated run completed in 45.93 seconds, extracting 511 files into
8,677 nodes and 12,802 links. It did not replace the aggregate AAP graph. This
is a repository-scoped structural snapshot, without a clustering-freshness claim.

For the PBLang compiler modules, the existing native tool can create a separate
view directly from `.sapp` sources:

```sh
pblang run tools/pbcodegraph.sapp stdlib/compiler --out graphify-out/pblang-compiler
```

Keep the views separate: the native tool is a lexical structure/import scanner,
not a compiler-resolved dependency graph or a security analyzer. Its module view
uses filename stems, so equal basenames from different directories can merge.
Its general directory walker currently stops at 5,000 files; do not use its output
as proof of a complete larger project scan. The compiler directory contains only
20 `.sapp` modules with distinct stems. Imported modules outside that directory
are references, not analyzed source. No full-project freshness claim follows.

Generated maps stay local under ignored `graphify-out/`; they are not release
artifacts. Check coverage and paths before sharing them. Improvements still needed
include native `.sapp` integration in Graphify, explicit truncation reporting,
path-based module identities, incremental source-hash invalidation and verified
full-project coverage. A faster refresh does not fix those correctness gaps.
