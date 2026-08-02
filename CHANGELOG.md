# Changelog

All notable changes to **pblang** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Canonical exact-i32 call-frame lowering image
- Added `stdlib:host_pbck_frame_lowering.sapp`, a PBLang-only dense primitive-
  array image for any bounded call graph accepted by
  `host_pbck_frame_calls.verify_i32_prepared()`. It remaps reachable functions,
  constants, defaults, captures and entry arguments, re-executes the verifier
  before publication and binds the complete image to a deterministic checksum.
- Added independent fail-closed validation for exact integer tables, canonical
  sentinel operands and capture ranges, function/default/argument arity,
  signed-i32 values and observed resource quotas. Tests cover three active
  frames, local-to-nonlocal shared capture, a non-zero original entry ID,
  direct function constants, defaults, entry arguments and tampering.
- Added a runnable example and a nine-sample staged benchmark. The three-frame,
  ten-instruction fixture measured `0.466 ms/image` median build and
  `0.179 ms/image` median validation. The complete suite passed 748/748. No
  Rust, Java, Kotlin or C source was added.

### Bounded call-graph selection
- Added validated `min_reachable_functions` and `max_reachable_functions`
  options to `host_pbck_frame_calls.find_i32_call_probe()`. Platform backends
  can now select a graph shape deliberately instead of accepting the first
  smaller compatible probe; invalid and inverted bounds fail closed.
- Added deterministic three-function selector coverage over a nested non-local
  capture graph. The complete PBLang semantic suite remains 747/747 and no Rust
  source was added.

### Verified portable i32 call graphs
- Extended `stdlib:host_pbck_frame_calls.sapp` with deterministic
  `verify_i32_prepared()` and `verify_i32()` lowering gates. They execute the
  selected graph under explicit quotas and statically validate every reachable
  constant, default, local/upvalue operand, closure target/capture, call arity
  and jump target before a platform backend consumes it.
- Required upvalue cells are derived from actual upvalue operations and
  non-local captures, preserving compatibility with older frame metadata while
  keeping malformed graphs fail closed. Two-function mutation/tail-call and
  three-function nested shared-cell tests pass. No Rust source was added.

### Deterministic securelayer tamper regression
- Corrected the authenticated-envelope test so it always flips the final hex
  digit. The previous fixture could replace an existing `00` suffix with the
  same bytes and intermittently ask a valid envelope to fail authentication.
  The production securelayer implementation was not changed; the corrected
  fixture passed 100/100 focused repetitions and the complete 747-test suite.

### Complete PBFP execution bridge
- Added `stdlib:host_pbck_frame_vm.sapp`, a bounded PBLang-only bridge from
  integrity-checked flat PBFP assets to the complete 48-opcode host PBCK VM.
  Preparation reconstructs and freezes canonical functions, constants,
  captures, defaults and operands once; repeated execution retains explicit
  capability injection and the VM's runtime quotas.
- Added exhaustive 48-opcode reconstruction parity, host-adapter isolation,
  malformed graph, foreign-slot, resource-limit and redaction tests, plus a
  runnable example and a staged benchmark. Per-function value-slot and closure
  indexes keep preparation linear in constants plus instructions instead of
  rescanning a function's constant table for every instruction. Prepared chunks
  retain compact opcode/operand arrays rather than allocating one map per
  instruction. Preparation reads validated flat instruction arrays directly and
  allocates capture descriptors only for closure opcodes. No Rust source was
  added.

### Bounded virtual environments
- Added isolated `stdlib:env.sapp` virtual stores with portable key validation,
  deterministic snapshots, explicit entry/value/aggregate-byte quotas and no
  fallback to ambient host variables.
- Fixed `.env` loading to publish a validated local map atomically instead of
  calling the deliberately disabled process-wide `env_set` builtin. Empty
  values remain distinguishable from missing values.
- Added positive, isolation, quota, invalid-input and failed-write regression
  coverage plus a runnable virtual-environment example. No Rust source was
  added.

### Bounded host-PBCK decoder and call-frame engine
- Corrected both PBLang-native PBCK executors to use canonical VM stack-slot
  locals. Arguments and declarations now share frame slots, captures retain the
  original mutable slot cell, and sparse `SetLocal` writes fail closed. A
  compiler-emitted closure/call regression and the real PBRemote portable call
  probe now return the same result as the native PBCK runtime.
- Added reachable-call-graph preparation and deterministic exact-i32 call-probe
  selection. Unsupported functions outside the selected entry subgraph no
  longer block a safe probe, while every reachable opcode and function
  signature is still validated before execution.
- Defined exact signed-i32 edge behavior for platform frame executors with
  tests for overflow, fractional division, signed remainder and negative
  powers. This prevents generated Android/JVM runtimes from silently adopting
  Java integer wraparound semantics.
- Added `stdlib:host_pbck_frame_binary.sapp`, a deterministic bounded `PBFP-v1`
  transport for complete canonical frame plans. It uses compact typed payloads,
  explicit resource limits and SHA-256 over canonical Base64 payload bytes, and
  rejects malformed, truncated, extended or tampered assets.
- Added closure/capture, rich-constant, metadata, deterministic round-trip and
  negative integrity tests, a runnable example and an encode/decode benchmark.
- Added `stdlib:host_pbck_frame_plan.sapp`, a bounded canonical lowering from
  recursive PBCK chunks to flat function, instruction, constant and capture
  tables. Closure operands become stable function IDs, constant operands become
  global value slots, and platform generators no longer need their own
  recursive decoder. Full validation remains at build boundaries while hot
  instruction and constant access performs only constant-time shape checks.
- Added fail-closed `function_at`, `capture_at`, `default_at` and
  `parameter_type_at` accessors and hardened existing instruction, constant and
  value lookups with integral range validation. Platform runtimes can now share
  one bounded frame-program access contract instead of indexing raw tables.
- Added compact byte/file construction, positive and fail-closed nested closure
  tests, a runnable example and a two-stage microbenchmark. No Rust source was
  added.
- Completed the PBLang-native PBCK-v1 execution contract for all 48 opcodes.
  The VM now implements exports, LIFO deferred calls, exponentiation, safe
  attribute/index access and result unwrapping with guest-exception routing.
  Tail-call frame reuse is conservatively disabled while a frame owns pending
  deferred calls, preserving cleanup and return-value semantics.
- Fixed capability-injected host adapter dispatch: `options.adapters` is now
  validated and invoked exactly once with a bounded argument vector instead of
  being recognized by `get_global` and then rejected by the hard-coded host
  call path. Adapter failures are redacted and non-callable entries fail closed.
- Added `stdlib:host_pbck_link.sapp`, a non-executing recursive link analyzer
  that deterministically inventories internal globals, exports, built-ins and
  unresolved host dependencies across every nested PBCK function.
- Added a PBLang-native PBCK-v1 decoder with strict artifact, depth,
  instruction, constant, string, parameter and capture budgets, exact trailing
  data rejection and full nested-function metadata validation.
- Added a bounded execution engine for the full 48-opcode PBCK-v1 contract.
  Closures retain shared mutable upvalue cells,
  cross-frame guest throws unwind to caller handlers, and tail calls use a
  constant-depth trampoline.
- Added a compact parallel-array representation for large artifacts. On the
  2,416,020-byte PBRemote core it reduced benchmark peak RSS from 391,140 to
  201,092 KiB (48.59%) and median decode latency from 819.470 to 742.830 ms
  (9.35%). A 1,000-call microbenchmark measured 43.740 ms median. No Rust source
  was added.
- Added explicit self-hosted `tail: true` call lowering to native PBCK
  `OpTailCall`, with direct translator/VM regression coverage. This lets
  PBLang-generated Android conformance artifacts exercise the same trampoline
  contract without mutating decoded bytecode after translation.

### Verified precompiled x86-64 codegen driver
- Replaced compiler-sized source-link scans that allocated per codepoint with a
  bounded UTF-8 byte-indexed scanner while preserving strings and comments.
- Added a PBLang-only content-addressed PBCK driver cache with dependency-graph,
  exact compiler-binary SHA-256, PBCK integrity and generated-ELF execution
  gates before atomic publication.
