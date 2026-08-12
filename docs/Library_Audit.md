# PBLang library audit

This is the rolling inventory and hardening queue for Section 7 of the PBLang
development directive. It separates verified implementations from file-name
coverage: the presence of a module is not evidence that every API is complete,
safe, or standards compliant.

## 2026-08-12 confined prefix reads

`stdlib/digest.sapp` supplies binary-safe SHA-1, MD5 and BLAKE3 plus unified
SHA-256 dispatch without Rust or runtime hash builtins. BLAKE3 implements the
full 1 KiB chunk tree rather than a single-chunk subset. Official vectors cover
0, 1024, 1025, 2048, 3072 and 4096 bytes; generated C99 output matches the
interpreter. SHA-1 and MD5 are compatibility algorithms and must not be used as
modern collision-resistant security primitives.

`stdlib/linux_dirfs.sapp` now exposes `read_prefix_bytes(fs, path, max_bytes)`.
It opens the final component relative to held directory descriptors with
`O_NOFOLLOW`, obtains the source size from that descriptor, rewinds it, and
copies at most the requested prefix into a private memfd before the VM loads
the bytes. This closes the memory-bound gap where callers had to read an entire
large file or reject it instead of safely inspecting a prefix. Symlink
traversal remains fail-closed. Concurrent content mutation of the already-open
inode is not serialized; callers needing an immutable source must coordinate
writers externally or compare a complete content hash.

`linux_dirfs.metadata` now returns type, size, and second-resolution mtime from
one held descriptor. It passes only `/proc/<server-pid>/fd/<held-fd>` to a
fixed-argv `stat` probe, so caller-controlled path text is never reparsed by the
subprocess and namespace replacement cannot redirect the result. Symlinks fail
closed; supported regular files/directories match the current product contract.
This closes the PBLang pb-mcp-server `file_info`
and 30-path `batch_file_info` parity group on Linux; a portable non-Linux
handle-stat backend remains a separate platform task.

`linux_dirfs.descriptor_path` exposes the `/proc/<pid>/fd/<fd>` path of a
currently open directory root. This is a narrow integration primitive for
fixed-argv tools that must operate on the already-resolved object rather than
reopen caller-controlled namespace text. It fails after close and does not
claim portability beyond Linux `/proc`; callers must retain the root until the
child operation is complete.

## 2026-08-12 authenticated file-crypto foundation

`stdlib/aes_gcm.sapp` now supplies pure-PBLang HMAC-SHA256,
PBKDF2-HMAC-SHA256 and AES-256-GCM. Official AES-GCM and PBKDF2 vectors,
independent Python-compatible vectors, and a generated-C99 differential test
cover the interpreter and native backend. The pb-mcp-server uses this module in
a lazily built PBLang C99 worker for Rust-compatible ENCV2 files.

The worker boundary is deliberately fail-closed: secrets use a fixed
mode-0700 queue, random request IDs, marker-last publication, one atomic claim
per process, bounded result polling and complete transient cleanup. The
project filesystem retains only the reproducible binary and SHA-256 metadata.
`linux_dirfs` now verifies the actual descriptor mode after `fchmod`; a mounted
filesystem that reports success but keeps 0755/0644 permissions is rejected.

Measured over 4 KiB, warm pb-mcp encryption is currently about 5.01 seconds
median and decryption 5.22 seconds median because every operation performs
210,000 pure-PBLang PBKDF2 rounds and starts a bounded worker. Correctness and
secret isolation are complete for this Linux contract; reducing KDF/worker
latency without changing the ENCV2 security parameters remains measured
performance work. Portable non-Linux descriptor/permission backends are not
claimed.

## 2026-08-11 structural inventory

The tracked tree now contains 519 `.sapp` files under `stdlib/`: 391 top-level
modules, 19 self-hosted compiler modules, 10 kernel modules, 4 native-codegen
modules, and 95 files under `stdlib/ai/`. It also contains 768 semantic test
files, 83 example sources, and 78 benchmark sources. Every tracked stdlib file
was included in the inventory by path, size, category, and exact import
association. Pattern searches were used only as auxiliary evidence.

The 95-file `stdlib/ai/` category is not a normal library namespace. It embeds a
second pbAI product tree, including its tests, tools, and another pb-mcp-server.
The standalone `products/pbai` tree differs at 51 paths from this copy, while
the standalone `products/pb-mcp-server` differs at 22 paths from the nested MCP
copy (generated/cache directories and lockfiles excluded). This is a patch
drift and security-maintenance risk. One source tree must become canonical;
other product layouts should be generated and verified against a manifest.

For the remaining 424 library/compiler/kernel/native modules, 421 have an exact
`stdlib:` import reference somewhere in tracked PBLang source. 409 of all 519
files have an exact direct import from semantic tests. This is a conservative
association metric, not a coverage percentage: relative imports, entrypoints,
transitive tests, and Rust bootstrap tests are not credited. The following
non-embedded modules have no exact direct semantic-test import and require
manual ownership/coverage confirmation:

`android_toolchain`, `app_templates`, `bitops`, `blockchain`, `bootstrap`,
`check_cli`, `cli`, `code_graph`, `date`, `emulator_support`, `ffi`, `heap`,
`ide`, `polyglot_web`, and `profiler`.

The largest review units are `native/codegen_x64.sapp` (270,053 bytes),
`ide_features.sapp` (219,389 bytes), `compiler/ctranspile.sapp` (68,830 bytes),
`compiler/parser.sapp` (63,389 bytes), and `compiler/typeck.sapp` (56,456
bytes). Splitting these by stable ownership boundary would reduce review and
incremental-import cost, but only after benchmarks prove no startup regression.

The project audit library now fails closed when it audits zero eligible files,
times out, or truncates collection. Reports expose `complete` and
`coverage_error`; module audits also reject zero-file modules as incomplete.
This prevents an empty scan from becoming release evidence. Focused regression
tests cover both regular and module-by-module modes.

Open structural priorities:

1. Remove live source duplication between PBLang, pbAI, and pb-mcp-server.
2. Add explicit coverage contracts for the 15 modules listed above.
3. Make stdlib snapshots immutable per test run and prevent tests from changing
   tracked corpus/cache fixtures.
4. Replace direct lockfile writes with atomic, serialized publication in the
   PBLang-native build path before retiring the bootstrap implementation.
5. Add a coherent media architecture; there is still no dedicated top-level
   audio or video stream/device library.

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
