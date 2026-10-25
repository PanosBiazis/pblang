# Idea intake: cache reads and local reuse — 2026-09-15

This is a bounded refresh, not a reading of every conversation, disk, marketplace
or plugin implementation. One recent relevant conversation about a screen-control
plugin reaffirmed PBLang-first reuse of existing product boundaries and treating
Rust as retained reference. No private conversation contents are copied here.

Metadata inventory: 249 cached manifests (215 Codex, 34 Claude), no new name/version
pairs relative to the previous intake. Cached metadata is not proof of installation,
activation or operational safety. A read-only Claude CLI inventory reported 32
installed entries: 30 configured enabled, two configured disabled. This records
configuration only. Current callable tool metadata contained 986 entries; this is
not a count of installed plugins. No plugin was installed, enabled, disabled or
executed as an integration, and no accounts or credentials were inspected.

## Primary sources and concrete transfers

- [cachetools documentation](https://cachetools.readthedocs.io/en/latest/) describes
  separate FIFO/LRU/TTL policies, memoization and the distinction between expiry
  and actual removal. Transfer implemented: preserve expiry semantics while
  removing work from reads that have no deadline; document mutable-value and
  accounting limitations. PBLang did not copy cachetools' overwrite policy or
  adopt its monotonic timer in this slice.
- [Gemini context caching](https://ai.google.dev/gemini-api/docs/caching) describes
  reuse of repeated input and exposes cached-token usage. Transfer inferred:
  expose and test cache hits separately from expensive producer evaluations.
  `examples/cache_memoize.sapp` demonstrates this with local derived work and the
  existing get_or_set API. This is not a Gemini integration, prompt retention,
  background training or proof of AI-model acceleration.
- [Claude plugin discovery](https://code.claude.com/docs/en/discover-plugins)
  distinguishes marketplace discovery and local plugin management. Transfer:
  keep discovery, installed configuration and tested capability evidence separate
  in future PBLang plugin catalogs. This is a proposal, not an implemented catalog.

## Delivered and pending

Delivered: lookup optimization, false-like-value/expiry/LRU regressions, limited
self-hosted lookup parity, a local memoization example, paired microbenchmarks and
explicit clock-gap documentation. Existing CDN, data_store and pbAI callers reuse
this library and passed their targeted suites; product-wide speedups are unmeasured.

Pending ideas: monotonic clock abstraction across backends; explicit cache ownership
and invalidation for project/script artifacts; immutable versioned capability
metadata; reproducible benchmarks for actual pbAI/compiler workloads before enabling
reuse there. These need correctness, privacy and resource tests before adoption.

Unresolved roadmap: shared/composite memory, native runtime/FFI, backends,
audit/release authenticity, PBIDE and guest operating systems. No universal support,
full disk scan, full marketplace audit or 1.0.0 readiness claim is made.