- The phase benchmark measured 43.804 ms source-import publication versus
  7.169 ms precompiled publication (6.11x), and 19,472 versus 12,720 KiB peak
  RSS (34.68% lower). No Rust source was added.

### Unicode character classification example
- Completed Panos Biazis's `check_char_type` example from its original C school
  exercise without recursion, undefined indexes or non-advancing loops.
- Extended `stdlib/unicode.sapp` with bounded scalar classification, Unicode
  whitespace, decimal digits from multiple scripts, cased-letter detection and
  ordered text analysis.
- Added Greek, Arabic-Indic digit, emoji, mathematical-symbol, punctuation,
  empty-input and multi-character regressions. No Rust source was added.

### Verified PBLang-native bootstrap refresh
- Added a PBLang-native source-to-PBCS compiler CLI and a bounded bootstrap
  refresh tool that snapshots all 19 compiler modules, verifies each artifact
  structurally and publishes a digest-bound manifest.
- Added a real 202-byte static Linux x86-64 probe generated by
  `stdlib/native/codegen_x64.sapp`; the refresh gate executes it and requires
  exit code 42 before publication.
- Hardened the self-host bundle dependency scanner to ignore comments and
  strings, normalize canonical `stdlib:` module paths and reject traversal.
- Fixed optional-value type mismatches in the guard preprocessor and C99 modulo
  lowering found by compiling the compiler modules through the self-hosted type
  checker. The complete semantic suite passes 733/733. No Rust was added.

### Bounded DEFLATE and PNG hot paths
- Added a 9-bit canonical-Huffman lookup fast path with the existing bitwise
  decoder retained as a deterministic fallback for longer codes.
- Changed RFC 1950 Adler-32 to the bounded NMAX=5552 reduction schedule,
  replacing two modulo operations per byte with two per block. A same-process
  PBLang benchmark measured 3.199 ms versus 1.603 ms for 12,000 bytes
  (1.996x, 49.90% lower latency).
- Moved output-cap checks from every copied match byte to bounded block/match
  growth checks and added zero-copy `crc32_range`.
- Specialized all PNG scanline filters, removed per-pixel RGBA function
  dispatch, cached downsample columns and now rejects trailing zlib bytes.
- Added CRC-range, strict-stream, cross-block Adler and malformed-range
  regression tests. On the current PBRemote 320x200 fixture, repeated final
  stage medians were 150.330-152.826 ms for zlib and 287.491-295.023 ms for
  integrity/decode/publish. No Rust source was added.

### Bounded PNG and interactive image surfaces
- Added a pure-PBLang PNG decoder with chunk CRC, zlib checksum, source/output
  quotas, scanline filters and 8-bit grayscale/RGB/indexed/alpha support.
- Added optional local-coordinate click callbacks to GUI image widgets.
- Optimized nearest-neighbour upscaling to draw source-sized blocks instead of
  issuing one point operation for every destination pixel.
- Added focused malformed-image, palette, transparency, bomb-bound and image
  input tests. No Rust source was added.

### Runtime-responsive cross-surface modern UI
- Changed PBLang-generated Android Activities to query the actual
  `screenWidthDp` instead of freezing the build-time preview width.
- Responsive rows now stack and grids collapse at a bounded runtime breakpoint,
  including after orientation-driven Activity recreation. Interactive controls
  default to 48dp touch targets and retain scroll/keyboard resize fallbacks.
- Hardened Web output for 360px-class screens, short landscape displays,
  notched safe areas, dynamic viewport height, long text and horizontal
  overflow. Added focused Android/Web tests and a multi-viewport benchmark.
- No Rust source was added.

### Native input and FreeBSD target verification
- Extended the PBLang-native X11/XTest adapter with modifier-correct ASCII text
  and current-keymap Unicode keysym injection, plus an isolated Xephyr live test.
- Added an authenticated, replay-protected, bounded loopback input service and
  the opt-in `tools/pbinputd.sapp` native-X11 daemon entrypoint.
- Added execution-model verification for generated ARM64 uinput ELF artifacts
  and a FreeBSD 12+ x86-64 ELF backend with explicit FreeBSD syscall ABI.
- The canonical semantic suite passes 708/708. Isolated live verification used
  Xephyr only; no input was sent to the active desktop and no daemon remained.

### Pure-PBLang PBRemote secure foundation and product integration
- Added the first executable PBRemote vertical slice entirely in PBLang:
  bounded/versioned protocol messages, capability negotiation, HMAC pairing,
  resumable authenticated sessions, replay protection, encrypted envelopes,
  revocation, hash-chained audit records, and a fail-closed service dispatcher.
- Added a loopback-first HTTP host with body/rate limits, security headers, an
  explicit trusted-tunnel gate for non-loopback binds, CLI status/demo/serve
  commands, a runnable pairing example, architecture/security docs, and focused
  protocol/auth/session/HTTP regressions.
- Fixed the common HTTP serializer so JSON response maps preserve validated
  custom headers on the actual wire. This closes the gap where in-memory maps
  carried `no-store`/`nosniff` but live TCP responses silently dropped them;
  CR/LF-bearing header names and values remain rejected.
- Extended project survey, PBIDE management boards, product health, AAP status,
  pbOS intake, and pb-mcp-server planning to six logical products. Replaced
  obsolete `ai/`, `os-dev/`, and `tools/` default roots with the current sibling
  `products/` repositories. No Rust code was added.

### Verified self-hosted module cache and x86-64 compile sessions
- Added PBCS v2 with bounded structural validation, exact input consumption,
  explicit fused-opcode operands, v1 reader compatibility, and rejection of
  unsupported constants, invalid references, excessive nesting, and oversized
  artifacts. Fixed multi-byte u32 serialization above 255 entries.
- Public `.pbck` loader execution is bounded by default and exposes explicit
  resource options and runtime accounting.
- Added an opt-in persistent self-hosted module cache with compiler/VM
  fingerprint invalidation, verified PBCS deserialization, corruption and
  store-failure telemetry, staged publication, source invalidation, tests, and
  a runnable example.
- Added a PBLang-only direct x86-64 compile session that imports the backend and
  fingerprints its dependencies once per multi-file build. It validates every
  safety certificate and ELF before publishing outputs and retains a verified
  content-addressed cache across sessions.
- On the recorded six-file benchmark, independent compiler processes measured
  445.675 ms median, one cold batch process 102.972 ms (4.33x faster), and a
  warm-artifact batch 81.758 ms (5.45x faster). Warm imported-module loading
  measured 84.549 ms versus 128.271 ms uncached (1.52x); the first verified
  cache store was slower at 271.464 ms. Full evidence and limitations are under
  `bench/results/2026-07-19-compiler-caches/`. No Rust code was added.

### Pure-PBLang compile cache, checked C99 modulo, and interpreter dispatch
- Added a bounded content-addressed Linux x86-64 artifact cache with source and
  backend dependency invalidation, digest/metadata/ELF validation, staged
  publication, corruption tests, and a standalone cached compiler driver.
- Added phase benchmarks for startup, backend import, emission, uncached/cold/
  warm publication. Initial uncached publication improved from 120.057 to
  78.954 ms; a saved independent run measured 97.660 ms uncached and 23.998 ms
  on a verified warm hit. Cold backend import remains the primary bottleneck.
- Optimized hosted C99 integer modulo while rejecting literal zero and checking
  dynamic zero plus `LONG_MIN % -1`. A paired 19-sample run reduced the integer
  fixture median from 14.675 to 5.915 ms with no artifact-size change.
- Added compact safe leaf-binary plans to the PBLang-written tree interpreter,
  plus direct plain-assignment and while-condition paths. The saved same-process
  control measured 170.536 ms optimized versus 223.439 ms generic dispatch.
- Added focused semantic tests, drivers, raw benchmark evidence, and a refreshed
  cross-language matrix. Build and phase suites pass; release Rust semantics are
  203/0/1 and the canonical PBLang suite is 695/695. No Rust code was added.

