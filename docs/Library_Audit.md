# PBLang library audit

This is the rolling inventory and hardening queue for Section 7 of the PBLang
development directive. It separates verified implementations from file-name
coverage: the presence of a module is not evidence that every API is complete,
safe, or standards compliant.

## 2026-07-18 inventory

`stdlib/` currently contains 326 top-level `.sapp` modules and 448 `.sapp`
files including nested compiler/pbAI/tooling modules. The current scan found
existing implementation surfaces for:

| Area | Existing modules or facades | Audit direction |
|---|---|---|
| GUI/graphics/games | `gui`, `micro_gui`, `graphics`, `canvas`, `gpu2d*`, `gpu3d*`, `game`, `image_processing` | Keep CPU fallback and verify GPU batching/resource lifetime rather than adding another GUI facade |
| Web/networking | `http`, `http_client`, `http_server`, `web_framework`, `websocket`, `url`, `networking*`, `tls`, `cdn_cache` | CDN policy/hierarchy is verified; continue facade ownership, framing, redirect, timeout, and provider-adapter work |
| Data/formats | `json`, `csv`, `dataframe`, `serialization`, `config`, `codec`, `encoding` | `serialization` delegates JSON/CSV and now provides the bounded common facade; do not add parallel JSON/CSV parsers |
| Database/storage | `db`, `orm`, `storage`, `cache`, `data_store`, `redis`, `render_cache` | Unified memory/SQLite/Redis KV contract is verified; continue bounded query-result and streaming large-object work |
| Security/crypto | `crypto`, `crypto_advanced`, `crypto_quantum`, `security`, `jwt`, `web_security` | Separate primitives, protocol policy, and convenience APIs; require vectors and misuse-resistant defaults |
| Math/science/AI | `math`, `stat`, `matrix`, `tensor`, `science`, `ml`, `nn*`, `llm*`, `pbai*` | Gate accelerators by measured workload size and preserve deterministic CPU reference paths |
| System/device | `fs`, `path`, `process`, `device`, `platform_emulator` | Continue capability and path-boundary review; emulation claims require executable conformance tests |
| Testing/performance | `testing`, `proptest`, `fuzz`, `mock`, `snapshot`, `benchmark`, `profiler`, `language_benchmark` | Reuse these foundations for every library slice instead of creating per-module test runners |

Audio/video do not currently appear as dedicated top-level library modules in
this inventory. That is a real coverage gap, but a new API should be designed
only after codec/container/device boundaries and the graphics/multimedia
ownership model are specified. XML support is entity-safe text handling, not a
standards-complete parser. TOML and YAML are documented native subsets.

## Completed slice: serialization foundation

- Fixed JSON sentinel collisions and escaped-string TOML/YAML round trips.
- Added bounded, structured, strict parse/stringify/convert APIs.
- Rejected duplicate keys and unsupported constructs instead of silently
  dropping or approximating them.
- Preserved legacy trusted-input APIs with hard safety ceilings.
- Added semantic, deterministic fuzz, example, benchmark, and API-boundary
  documentation.

See `docs/Serialization.md` and
`tests/semantics/serialization_hardening_test.sapp` for the verified contract.

## Completed slice: storage and cache foundation

- Added a shell-free, bounded RESP2 Redis client with TLS, AUTH, SELECT,
  pipelining, typed server errors, and deterministic protocol tests.
- Added a namespaced memory/SQLite/Redis key/value facade that preserves cached
  `nil`, TTL, invalidation, metrics, and explicit resource lifecycle.
- Extended `cache.sapp` with opt-in LRU while preserving FIFO compatibility.
- Replaced new pbAI persistent-cache keys with verified SHA-256 metadata and
  added TTL, hard quotas, staged publication, backup recovery, cached `nil`, and
  explicit legacy-v1 status.
- Added a runnable example and a correctness-gated multi-sample benchmark.

See `docs/Data_Storage.md`, `tests/semantics/data_store_test.sapp`,
`tests/semantics/redis_resp_test.sapp`, and `bench/data_store_stages.sapp`.

## Completed slice: HTTP CDN cache semantics

- Added a bounded edge/regional/origin-shield hierarchy with promotion and
  separate browser/shared TTL policy.
- Added Cache-Control parse/render, ETag/304, SWR, explicit revalidation,
  stale-if-error, immutable hashed assets, exact/path/tag purge, and metrics.
- Default admission bypasses private/authenticated/cookie/range traffic,
  Set-Cookie, unsafe Vary, hostile headers/hosts, and oversized objects.
- Fixed HTTP response byte Content-Length and standard 304/502 reason phrases.
- Added semantic coverage, a runnable static-site example, phase benchmark, and
  deployment/trust-boundary documentation, all without new Rust.

See `docs/CDN_Caching.md`, `tests/semantics/cdn_cache_test.sapp`,
`examples/cdn_cache/`, and `bench/cdn_cache_stages.sapp`.

## Next verified slices

1. **Duration and money parser hardening.** The existing seeded parser probe
   reports thrown inputs in both modules; minimize each reproducer, add tests,
   and decide whether invalid user input returns a result error or throws.
2. **HTTP/network ownership and provider adapters.** Map `http` versus
   `http_client` and `networking` versus `networking_advanced`; specify an
   authenticated external-CDN purge/deployment adapter only after compatibility
   tests identify the canonical layers.
3. **Cryptography API boundary audit.** Classify primitives versus protocols,
   run known-answer tests, and ensure insecure/legacy operations cannot be
   selected accidentally.
4. **Database result streaming and native server drivers.** SQLite transaction
   failure atomicity and the shared KV layer are covered. Bounded row streaming,
   pooling, and native PostgreSQL/MySQL drivers remain separate future slices.
5. **Media architecture.** Specify audio/video streams, codecs, buffering,
   hardware acceleration, and portable fallback before adding modules.

Each queue item is complete only with focused tests, a runnable example where
applicable, documentation, full semantic regression, and measurements for
performance changes.
