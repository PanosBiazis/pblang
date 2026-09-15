# Owned callback cleanup and idea conversion — 2026-09-15

## Implemented

`memory_owned.sapp` adds optional automatic cleanup of one scalar owner around
a synchronous callback, including caught failure. It checks the borrow lifetime
by depth and token, attempts one explicit free, and exposes separate allocation,
body and cleanup results. Ordinary memory imports do not load this module.
`SafeRef` now uses the correct owner read, starts inactive, rejects reacquisition
and explicit inactive access, and fails clearly for unsupported borrowed writes.
The [API contract](MEMORY_OWNED_CALLBACKS.md) states supported routes and limits.
No Rust source or native executable was changed.

## Correctness and security

38 suites passed on local Linux: 21 memory/compiler/VM suites and 17 emulator,
device and input-model suites. The new memory tests execute on the retained
host and again as guest source through the PBLang VM. They cover nil/false
results, callback failure, mutable/immutable access, nested helpers, manual free,
unsupported initial composites, non-callable callbacks, premature pop,
replacement lifetime tokens, extra pushes, repeated failures, bounded VM use,
SafeRef fresh/repeated release, acquisition rejection and final owner/borrow
counts. Caller scope preservation and zero retained owners are asserted.

The original SafeRef baseline failed on release before acquire. Separate old
setter probes on both execution routes failed at existing arity checks before
memory_set; no cross-owner write was reproduced. New tests ensure both target
and unrelated owners stay unchanged on rejected writes.

Independent security review found no blocker for the documented scalar-only,
trusted-callback scope. It also found source-level problems in retained native
composite alias bookkeeping: failed store/free update the alias map before
success, and multiple owners share a single pointer-to-owner entry. These are
not fixed or certified here. Initial composite values are rejected, and native
raw replacement with composites is outside this helper's supported contract.
There is no claim of a sandbox, opaque handles or full native memory safety.

Eight touched PBLang files passed self-hosted checks, retained lint and strict
entry-only scans. Lint may report nonblocking style/unused-variable warnings;
the scans are not dependency-wide semantic proofs. The example returns 42 and
a zero owner-count delta. Build, portable lock, privacy and diff checks are
recorded with publication. No server or remote workstation testing occurred.

Graphify AST update completed in an isolated snapshot: 523 supported files,
8,814 nodes and 12,941 edges. Its installed extractor does not parse `.sapp`;
this is not a complete graph of the new library. The main graph was preserved.

## Measurements

Baseline source revision: `db1d00d84d2028cb4da37b0addfb10600c1d010e`.
Retained executable SHA-256:
`7f1b97c0037736553ab39d165c13c36f3fd8497bc6aca9ca94a4c680858e6500`.
All measurements use the same Linux x86-64 machine, executable and 20,000-call
workload. There is one whole-process warmup, then five measured processes per
variant with alternating order. Each process internally warms the same 20,000
calls before timing them. GNU time collects user+system CPU and peak RSS; p95 is
nearest rank, hence the maximum of five samples.

For fair imports, baseline/current memory modules and before/after helper
modules were loaded from equivalent temporary relative source paths. This
avoids attributing differences in package/hash admission to the loop itself.
The script and expected checksum/count checks were otherwise identical.
The pre-optimization helper is retained under
`bench/reference/memory_owned_map_results.sapp` for reproduction only.

| Variant | Loop median / p95 ms | Process CPU median / p95 s | Peak RSS median / p95 KiB |
| --- | ---: | ---: | ---: |
| Baseline explicit cleanup | 83.16 / 86.42 | 0.19 / 0.20 | 21940 / 22044 |
| Current explicit cleanup | 74.39 / 93.56 | 0.17 / 0.21 | 22052 / 22192 |
| First automatic implementation | 183.43 / 275.47 | 0.36 / 0.57 | 40796 / 40908 |
| Optimized automatic implementation | 109.75 / 181.53 | 0.24 / 0.40 | 32256 / 32376 |

The optimization replaces private intermediate maps with compact tuples and
avoids constructing a public result map for throwing helpers. All cleanup
checks remain. Against the first automatic implementation, median peak RSS fell
20.9%, loop time 40.2% and process CPU 33.3% on this workload. Every paired
before/after automatic sample improved, but substantial desktop timing variance
remains. These numbers are not a language-wide speedup or power measurement.

Automatic cleanup still costs more than explicit manual cleanup: the measured
optimized peak RSS is about 47.1% higher and loop median about 47.5% higher than
current manual cleanup. It is opt-in and must not be sold as zero overhead.
Current manual median improved but its p95 worsened and RSS rose about 0.5%;
there is no demonstrated broad manual-path speedup. Aggregate disk writes,
GPU/NPU use, energy and non-Linux platforms were not measured in this slice.
The depth check's use of native memory_stats can also scale with ambient
borrow/reference metadata, so an inexpensive lifetime-depth primitive remains
a performance follow-up.

[Raw samples, suite markers and gates](evidence/owned-callback-2026-09-15.json)
contain no personal paths. Reproduction entrypoint:

```sh
target/release/pblang run bench/memory_owned_callback.sapp --allow-all -- manual
target/release/pblang run bench/memory_owned_callback.sapp --allow-all -- automatic
```

## Idea intake and 1.0.0

The [discovery record](IDEA_INTAKE_2026-09-15.md) extends the source inventory
with a prior PB-MCP/PBRemote discussion, local Codex/Claude plugin metadata,
available and recommended-but-not-installed tools, and official Claude,
Gemini CLI, MCP Registry and VS Code extension documentation. No plugin was
installed or executed for discovery. Two candidate rows became tested code:
owned callback cleanup and explicit guard repair. Other rows remain proposals.
The [bounded scan counts](evidence/idea-intake-2026-09-15.json) are lexical
selection evidence, not a complete review of every conversation or mounted disk.

1.0.0 is not complete. Priority gaps remain composite ownership/aliases,
borrow-token operations, deterministic fatal-error cleanup, native/FFI and
backend parity, cumulative bounded import accounting, authenticated audits and
releases, PBIDE workflows and guest/legacy-device validation. Cross-product
session cleanup, declarative extension discovery, document transformations and
streaming/resource helpers need their own implementations and tests. No
production-readiness or universal-device claim follows from these local suites.