### Pure-PBLang x86-64 constant division and immutable model caches
- Added conservative unsigned-32 range facts to the PBLang-written direct
  x86-64 backend. Proven constant division/remainder now uses exact reciprocal
  multiplication or bounded compare/subtract lowering; unknown, negative, and
  overflowing values retain the previous hardware `div` fallback.
- Added executable boundary/opcode/fallback tests and a fixed five-toolchain
  benchmark. On the recorded million-iteration checksum fixture, direct codegen
  improved from 5.676 ms to 1.924 ms median (2.95x) while the ELF grew from 487
  to 507 bytes. C measured 3.988 ms and 14,472 bytes on the same fixture. The
  complete direct compile path remained 1.99x slower than C compilation.
- Added `stdlib:model_cache.sapp` with bounded JSON reads, strict MLP shape,
  version and numeric validation, per-user versioned paths, staged verification,
  atomic POSIX publication, and a recoverable Windows replacement fallback.
- Moved both pbAI intent-model writers out of locked `stdlib/pbai_model.json`.
  Lazy training now writes under the user cache (or an explicit environment
  override), while the bundled model remains read-only.
- Added cache corruption and source-immutability regressions. The canonical
  semantic suite passes 692/692 in 90.06 seconds; the bundled model SHA-256 and
  mtime were unchanged across the parallel run. No new Rust code was added.

### Pure-PBLang CDN cache and HTTP correctness
- Added `stdlib:cdn_cache.sapp` with bounded edge and regional/origin-shield
  caches, promotion, browser/shared TTL policies, Cache-Control parsing and
  rendering, ETag/304, SWR, explicit/blocking revalidation, stale-if-error,
  immutable content-hashed static assets, bounded purge, and metrics.
- Hardened admission against authenticated/cookie/range/private traffic,
  Set-Cookie, unsafe Vary, Host/header injection, hop-by-hop headers, oversized
  objects, unbounded query/tag cardinality, and downstream caching of sensitive
  bypass responses.
- Added bounded canonical-key memoization. On the recorded local benchmark it
  reduced edge-hit median from about 3.68 ms/op to 0.108 ms/op while retaining
  pure-PBLang SHA-256 as the final key.
- Fixed HTTP `Content-Length` to count UTF-8 bytes and added standard reason
  phrases including 304 and 502. Added semantic coverage, a runnable static
  example, a nine-sample phase benchmark, and `docs/CDN_Caching.md`.
- Hardened pbAI backup recovery so a corrupt primary cannot discard a valid
  backup, and excluded structurally invalid JSON from cache quota/status counts.
  The canonical suite passes 689/689 in 205.21 seconds with zero failures,
  ignored, or filtered tests. No new Rust code was added.

### Pure-PBLang bounded storage and cache foundation
- Added `stdlib:redis.sapp`, a shell-free RESP2 client with bounded framing,
  TLS certificate verification, AUTH/SELECT, pipelining, SCAN, typed errors, and
  pure protocol tests.
- Added `stdlib:data_store.sapp`, a namespaced memory/SQLite/Redis key/value API
  with TTL, cached-nil semantics, invalidation, metrics, hard memory/SQLite
  quotas, bounded Redis values, and explicit close lifecycle.
- Preserved FIFO compatibility in `cache.sapp` and added opt-in exact LRU,
  non-mutating `peek`, and live `keys` snapshots.
- Hardened the pbAI persistent cache with domain-separated SHA-256 metadata,
  TTL, hard per-kind quotas, staged publication, backup recovery, cached nil,
  corruption rejection, and explicit legacy-v1 compatibility status.
- Added `examples/data_store`, `bench/data_store_stages.sapp`, focused semantic
  coverage, and `docs/Data_Storage.md`. No new Rust code was added.

### Pure-PBLang reproducible competitor benchmark core
- Added a bounded PBLang-written registry and runner for the PBLang interpreter,
  hosted C99, direct Linux x86-64 codegen, C, C++, Rust, Go, Java, C#/Mono,
  Python, Node.js, and Zig. Foreign fixtures are fixed data; callers cannot
  inject commands or arbitrary source.
- Added a fixed pure-PBLang direct-codegen driver that imports
  `stdlib/native/codegen_x64.sapp`, requires its safety certificate, validates
  the generated ELF header, writes the artifact, and executes it under the same
  bounded correctness/resource contract. Unsupported OS/architectures are
  explicit unavailable rows; there is no silent C99 fallback.
- Added correctness-gated startup and one-million-iteration integer workloads,
  first/cold compile and run samples, discarded warm-up, warm median/p95 and raw
  samples, GNU-time CPU/peak-RSS metrics, stripped release artifact sizes, host
  and toolchain metadata, timeouts, output limits, and bounded repetition counts.
- Temporary workspaces use the system temp root plus random UUID v4 names;
  setup failures are contained and cleaned instead of leaking partial fixtures.
- Added an explicit ledger for all 22 requested benchmark categories. Missing
  equivalence contracts are reported as partial, conditional, or not comparable
  rather than represented by fake results.
- Added `examples/repro_codegen_benchmark/main.sapp` and a tested PBLang-native
  `src/main.sapp` workspace entrypoint, repairing the previously declared but
  missing entry that prevented self-hosted `pblang build .`.
- Fixed the PBLang `os.script_args()` helper to honor the CLI-provided
  `PBLANG_SCRIPT_ARGS` channel for package-directory execution. Commands passed
  through `pblang run . -- ...` no longer disappear merely because the user
  selected a package instead of a literal `.sapp` path; older hosts retain the
  existing argv-parser fallback.
- The verified i7-1195G7 run produced 22 successful rows, 2 explicit Zig-
  unavailable rows, and 0 failures. Direct x86-64 codegen measured 5.361 ms
  median versus C at 4.011 ms and hosted C99 at 15.386 ms on the fixed integer
  kernel. Its 487-byte ELF was 96.6% smaller than the C artifact. The interpreter
  measured 277.461 ms versus Python at 126.454 ms. These strengths, gaps,
  methodology boundaries, completion evidence, and all raw samples are recorded
  in the benchmark report. The clean matrix completed in 16.06 seconds with
  95,236 KiB process peak RSS. A separate six-category interpreter matrix added
  54 passing rows and 36 explicit unavailable rows with no failures. All 27
  PBLang compiler/backend phases, five polyglot phases, and six platform-emulator
  phases passed. The canonical eight-worker semantic suite passes 689/689 in
  205.21 seconds with zero failed, ignored, or filtered tests and 1,801,672 KiB
  whole-runner peak RSS. No Rust code was added.

### Cost-aware pure-PBLang hardware and I/O routing
- Replaced item-count-only CPU routing with a deterministic, saturating cost
  model. Cheap jobs remain serial; heavier jobs receive only as many bounded
  contiguous workers as can amortize launch cost and useful work per worker.
- Added launch and transfer-cost gates for registered GPU, NPU, SIMD, and
  distributed runtimes, a 1 GiB default transfer budget, explicit rejection
  reasons, and cost-aware CPU recovery after accelerator runtime failure.
- Hardened device lists, weights, task counts, cost hints, proportional planning,
  paths, and policy mutation. Registered runtime hooks are visible as executable
  devices even when a platform probe cannot identify their physical backend.
- Replaced one-thread-per-file compatibility behavior with
  `storage_read_controlled`: tiny batches stay serial and larger batches use at
  most 64 chunk workers (eight by default). Whole-file byte usage remains bounded
  only by the underlying `fs_read` backend and is documented as such.
- Added focused semantics, a runnable routing example, and a correctness-gated
  benchmark, all in PBLang. On the recorded i7-1195G7 run, the cheap 128-item
  kernel measured 42.397 ms serial median versus 43.533 ms forced-parallel
  median across nine warm, interleaved samples (about 2.6% lower); this is a
  local workload result, not a whole-language or cross-machine claim. The final canonical
  eight-worker semantic suite passes 685/685 in 265.61 seconds with zero
  failures, ignored tests, or filtered tests and 1,727,616 KiB whole-runner peak
  RSS.

