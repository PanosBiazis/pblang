# File Catalog self-hosted summary validation — 2026-09-17

## Delivered behavior

Full metadata summaries and visualization plans now pass the same assertion
fixture in the retained executable and self-hosted frontend. The previous
failure had two causes: missing option attributes return `nil` in the VM, so
`try/catch` did not apply defaults; and the VM lacked `array_sort`. Presence
checks now default only absent options. Explicit invalid values still fail.
Indexed histogram reads already preserve the required missing-key exception
behavior, so their original update path is retained rather than paying for
additional presence checks per item. `scan_directory` shares option handling;
its filesystem execution is still a retained-host boundary.

[The sort bridge](SELFHOST_ARRAY_SORT.md) preserves one-argument arity,
retained ordering, detached outer arrays, nested aliasing and builtin shadows.
Existing bounded execution checks the input size before sorting. This does not
provide hard host-sort time limits, complete heap accounting or native isolation.
There is no new general language/compiler/project size or runtime limit.

Basename extraction keeps the existing portable separator and drive-prefix
rules. A 64-character tail handles short names. For longer names, a single
remaining component can be returned directly; otherwise the tail doubles until
the final separator is present. The complete admitted name is preserved.
No suffix label, rule ordering or special-name precedence changes.

## Validation

- 45 selected suites passed, including 15 emulator suites, compiler/VM,
  memory/cache regressions, all 151 catalog rules, portable names, and dashboard
  output records. This is not the full repository suite.
- After the last basename refinement, all six catalog-dependent suites and the
  catalog's three source gates were rerun successfully. Other tested VM/registry
  code was unchanged by that refinement.
- The shared fixture checks totals, sorted rows, empty/partial options, explicit
  nil rejection, input preservation, aggregate privacy, basename disclosure,
  malformed collections and portable visualization geometry.
- Portable-name tests cover ASCII and Greek basenames around 64/128/256
  boundaries and up to 4,080 characters, with POSIX/Windows/mixed prefixes.
- Two pure-catalog checks passed with filesystem/network/HTTP/exec/FFI denied.
  Source import remains its separate loader boundary.
- The dashboard test generated web, native and Android records plus a local
  HTTP response object. No server was deployed or interactive device tested.
- The WASM suite executed generated modules in Node/V8. Its initial harness
  mistakenly expected the no-Node success message; the exit-zero log confirmed
  the real-engine success branch. The test itself was not weakened or changed.
- 27 gates passed (selfhost check, lint, strict entry-only pbsecure on nine
  touched PBLang files). Automated analysis is bounded, not a proof of safety.
  The requested independent security delegation could not start because the
  specialized model was unavailable to this account. No independent approval
  is claimed. No authentication, sandbox or release-trust policy was changed.
- Final retained build and native portable-lock publication passed. The dashboard
  still requires a scratch nested build for its dependency lock; that generated
  lock is not published.

[Exact suite/gate list](FILE_CATALOG_SUMMARY_TESTS_2026-09-17.json).

## Resource measurements

Baseline `c3a1842`; identical retained executable and workloads, one warmup,
seven alternating process samples per variant. Timed catalog work excludes
import/index/input setup. GNU time process CPU/RSS include setup. No tests or
Graphify overlapped the measurements; unrelated laptop activity was uncontrolled.
P95 is the maximum of seven samples. The same frozen classifier driver is used
for both module copies, and checksums agree. The summary driver repeats 300
metadata names ten times. This compares retained execution; the newly working
self-hosted summary had no successful baseline to compare against.

| Corpus | Repetitions | Before median / p95 ms | After median / p95 ms | Median change | Before / after RSS KiB |
| --- | --- | --- | --- | --- | --- |
| mixed | 200 | 11.969 / 13.090 | 12.285 / 12.850 | +2.64% | 32296 / 32152 |
| early | 300 | 7.151 / 7.713 | 7.223 / 7.968 | +1.01% | 32124 / 32116 |
| miss | 200 | 9.290 / 9.767 | 9.419 / 10.644 | +1.40% | 32152 / 32184 |
| long | 50 | 2.643 / 2.877 | 2.679 / 3.263 | +1.36% | 32320 / 32168 |
| dots | 20 | 0.801 / 1.172 | 0.806 / 0.822 | +0.57% | 32208 / 32120 |
| separators | 20 | 0.791 / 0.838 | 0.823 / 0.866 | +4.10% | 32272 / 32224 |
| backslashes | 20 | 0.848 / 1.010 | 0.814 / 1.216 | -4.02% | 32216 / 32300 |
| fallback | 20 | 6.301 / 6.615 | 0.901 / 1.438 | -85.70% | 32132 / 32216 |
| summary | 10 | 30.661 / 49.552 | 31.034 / 33.667 | +1.22% | 32152 / 32144 |
| basename-long | 20 | 2.122 / 2.432 | 2.287 / 2.320 | +7.76% | 32280 / 32248 |

[Raw samples and hashes](FILE_CATALOG_SUMMARY_BENCHMARK_2026-09-17.json).

`fallback` includes long names after thousands of separators; `basename-long`
adds ASCII/Greek names around 4,000 characters after a single directory prefix.
These cases are both retained so the favorable result cannot hide a regression.
Small intervals are noisy. No universal speedup, zero overhead, general RAM
saving, disk-write/executable-size reduction or GPU/NPU gain is established.
The retained executable is unchanged. The larger VM registry adds one name.

Reproduce the workloads with:

```sh
pblang run bench/file_catalog_index.sapp --allow-all -- 20 fallback
pblang run bench/file_catalog_index.sapp --allow-all -- 20 basename-long
pblang run bench/file_catalog_summary.sapp --allow-all -- 10
```

For before/after, copy the baseline/current catalog to separate scratch modules
and change only each driver's import. Run the same corpus and repetitions,
including warmups, alternating order and output checks. The earlier exploratory
counter implementation regressed summary work by 7–14%; it was removed, not
included as a successful optimization.

## Ideas and remaining work

[Bounded intake and independent implementation provenance](IDEA_INTAKE_CATALOG_2026-09-17.md)
cover prior context, six mounted data volumes, application entries, cached
Codex/Claude manifests and public extension documentation. No discovered source
was executed or copied wholesale. No EnterpriseCore/Hetzner server was contacted.

Self-hosted metadata summaries are now covered; filesystem scans, complete
native runtime replacement, direct VM/frontend failure cleanup, composite/shared
memory, runtime/FFI isolation, remaining backend compatibility, report/release
authenticity, PBIDE Run/Designer and real guest operating systems remain open.
Version 1.0.0 is incomplete. The 211 unrelated source/workspace changes are
preserved; generated local lock metadata is refreshed separately.

The old OS-owned compressed diagnostic (about 44 MiB) remains outside the
repository and still needs administrator cleanup. It was not opened or uploaded.
This pass disabled core dumps in test/benchmark children and limited their logs;
no recursive frontend result was printed.

Graphify update exit: 0. Its AST analyzer does not support `.sapp`; this is not semantic PBLang coverage. Generated graphs and scratch probes are excluded from publication.
