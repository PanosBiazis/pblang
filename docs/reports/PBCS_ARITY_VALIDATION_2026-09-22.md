# PBCS function-arity preservation — local validation

Baseline: `125152fe86631fde54c773f785506cd345907e68`. All product changes in
PBLang. The retained host executable is identical on both sides. Tests/builds
ran in disposable clean archives with only this change overlaid; 212 unrelated
working-tree files were preserved. No EnterpriseCore server, remote test machine,
desktop service or installed PB-MCP configuration was used or changed.

## Reproduced defect and correction

The valid source `fn add(a,b=2) { return a+b; } print add(3);` compiles successfully.
On baseline, direct native PBCK translation prints `5`, but translation after
PBCS serialization/reload fails with an arity error (expected at least 2, got 1).
With v3, both native artifacts print `5`. The new semantic regression test also
fails on baseline at metadata presence when its version assertion is omitted.

The writer now retains optional `min_arity`. The reader validates its presence
flag and range; v1/v2 remain readable without synthesizing missing metadata.
See [format and migration contract](PBCS_FORMAT.md). A baseline reader was also
executed against the new file and rejected v3, as required. Upgrade bundled
readers before publishing new artifacts. This is a documented container-version
change, not a transparent byte-compatible writer update.

## Validation

- **39 suites passed**: codec, loaders, bundlers, native translation, cache,
  defaults, closures, try/catch, optimizer, VM admission/budgets and five emulator
  suites. These are model/emulator checks, not tests inside all guest operating
  systems or certification of physical devices.
- New suite compares plain and optimized source paths, byte-stable roundtrips,
  exact native translation bytes, VM and loader outputs. Raw IR deliberately
  exercises missing-argument rejection through zero/one/two/generic call paths
  independently of the static frontend checker.
- Negative coverage: negative/fractional/oversized/string/boolean/nil minima,
  invalid presence byte, encoded minimum greater than total arity, truncated
  minimum field, legacy function metadata absence. Existing malformed-container
  and runtime-budget suites pass.
- Actual native default-argument execution before/after, plus old-reader rejection,
  is recorded separately. No Rust source, VM opcodes, per-instruction accounting,
  application limits or permission policy changed.

Evidence: [suite results](evidence/pbcs-arity-validation-2026-09-22.json),
[native comparison](evidence/pbcs-arity-native-2026-09-22.json).

The focused review checked presence versus zero, numeric validation, nested
function/try traversal, v1/v2 layout dispatch, truncation, byte-count growth and
cache fingerprint inclusion. This is not a complete bytecode verifier or an
independent security audit. Whole-file read allocation, unsupported VM entry
paths and general malicious-program isolation remain outside this fix.

## Reproducible cost measurement

Run `target/release/pblang run bench/selfhost_pbcs_arity.sapp --allow-all --
functions 600` (one line), or replace `functions` with `plain`. Each iteration
serializes, deserializes and runs the same compiled chunk, verifying output.
Two warmups per side/workload, seven samples per side in alternating order;
CPU is user+system time, RSS is process peak including host and compilation.
Nearest-rank p95 at seven samples equals the maximum; it is not a reliable tail
latency estimate. No other validation/Graphify job ran during this measurement.

| Workload | CPU median before → after | CPU p95 before → after | RSS median KiB before → after | Artifact bytes before → after |
| --- | --- | --- | --- | --- |
| Three function constants | 2.17 → 2.23 s | 2.25 → 2.27 s | 39,028 → 39,716 | 462 → 477 |
| No function constants | 1.10 → 1.11 s | 1.11 → 1.36 s | 36,680 → 36,496 | 109 → 109 |

The function workload costs about **2.8% more CPU and 1.8% more process peak RSS**.
The control median is about +0.9% CPU / −0.5% RSS with a slower outlier. Raw
[samples and statistics](evidence/pbcs-arity-cost-2026-09-22.json) retain the
variation. These results do not demonstrate a speedup or general RAM reduction.
The byte cost is exactly 5 per function with a minimum, 1 when absent. This cost
belongs to preserving a previously lost contract; no extra instruction-level
check was added. It does not establish unchanged performance on every workload.

## Remaining work and stable release

Scalar shared-capture semantics, large GC pauses, the rooted collector regression,
general runtime/FFI/polyglot integration, whole-program resource lifetime and
broader platform verification remain open. This does not speed up live streaming
or update installed PB-MCP. The allocator remains unchanged.

The [1.0 criteria and planning estimate](STABLE_1_0_READINESS.md) remain applicable:
there is no reliable release date. The earlier 6–12 months for a clearly scoped
stable release and 12–24+ months for the broader product/platform goal are
low-confidence planning ranges under the documented staffing/test assumptions,
not a delivery promise. Version stays `0.6.0 alpha`.

Final local gates: build passed and its generated lock was normalized with
`tools/pblock.sapp` in the clean test tree. Lint exited successfully with five
warnings: two existing NaN self-comparisons and three variables used through
expressions that this lint reports as unused. No warning was suppressed.
Touched-file privacy/whitespace checks passed. Graphify update exceeded a
35-second limit and was terminated; no semantic `.sapp` graph coverage is claimed.