### Bounded pure-PBLang thread supervision from AAP review
- Replaced the incomplete `thread.supervisor()` implementation: configured
  timeouts are now enforced, completion/failure state is owned by the parent VM,
  and `check` no longer reports every retained record as alive.
- Added bounded cooperative cancellation, deadline/status snapshots, deterministic
  waiting/reaping, explicit cooperative arguments, late-completion collection,
  dot/colon method compatibility,
  fail-closed token validation, channel-backed cross-VM cancellation with
  observation proof, bounded cancellation reasons, and explicit
  `active`, `unreaped`, and `detached` observability under a 1,024-record ceiling.
- Kept `kill_all` for source compatibility but removed its false force-kill claim;
  it returns `preemptive: false` and requests cancellation. Blocking native work
  still requires a timeout or disposable process at the external boundary.
- The design was informed by the AAP Asio timeout example, the legacy MCP timeout
  helper, and current VM supervisor/reference code, then reimplemented in PBLang
  without copying their blocking or non-preemptive behavior. Added semantic
  coverage, a correctness-gated phase benchmark, safety/tooling documentation,
  and `examples/supervised_tasks`. No new Rust code was added.
- The local phase baseline measured 64 spawn+wait completions at 28.09 ms total,
  4,096 terminal status lookups at 51.24 ms total, and 32 cooperative
  cancel+wait operations at 6.98 ms total. These are single-run subsystem
  baselines, not cross-language claims. The final canonical eight-worker suite
  passes 684/684 in 328.52 seconds with zero failures, ignored tests, or filtered
  tests and 1,766,092 KiB whole-runner peak RSS.

### PBIDE bounded adaptive render-policy cache
- Removed full synthetic frame-schedule construction from the 60 Hz PBIDE idle
  callback. The loop now retains one bounded runtime-policy map and refreshes it
  after measured draws at a clamped `100..5000 ms` cadence.
- Runtime throttling now uses the same measured quality inputs as the full
  planner without rebuilding dirty-region, command-batch, device-route, and
  lane records. Real input remains immediate; unchanged frames still skip draw
  and present; SDL2 GPU-assisted present and CPU/software fallback are unchanged.
- Added cache lifecycle tests, PBIDE source contracts, visible policy telemetry,
  and a correctness-gated headless benchmark. Seven CPU-pinned samples measured
  a local median planner reduction from `0.8164` to `0.00861 ms/tick` (`94.8x`)
  for the modeled busy 60 Hz loop. This is not a whole-IDE or GPU claim. No Rust
  code was added.
- Replaced the legacy foreign-tool `--version` detection wrapper with a bounded
  PBLang registry and cached `os_which` path lookup. Discovery no longer starts
  Tcl or any other compiler/interpreter, preventing `tclsh --version` from
  waiting on stdin and blocking parallel semantic workers.
- The final canonical eight-worker semantic suite passes 683/683 in 309.63
  seconds with zero failures, ignored tests, or filtered tests (1,761,296 KiB
  process peak RSS). The full-suite memory value is recorded as test-runner
  evidence, not as a PBIDE memory-improvement claim.

### Bounded multi-format serialization library
- Added pure-PBLang `parse_safe`, `stringify_safe`, and `convert_safe` APIs with
  versioned result envelopes, structured diagnostics, configurable input,
  output, line, depth, and item budgets, and non-disableable hard ceilings.
- Added strict validators for the documented TOML and YAML subsets. Duplicate
  keys, malformed assignments/indentation, unsupported table arrays, YAML
  anchors/tags/block scalars, mixed root kinds, and unknown formats now fail
  explicitly instead of being ignored or silently overwritten.
- Removed JSON validation sentinel collisions, fixed quoted escape round trips,
  made TOML/YAML output order deterministic, corrected format detection for
  comma-bearing TOML/YAML arrays, and reused the tested JSON pretty printer.
- Kept the legacy TOML/YAML parsers for trusted-input compatibility, adding
  absolute input/line/depth/output ceilings. Safe serialization rejects values
  the subset cannot represent losslessly, including TOML nulls/maps in arrays.
- Added focused semantic coverage, a runnable example, a correctness-gated
  phase benchmark, and an explicit XML text-escaping boundary. No Rust code was
  added. The final canonical eight-worker semantic suite passes 683/683 in 195.19
  seconds with zero failures, ignored tests, or filtered tests.

### Controlled polyglot, FFI, parallel, and device execution
- Hardened the PBLang thread pool with 1..64 worker bounds, a queued-task cap,
  shutdown/callable validation, transferable closed-upvalue worker factories,
  parent-side deterministic task-state updates, and correct parallel failure
  accounting. Removed the inaccurate work-stealing claim and the race helper's
  exec-gated sleep dependency.
- Added versioned compute policy, route, and result records. Hardware presence
  is no longer treated as generic runtime availability; small CPU jobs run
  serially, larger jobs use bounded contiguous chunks, and accelerator failure
  either fails strictly or records an explicit CPU fallback.
- Validated accelerator runtime classes/hooks and added contained dispatch for
  GPU, NPU, SIMD, and distributed runtime adapters.
- Added runner, language, and source-size validation to every polyglot policy
  mode. Added a PBLang-side exact library/symbol FFI allowlist with arity/type
  checks, argument bounds, and raw-pointer opt-in; it can only narrow the native
  host `FfiPolicy`.
- Added semantic coverage, a runnable controlled-compute example, a
  correctness-gated phase benchmark, and boundary documentation. No Rust code
  was added. The canonical semantic suite passes 682/682 in 163.68 seconds; the
  seven-process controlled benchmark and its disclosed validation cost are in
  `bench/results/2026-07-18-controlled-execution/REPORT.md`.

### Bounded self-hosted name diagnostics and project symbol resolution
- Added the pure-PBLang `compiler/diagnostic_suggestions.sapp` engine with
  bounded input/candidate/edit-distance work, candidate de-duplication, early
  distance exits, and a lexical tie-break independent of map iteration order.
- The self-hosted type checker now fails closed on unknown identifiers and
  emits structured suggestions for identifiers, struct fields, and enum
  variants. Suggestions can be disabled without disabling the error itself.
- Cached the builtin registry once per type environment instead of rebuilding
  its full map for every generic builtin lookup. Seven samples measured a
  200.48x median reduction for 5,000 isolated registry lookups (388.6189 ms to
  1.9385 ms); this is a local hot-path result, not a whole-compiler claim.
- Added bounded project-wide declaration discovery and `extern_names` typecheck
  options, preserving valid cross-file references while continuing to reject
  names absent from the project. Frontend lint and public diagnostics now use
  the shared suggestion policy and produce compatible fix-it metadata.
- Preserved intentional dynamic semantics: missing globals inside a protected
  `try` body remain runtime-catchable, while the same name outside `try` is a
  static error. Parser-desugared class tables are predeclared so methods and the
  generated constructor can refer recursively to their class name.
- Added focused safety/parity tests, a runnable example, benchmark evidence,
  and documentation. The canonical suite passes 681/681. No Rust code was added.

### Bounded one-line application facades
- Expanded the pure-PBLang `app.sapp` facade from CLI/GUI/web/full-stack to
  REST, desktop, game, multimedia, database, networking, automation, AI, and ML
  helpers. REST and desktop reuse the hardened web and real GUI builders.
- Added `app_domains.sapp`, which lazy-loads the real PBLang domain libraries,
  passes complete modules to advanced actions, keeps previews plan-only by
  default, bounds steps/options/input/collection, and requires explicit I/O
  permission for database/networking/AI execution.
- Fixed callable validation to recognize the runtime's `fn`, `closure`, and
  `builtin` values, and fixed REST/desktop plans so adapter validation cannot
  hide facade errors.
- Added semantic coverage, a terminating multi-domain example in the PBIDE
  catalog, and a seven-sample median/p95 benchmark. Removing duplicate domain
  validation and per-check profile arrays cut the measured plan batch median by
  51.6%, process elapsed time by 31.2%, and peak RSS by 27.9% on the recorded
  local workload. Image p95 and unchanged desktop timing remain disclosed.
- The complete eight-worker semantic regression passes 680/680 with zero
  failures, ignored tests, or filtered tests. No Rust implementation was added.

### PBLang-only implementation and migration enforcement
- Added the pure-PBLang `migration_policy.sapp` engine and `pbmigrate.sapp` CLI.
  The fail-closed manifest gate rejects new Rust files and every added Rust
  line, while deletion-only Rust changes remain available for verified ports.
- Necessary non-Rust bootstrap/system/ABI/generated boundaries now require an
  exact, dated, bounded exception with an owner, verification checks, and a
  concrete removal plan. Rust additions cannot be excepted.
- Added ordered reference/parity/verified/retired migration evidence, a passing
  manifest example, focused safety/lifecycle tests, CLI contract coverage, and
  canonical policy/tooling documentation. The complete semantic suite passes
  679/679 with zero failures, ignored tests, or filtered tests. No Rust
  implementation was added.

### Self-hosting type-system parity (recursive types and ADTs)
- The PBLang-written parser now accepts bounded recursive `array`, `map`, and
  `result` types, legacy array syntax, option/non-null postfixes, and nested
  generic closers without mutating reusable tokenizer output.
- The self-hosted checker preserves typed struct fields and enum payloads,
  substitutes nested generic parameters, predeclares cross-referenced ADTs,
  diagnoses unknown fields/variants and non-exhaustive enum matches, and bounds
  type text, nesting, fields, and variants.
- Non-generic calls and untyped function hoisting retain direct fast paths. The
  final seven-process legacy median is 0.266436 ms/check: 13.32% below the first
  recursive-type draft, while the remaining 7.94% over the prior bounded checker
  is recorded as feature cost. The dedicated benchmark reports p50/p95 and RSS.
- Added public constructors, differential/resource tests, benchmark evidence,
  and status documentation. All 44 self-host tests and the complete 677-test
  semantic suite pass; no Rust implementation was added.

### Self-hosting correctness and safety (type checker/frontend parity)
- Replaced the shallow self-hosted type checker with a bounded gradual checker
  that tracks lexical bindings, hoists function signatures, resolves scalar and
  nested-array annotations, checks assignments, defaults, call arity/arguments,
  and declared returns, and infers core collection/access expressions.
- Malformed caller-provided ASTs, including recognized nodes with missing or
  invalid required fields, and configurable node/depth exhaustion now return
  structured fail-closed diagnostics. The self-hosted frontend no longer
  swallows checker failures with `catch []`.
- Replaced `frontend_analyze` whitespace/semicolon substring heuristics with the
  real pblang tokenizer/parser. The previous heuristic falsely rejected its own
  source when it encountered the diagnostic fixture string `"= ;"`.
- Added focused parity/resource tests and a PBLang-only phase benchmark. The
  complete semantic suite now passes 677/677; all 44 self-host tests pass. On the
  pinned legacy-compatible fixture, the final fail-closed checker costs 0.2138
  ms more per check than the old shallow pass. Arity-specific schema validators
  reduced the allocation-heavy safety draft's median by 8.49%.

### Benchmarks and portability (language categories and platform contracts)
- Added the pure-pblang `language_benchmark.sapp` registry and runner with 15
  runtime definitions and correctness-gated startup, recursion, integer-loop,
  array, string, and hash-map fixtures. Subprocess timeouts/sample counts are
  bounded, caller-provided commands are rejected, and missing runtimes remain
  explicit in the output.
- Added the pure-pblang `platform_emulator.sapp` userspace/ABI model for
  validated Linux, Windows, macOS, Android, iOS, FreeBSD, and WASI version and
  architecture combinations. It provides bounded memory/files/output,
  sandboxed path normalization, deterministic services, and fail-closed
  portable syscall dispatch.
- Expanded the phase/competitor orchestration and emulator documentation. The
  local baseline produced 54 verified language/category rows, 36 explicit
  unavailable rows, zero failures, and eleven passing execution-model gates.

### Performance and correctness (tree-interpreter builtin resolution)
- The pure-pblang tree interpreter now performs a bounded, conservative binding
  scan while preparing an AST. Calls to a known builtin bypass repeated scope-
  chain lookup only when no variable, function, parameter, function-literal
  parameter, or loop binding can shadow that name. All ambiguous programs keep
  the previous dynamic lookup path.
- Prepared AST metadata is versioned so stale layouts are rebuilt. Focused
  tests cover all 16 interpreter builtins, global/local/function/parameter and
  loop shadowing, call-before-declaration behavior, non-identifier callees, and
  raw/stale prepared ASTs.
- Seven fresh CPU-pinned processes, each with alternating same-process dynamic
  and direct controls, measured median execution reductions of 7.87% for
  `len`, 6.95% for `map_has`, 7.42% for `str_starts_with`, and 11.52% for a
  mixed collection workload. The conservative compile scan added a measured
  median 0.0308 ms on the small `fib18` fixture. Absolute cross-process timing
  was rejected because CPU-frequency drift was visible.
- A numeric builtin-ID dispatcher was also measured and removed: its mostly
  0-2.4% changes were not stable enough to justify the extra implementation
  surface. Full semantic verification passed 675/675 tests.

### Performance and correctness (self-hosted interpreter block cache)
- The pure-pblang AST preparation pass now caches immutable statement arrays
  and declaration bits for `if`, `elif`, `else`, and `while` blocks. Repeated
  execution no longer rechecks block shape or rescans statements to decide
  whether lexical scope allocation is required.
- Public parser AST fields remain unchanged. Focused tests protect lexical
  shadowing, loop-local bindings, reusable/raw AST preparation, and the private
  cache contract.
- Two seven-process CPU-pinned A-B-A-B rounds reduced `fib18` execution by
  8.12% and 5.63%, `choose(15,7)` by 6.14% and 5.40%, and end-to-end `fib18`
  by 7.27% and 5.97%. Preparation increased by less than 0.81%; median RSS was
  effectively unchanged (+16 KiB, +0.05%).
- A shared return-sentinel candidate was rejected and removed after regressing
  recursive execution by 3.78-5.63%.

### Performance and safety (direct x86-64 linear recurrences)
- Added a fail-closed pure-pblang proof for the exact one-parameter additive
  recurrence with an `n < 2` identity base case. The accepted form emits a
  frame-free constant-space AMD64 loop; captures, changed guards/arguments,
  extra statements, and near matches retain the general recursive path.
- Checked-overflow mode deliberately disables the transformation, preserving
  recursive evaluation and trap ordering. Focused positive, swapped-term,
  near-match, and checked-mode tests protect the boundary.
- Seven alternating CPU-pinned `fib(36)` processes reduced median elapsed time
  from 49.738 to 1.466 ms (33.93x) and ELF size from 442 to 405 bytes (-8.37%)
  at the same 264 KiB peak RSS. Thirty compile samples reduced median codegen
  from 1.820 to 1.230 ms (-32.39%).

### Performance and memory (self-hosted VM direct builtins)
- Added the pure-pblang `compiler/vm_builtins.sapp` registry shared by bytecode
  codegen and the self-hosted VM. Direct calls to supported builtins carry
  optional private descriptor metadata while retaining the existing opcode and
  operand, preserving Rust VM and serialized-bytecode compatibility.
- The self-hosted VM reuses the private call-site descriptor only when no user
  global shadows the name. First-class builtin lookups still allocate
  independent mutable descriptors, and callee-before-argument evaluation is
  unchanged. The safe optimizer now preserves the optional metadata.
- Seven alternating same-process A/B runs over 10,000 calls show median
  throughput gains of 4.19% for `len` and 4.44% for `bit_shr`. Seven fresh
  processes reduce median peak RSS from 22,612 to 13,500 KiB (40.30%).

### Performance and safety (graph shortest paths)
- Added a reusable pure-pblang `min_priority_queue.sapp` with encapsulated
  binary-heap storage, O(log n) push/pop, O(1) peek, and fail-closed rejection
  of non-numeric, NaN, and infinite priorities.
- `graph.dijkstra` now shares the validated heap implementation used by
  `algorithms_advanced.dijkstra`, replacing the O(V^2+E) unvisited-node scan
  with O((V+E) log V) relaxation while preserving the existing unreachable
  distance sentinel. Negative and malformed edges remain rejected.
- Seven pinned release processes on a verified 600-edge sparse chain moved
  from a 443.201 ms median to 5.278 ms (83.97x, 98.81% lower). Median peak RSS
  moved from 11,820 to 12,068 KiB (+2.10%). `topological_sort` also uses a
  head-index queue and undirected cycle detection is iterative.

### Performance and correctness (graph algorithms)
- `graph.bfs` and `algorithms_advanced.bfs` now use head-index queues, changing
  wide traversal from O(V^2+E) queue copying to O(V+E). DFS uses constant-time
  `array_pop`; the advanced path is iterative and no longer consumes host call
  stack per vertex.
- Seven pinned release processes on a 2,000-leaf star moved from a 1592.228 ms
  median to 13.802 ms (115.36x); median process peak RSS moved from 15,624 to
  15,020 KiB. A permanent phase metric protects the traversal path.
- Reimplemented the previously untested `algorithms_advanced` module as valid,
  readable pblang. Its ten exports now have semantic coverage. Dijkstra uses a
  binary min-heap with correct repeated relaxation and rejects negative or
  malformed edges; 0/1 knapsack now uses O(capacity) memory.

### Performance (portable register-IR evaluator)
- `popr` now removes the final stack value in place with `array_pop` instead of
  allocating and copying the remaining stack on every operation. Empty-stack
  access still fails deterministically with an explicit underflow error.
- On seven pinned release processes, the 2,000-value push/drain fixture moved
  from a 1643.439 ms median to 10.910 ms (150.63x), while median peak RSS moved
  from 11,568 to 10,780 KiB. The permanent phase suite now covers this path.

### Performance (shared native IR)
- Backward liveness now removes dead pure `add`, `sub`, `mul`, and `addi`
  register writes, including inside structured loops and branches. Arithmetic
  feeding branch conditions, loop counters, loop-carried values, syscalls,
  stack operations, and local-store effects remains protected.
- On the pinned dead-arithmetic fixture, the optimized loop body shrank from
  five instructions to one. Final output fell by 10.0% on x86-64, 11.9% on
  i386, 9.5% on ARM64, and 24.1% for portable C99; corresponding native codegen
  medians improved by 19.1-19.5%.

### Performance (self-hosted bytecode VM)
- Replaced the per-call linear builtin-name membership chain with a registry
  built once when the pure-pblang VM module loads. Builtin descriptors remain
  independent mutable values, so first-class aliases preserve prior semantics.
- Matching one-, two-, and three-argument builtins consume scalar stack values
  directly instead of allocating and populating a temporary argument array.
  User closures and unusual arities retain the generic path.
- Seven pinned release-process medians for 10,000 calls improved from
  215.466 to 207.677 ms for `len` (3.62%) and from 246.038 to 197.581 ms for
  `bit_shr` (19.69%). Permanent phase metrics and regression tests cover both.

### Performance (self-hosted interpreter)
- Added `compile_program(source)` and `eval_ast(ast)` so callers can prepare a
  validated AST once and execute it repeatedly. The phase suite now reports
  front-end preparation, execution-only, and end-to-end interpreter costs.
- One- and two-argument builtins use scalar dispatch for their matching arity,
  avoiding a temporary array for common `len`, `str`, `map_has`, `push`, and
  string operations. The pinned 5,000-call `len` workload improved by 3.85%.
- Two-argument user calls now evaluate into scalar locals and bind exact
  two-parameter closures directly, avoiding the temporary argument array and
  generic parameter loop on recursive numeric workloads. Missing/extra
  parameter behavior and argument-before-callee order remain unchanged.
- The phase benchmark now reports `tree_interpreter_choose15_7` alongside the
  one-argument Fibonacci control. A pinned, normalized A/B reduced the
  two-argument/control ratio by 11.5%; raw samples and method are recorded in
  `bench/results/2026-07-13-tree-interpreter-apply2/`.

### Fixed (CLI dispatch and REPL lifecycle)
- Bare `.sap`/`.sapp`/`.bpx`/`.pbck` paths and package directories now dispatch
  as the short form of `pblang run`, including when capability flags precede
  the path. They no longer fall through into the REPL.
- The REPL now exits on stdin EOF (`read_line` returning zero) and emits its
  banner/prompts only on an interactive terminal. Closed stdin can no longer
  create an unbounded prompt stream; piped source still executes normally.
- Unknown commands and non-runnable paths now fail with exit code 2 and a run
  hint instead of silently opening the REPL.

### Fixed (native c-transpiler — semantic parity with the VM)
- **`str_split(s, "")` (empty delimiter) now splits into individual UTF-8 codepoints** instead of
  returning an empty array. This unblocked the self-hosting tokenizer (which does
  `str_split(source, "")`); the native binary now produces byte-identical tokens to the interpreter.
- **`exec(cmd)` now returns the process EXIT CODE (a number), not captured stdout**, and matches the
  VM's hardening: it rejects whitespace + shell metacharacters (single-token commands only — use
  `proc_spawn_wait` for arguments) and runs the binary directly via `execvp` (no shell → no
  injection) with inherited stdio. The previous string return made `exec(cmd) + n` throw
  "type error in +" in native binaries.
- **User-level `m[k]` and `m.field` now THROW on a missing map key** (and `a[i]` on an
  out-of-bounds array index), matching the VM's `undefined key '<k>'` / `array index out of bounds`.
  They had silently returned `nil`, which broke the `try m[k] catch fallback` idiom natively. The
  safe operators `?.` / `?[]` and internal `pb_map_get` stay nil-returning.
- **`str()` now recursively renders arrays and maps** (`[map{k: v}, …]`, strings bare, insertion
  order) instead of `[array]` / `[map]` placeholders, so `str_contains(str(container), …)` works.

### Added
- **Native c-transpiler builtins:** type predicates `is_map`/`is_array`/`is_string`/`is_number`/
  `is_bool`/`is_nil`/`is_fn`/`is_int`/`is_float`, `typeof`, `platform`, `now_iso`, `exec`,
  `proc_spawn_wait` (POSIX fork/exec → `map{exit_code,stdout,stderr}`), `slice(seq,start,end)`
  (polymorphic string/array), and the tensor ops `tensor_sub`/`tensor_scale`/`tensor_sum`/
  `tensor_dot`/`tensor_relu`/`tensor_softmax`/`tensor_gelu`/`tensor_backend` (GELU bit-identical to
  the VM). Native builtin coverage: 107 mappings.
- **`stdlib/source_inliner.sapp`: bare `stdlib/X` imports resolve against `$PBLANG_STDLIB_DIR`**
  (slash form, mirroring the `stdlib:` alias and the VM's own rooting), so multi-file programs that
  pull the self-hosting compiler modules link. (Project-local `stdlib/` still shadows it.)
- **`stdlib/source_inliner.sapp`: a top-level def that shadows a builtin is now prefixed, not
  rejected** — the prefixed inliner renames only in-module references, so modules like the
  self-hosting tokenizer (which defines `fn push`) link natively. Result: **the pblang tokenizer
  compiles to a native binary that tokenizes identically to the interpreter.**
- **`stdlib/sha256_pb.sapp` — a pure-pblang SHA-256** (FIPS 180-4) built only on the 32-bit
  bitwise builtins + `str_to_bytes`, no dependency on the native `sha256` builtin. Byte-for-byte
  identical to the builtin across empty/`abc`/`hello`, the 55/56/64-byte block+padding boundaries,
  and multi-block input (`tests/semantics/sha256_pb_test.sapp`). A step toward reimplementing the
  Rust builtin surface in pblang. (Note: it native-compiles only once the C-transpiler runtime
  grows `bit_*`/`str_to_bytes` — see Known limitations.)
- **`stdlib/source_inliner.sapp`: `inline_source_prefixed` / `inline_file_prefixed`** — a
  scope-aware, per-module *prefixed* source linker so multi-file programs whose modules define
  **colliding top-level names** (e.g. several modules each defining `new`) can be flattened into
  one self-contained source and native-compiled. Renaming is shadow-safe (a local `let`/param that
  shadows a top-level name is left untouched via scope tracking) — no silent miscompilation.
  `scripts/compile_native.sh` now uses it; verified end-to-end (a colliding-`new` two-module
  program compiles to a correct native binary; pbos's 4-module monitor test flattens + runs).
  Regression test `tests/semantics/source_inliner_prefixed_test.sapp`.

### Added — native tensor kernels + concurrency
- **`tensor_matmul`, `tensor_argmax`, `tensor_add`** in the C runtime (over the `map{shape,data}`
  tensor format) — native == interpreter (verified `[[1,2,3],[4,5,6]]·[[7,8],[9,10],[11,12]]` =
  `[[58,64],[139,154]]`). **`spawn`/`join_all`** as **sequential** native emulation (run the thunk
  now, return results — correct output, no real OS threads; parallel best-of-N training runs
  serially). This was the last piece: **pbai's sampling/LM test now native-compiles and runs
  (`pbai_sampling_pass`)** — all 19 sampled pbos+pbai tests now compile natively and pass (pbai_cache
  needs a writable CWD for its relative `target/pbai-cache` path). Regression:
  `ctranspiler_runtime_tensor_matmul_argmax_and_spawn`.

### Fixed — native (C-transpiler) codegen + runtime
- **Local variables that shadow a builtin now native-compile.** A `let sum = 0; sum = sum + e`
  emitted the builtin function pointer `pb_builtin_sum` instead of the variable (C type error) —
  the identifier emitter checked the builtin table before scope. Scope (local + module global) is
  now checked first, matching the VM. Affects any local named `sum`/`min`/`max`/`clamp`/etc.
- **Function parameters named like a positional placeholder now native-compile.** C params were
  named `a0,a1,…`, so a pblang param named `a1` (common in neural code) was "redeclared". Renamed
  the placeholders to `__pba{i}`, which user identifiers can't collide with. Regression:
  `ctranspiler_runtime_param_and_builtin_name_shadowing`.
- **`try/catch` now catches thrown errors in native binaries.** It was transpiled to `pb_try(EXPR,
  C)`; C's eager argument evaluation ran a throwing try-expr before any `setjmp` was installed, so
  the throw escaped the catch. Now emitted as an inline `setjmp` block (evaluate the try-expr only
  after installing the handler; on `longjmp`/error-value run the catch). Fixed **all 6** native
  runtime divergences in the pbos+pbai sweep at once (now 18/19 native-correct, was 12/19).
  Regression: `tests/ctranspiler_runtime.rs::ctranspiler_runtime_try_catch_catches_a_thrown_error`.
- **Module-level `let`/`const` now native-compile.** File-scope bindings were emitted as locals in
  `main`, so top-level functions referencing them failed with `undeclared` in C — a blocker for
  almost all real modules (constants used across functions). They are now emitted as C globals
  (declared up top, initialized in order in `main`). Scope-aware: a `let` **inside** a function/block
  stays local and correctly shadows the module global (verified: `calc(){ let BASE=x; }` uses its
  local while `plain()` uses the global). No regression (fib/greet/pbos-monitor native unchanged).
- **Filesystem + env builtins in the native runtime:** `fs_read`/`fs_write`/`fs_append`/`fs_exists`/
  `fs_remove`/`fs_read_bytes`/`fs_write_bytes`/`fs_mkdirs`/`fs_listdir`, `bytes_to_str`, `env_get`
  (were undeclared → compile errors). Native binaries have no capability sandbox — these are raw
  file I/O. Combined effect: **`pbai_cache_test` now compiles to a native binary that runs correctly
  (`pbai_cache_pass`)**, and pbos's security monitor still runs native (`monitor_pass`).

### Fixed — native (C-transpiler) runtime builtins
- **Real SHA-256 in native binaries.** `pb_builtin_sha256` was a zero-returning stub — any
  `sha256`-using program (audit_log/merkle/crypto/JWT) silently miscomputed when native-compiled.
  Replaced with a full FIPS 180-4 implementation in `pblang_rt.h`; native `sha256` now equals the
  interpreter (canonical `sha256("abc")` verified).
- **Added `bit_and`/`bit_or`/`bit_xor`/`bit_shl`/`bit_shr`/`bit_not` and `str_to_bytes`** to the
  C-transpiler runtime + builtin map (were undeclared → compile errors). So bit-ops and byte access
  now native-compile. Net effect: the whole pbos security monitor (4 modules + a SHA-256
  tamper-evident chain, flattened via the prefixed linker) compiles to a **native binary that runs
  correctly** (`monitor_pass`). Regression test:
  `tests/ctranspiler_runtime.rs::ctranspiler_runtime_sha256_bitops_and_str_to_bytes_work`.
  (`char_code`/`ord` still absent; `--llvm`/`--backend cranelift` still fall back to C99.)

### Fixed
- **`stdlib/docs_generator.sapp`: runtime crash in HTML generation.** `generate_html` and the
  index page concatenated a bare number (`len(functions)` / `len(results)`) into an HTML string
  with `+`; since pblang's `+` does not coerce number→string, both threw
  "type mismatch: string + number" the moment the generator ran (the module still *imported*, as
  the fault was inside a function body). Wrapped both in `str(...)`. Regression test:
  `tests/semantics/docs_generator_test.sapp`. (Found by a `pblang --check` sweep of all 302
  stdlib modules — which otherwise came back parse-clean; the remaining `--check` diagnostics are
  benign gradual-typing flow conservatism, verified by importing+running the affected modules.)

### Added
- **pbide GPU render M3 persistent buffers (`wgpu-render`):** the wgpu backend no longer
  allocates a fresh vertex buffer per frame — rect and glyph instances go into reused, grow-on-
  demand `VERTEX|COPY_DST` buffers written with `queue.write_buffer`, so steady-state per-frame
  GPU allocation is zero. (Damage/scissor is the remaining M3 item.)
- **pbide GPU text (M2, `wgpu-render`):** the wgpu render backend now draws text on the GPU via
  an R8 glyph-atlas + instanced text pipeline (shelf packer, `(font,char,px)` cache, samples
  coverage as alpha). Reuses the existing fontdue rasterizer (`rasterize_glyph_for_atlas`) so GPU
  and CPU text are pixel-identical; `gfxdrawtextaa` routes to it and returns real `TextDrawStats`.
  Compiler-verified end-to-end (`cargo check --features gpu-render`); frame-testing on a real GPU
  pending. See `docs/GPU_RENDER_PLAN.md` (M2).
- **`gpu-render` / `gpu-render-shared` cargo features** (off by default): enable the
  experimental wgpu-24 GPU render backend for the pbide window from the top-level `pblang`
  binary (`cargo build --features gpu-render`, select at runtime with `PBIDE_GRAPHICS=wgpu`).
  M1 (surface + swapchain + instanced solid-rect pipeline in
  `crates/pblang-graphics/src/wgpu_backend.rs`) is now compiler-verified against wgpu 24 +
  raw-window-handle 0.6; the minifb-0.28 handle path builds with no winit shim. Runtime
  frame-testing and M2 (glyph-atlas text) remain. With the feature off, the default build is
  byte-for-byte unchanged. See `docs/GPU_RENDER_PLAN.md`.

- `stdlib/merkle.sapp`: a binary Merkle tree over SHA-256 hex digests with
  RFC-6962-style domain separation and O(log n) inclusion proofs, built only on
  the existing `sha256` builtin (64-char lowercase hex) and `constant_time_eq` —
  - `leaf_hash(data)` = `sha256("L:" + data)` and `node_hash(l, r)` =
    `sha256("N:" + l + r)`. The disjoint `"L:"`/`"N:"` domain-tag prefixes stand
    in for RFC 6962's `0x00`/`0x01` (inputs are strings) and defend against
    second-preimage attacks; because children are fixed-width 64-hex digests the
    `"N:"` input is unambiguous.
  - `build(items)` (O(n), stores every level bottom-up), `root(tree)`,
    `prove(tree, index)` (O(log n) `{hash, side}` sibling steps), and
    `verify(root_hex, data, index, proof)` (constant-time compare).
  - Odd levels DUPLICATE the lone final node (Bitcoin convention), applied
    consistently in build/prove/verify. The canonical empty-tree root is
    `sha256("L:")`. New test: `tests/semantics/merkle_test.sapp`.
- `stdlib/security.sapp`: standards-based 2FA and password-hardening primitives,
  built only on the existing `hmac_sha256` builtin —
  - `pbkdf2_sha256(password, salt, iterations, dklen)` (RFC 2898/8018 F-function /
    U-cascade over HMAC-SHA256), plus `hash_password_pbkdf2(password, iterations)`
    producing a self-describing `pbkdf2_sha256$<iters>$<salt_hex>$<dk_hex>` string
    and `verify_password_pbkdf2(password, stored)` (constant-time). Recommended over
    the legacy `hash_password`/`verify_password` for new code.
  - `hotp(secret, counter, digits)` (RFC 4226 dynamic truncation) and
    `totp(secret, unix_time, step, digits)` / `totp_now(secret, …)` /
    `totp_verify(secret, code, unix_time, step, digits, skew)` (RFC 6238). HOTP is
    built over HMAC-SHA256 (not the classic SHA-1) since that is the only HMAC
    builtin — an RFC-6238-sanctioned variant; codes will not match SHA-1 apps.
  - `base32_decode`/`base32_encode` (RFC 4648) so authenticator-app secrets
    interoperate. New test: `tests/semantics/security_2fa_test.sapp`.

### Added (experimental)
- graphics: experimental, off-by-default `wgpu-render` backend scaffold (surface +
  cleared frame + solid rects) + shared wgpu device accessor; see
  docs/GPU_RENDER_PLAN.md. Not yet built — requires `--features wgpu-render`.

### Fixed (robustness)
- `crates/pblang-vm/src/vm/http.rs`: `http_get_auth()` / `http_post_auth()` no
  longer swallow request failures behind `unwrap_or_default()` /
  `Ok("error: …")`. They now surface errors as a thrown `RuntimeError` (via
  `host.err`), matching `http_get`/`http_post`. Callers that already wrap these
  in `try … catch` (stdlib `ai.sapp`, `http.sapp`) now correctly see failures
  instead of parsing a bogus success string.
- `crates/pblang-vm/src/vm/ffi.rs` + `ffi_dispatch.rs`: FFI symbol-resolution
  and library-load errors now include the symbol name and library path being
  searched, instead of a bare libloading message. Unsafe loading contract and
  capability gating are unchanged (message only).
- `src/main.rs` (`--ide` dispatch): a failing `set_current_dir` to the repo
  root is now logged as a clear warning (previously silently ignored via
  `let _ =`); behaviour is otherwise unchanged and non-fatal.

## [0.5.7] — 2026

### Added
- `pblang_ir::serialize_chunk_pbck()` — single canonical PBCK bytecode serializer shared by CLI and Cranelift.
- Semantics tests: `thread_pool_module_test.sapp`, `build_stdlib_test.sapp`.
- Fixed `stdlib/build.sapp`: valid `export` syntax and `host_platform()` via `stdlib:os.sapp` (was invalid `export fn` and undefined `os()`).
- `[workspace.dependencies]` in root `Cargo.toml` for shared crate versions.

### Changed
- Workspace crate versions aligned to **0.5.7** (was 0.5.0 in manifests while docs referenced 0.5.6+).
- Removed ~180 lines of duplicate PBCK serialization from `src/main.rs` and `pblang-cranelift`.

### Added (accumulated)
- `tests/semantics/memory_stdlib_test.sapp` — covers `stdlib/memory.sapp`
  convenience API (`memory.box`, `read`, `write`, `borrow`, `release`,
  `free`, `stats`).
- `CHANGELOG.md` for tracking changes across releases.
- Self-hosting compiler modules: `stdlib/compiler/{ir,optimizer,bootstrap}.sapp`.
- Memory safety stdlib wrapper: `stdlib/memory.sapp` exposing the runtime
  `memory_*` builtins behind a Rust-flavoured API.
- `stdlib/thread_pool.sapp` — enhanced thread-pool helpers.
- `stdlib/registry.sapp` — high-level package-registry operations.

### Changed
- `crates/pblang-vm/src/concurrency.rs` `VmThreadSupervisor` now polls the
  join handle until completion **or** the deadline passes, so `timed_out`
  statistics reflect real timeouts instead of blocking forever on `join()`.
  Added `Default` impl and removed unused struct fields.
- `src/main.rs` hoists the Cranelift AOT helpers
  (`bincode_serialize_chunk`, `generate_c_stub_with_bytecode`) to a single
  module-level definition. The previous four duplicated copies have been
  removed.
- `stdlib/memory.sapp` documentation now reflects the actual runtime
  behaviour: prefer the top-level `memory.box/read/write/...` helpers
  because method-style dispatch does not auto-bind `self`.
- `STRUCTURE.md` rewritten to cover all 16 workspace crates and the full
  stdlib layout, including self-hosting compiler modules.
- `README.md` feature list now lists runtime memory safety, concurrency
  primitives, async/await, multi-backend codegen, AI/quantum integration,
  TLS, native binaries, and self-hosting.
- `docs/CURRENT_STATUS.md` and `docs/Tasks.md` test counts updated.

### Fixed
- Removed `unreachable_patterns` warnings in
  `src/main.rs` and `crates/pblang-cranelift/src/lib.rs` (the `Obj` enum
  has a single variant, so a fall-through arm is dead).
- Removed `unused_variables` warnings (`_arity`, `_use_i64`, `_obj_path`).
- Marked `pblang_cranelift::serialize_chunk_to_vec` with `#[allow(dead_code)]`
  + doc comment until a public caller is wired up.

## [0.5.6] — 2026

### Added
- **AI integration** (`pblang-ai` crate, `ai_chat`, `ai_embed`,
  `ai_token_count`, provider auto-registration, streaming, RAG helpers).
- **Self-hosting type checker** completes the 4-stage pipeline.
- **Bit-level optimizer**: string constant folding, peephole, jump
  threading, native bitwise opcodes.
- **New stdlib modules**: `bitops`, `compression`, `serialization`,
  `machine_learning`, `blockchain`, `networking_advanced`,
  `data_structures`, `image_processing`, `algorithms_advanced`.
- **LSP**: goto-definition, hover with 40+ builtin signatures, completions.

## [0.5.2] — 2026

### Added
- Concurrency primitives: `SafeMutex`, `SafeRwLock`, `SafeChannel`,
  `ThreadPool`, `Barrier`, `Once`, `Timer`, `WaitGroup`, `Semaphore`,
  `CircuitBreaker`, `RateLimiter`, `FanOut`/`FanIn`.

## [0.5.1] — 2026 — Security Hardening

### Fixed
- 🔴 **Critical**: TLS `DangerousAllowAllVerifier` replaced with
  webpki-roots + rustls-native-certs.
- 🟠 **High**: Path traversal in `static_file_response()` (`_path_safe`
  strips `../`, URL-decodes, returns 403).
- 🟠 **High**: `fs.sapp` now blocks absolute paths and `../` by default;
  opt-in `*_abs()` variants.
- 🟡 **Medium**: Command injection in `os.which()` — replaced with safe
  PATH search builtin.
- 🟡 **Medium**: URL query parameters now URL-decoded.
- 🟡 **Medium**: Full RFC 7231 HTTP status code table.
- 🟡 **Medium**: `NonNull` (`T!`) type preserved in typechecker.

## [0.5.0]

Initial public release.

### Highlights
- Stack-based bytecode VM with RC + cycle-breaking GC.
- Capability-based sandboxing (`VmLimits`, `FfiPolicy`, `StdlibConfig`).
- Package manager with semver, lockfiles, SHA256 verification.
- 50+ stdlib modules.
- VM-per-thread concurrency with typed channels and `select`.
- Polyglot blocks (`@python`, `@sql`, `@rust`).
- FFI to C libraries with allowlist security.
- LSP server, REPL, full CLI toolchain.
