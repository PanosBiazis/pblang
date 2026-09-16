# File catalog portability and index validation — 2026-09-16

## Delivered behavior

The existing `stdlib/file_catalog.sapp` now extracts metadata basenames with
PBLang string operations. Both POSIX and Windows separators work on the same
host. Before this change the retained Linux `path_basename` could return a full
Windows-style path as a basename, contradicting the summary's privacy message
when explicit name disclosure was enabled. The pure helper also removes the
filesystem capability requirement from classification/summarization. Source
module loading still uses its separate import boundary; directory scans still
require filesystem access.

Deliberate compatibility changes: both slash styles are separators even if a
Linux filename could contain a literal backslash. A separator-free leading
ASCII letter plus colon is treated as a drive-relative prefix. Trailing
separators and final empty/dot/dot-dot components now fail with the existing
fixed name error. Multi-letter/numeric colon prefixes and colons in a final
component after a separator are retained. This is metadata normalization, not
filesystem canonicalization, path traversal prevention or authorization.

Classification builds a private index once, keeping rules in their original
order within final-extension buckets. Exact suffix checks preserve multipart
precedence; special names still win first. The longest final extension derives
the inspected tail length (currently eight characters including the dot), so
per-name suffix splitting does not scale with the number of dots in a name.
Results remain detached maps. A 64-character basename-tail fast path reduces separator splitting; it falls
back to the complete path when the basename is longer, without truncation.
No rule, label, general compiler/project limit,
foreign runtime or automatic executable launch was added.

## Tests and limitations

- 62 selected suites passed, including 17 emulators, the original catalog suite,
  all 151 rules across five name variants, privacy/portability tests and the
  existing dashboard test. This is not the complete repository suite.
- The new portable test passed with filesystem, network, HTTP, execution and FFI
  denied. The baseline positive control failed with filesystem denial as expected.
- The selfhost classifier test passed for Windows basenames, multipart suffixes,
  leading-dot known names and unknown names. Full `summarize` in that VM still
  fails: missing-key default behavior differs, and an experimental default/count
  amendment did not resolve the complete path and was reverted. No selfhost
  summary parity is claimed.
- The dashboard generates web, native and Android UI records and a local HTML
  response. This validates the shared presentation contract, not an interactive
  browser, native window, Android device or guest operating system.
- The dashboard test initially lacked its nested dependency lock. A scratch-only
  nested build generated it for the run; pblock subsequently made it portable.
  That local lock is excluded from publication. Root-launcher/nested-lock setup
  remains a bootstrap integration detail, not a new portability guarantee.
- 15 gates passed: selfhost source check, lint and strict entry-only pbsecure for
  the library, three new tests and benchmark. Independent security review found
  no code blocker after the rule-derived tail refinement.

Only metadata/source already authorized for this work was read. External
marketplace/MIME documentation supplied architectural ideas; implementation is
independent PBLang, with no copied external source and no execution of discovered
code. See [bounded intake](IDEA_INTAKE_CATALOG_2026-09-16.md).

## Reproduce the resource comparison

Baseline: `2d3e64d8f73651ce0242d2c371d200758fa9b72b`.
Retained executable SHA-256:
`7f1b97c0037736553ab39d165c13c36f3fd8497bc6aca9ca94a4c680858e6500`.

```sh
pblang run bench/file_catalog_index.sapp --allow-all -- 200 mixed
```

The same driver imports baseline/current catalog copies. Inputs are identical;
checksums of label/suffix lengths agree. Separate exhaustive fixtures verify
classification fields. Backslash-heavy basename fields intentionally differ
from the baseline on Linux; only unchanged labels/suffixes are checksummed. One warmup and seven process samples per variant/corpus,
alternating order; p95 is nearest rank (maximum of seven). Timed intervals cover
classification/checksum work, excluding imports, index construction and corpus
creation. GNU time whole-process CPU and peak RSS include that initialization.
No test/build/Graphify runs overlapped the reported measurements; unrelated
system activity was not controlled. Benchmarks run serially on the local Linux
host. Python/GNU time orchestrate PBLang only. No GPU/NPU/disk-write or executable
size improvement is claimed.

[Raw samples and module hashes](FILE_CATALOG_INDEX_BENCHMARK_2026-09-16.json).

| Corpus | Repetitions | Before median / p95 ms | After median / p95 ms | Median time change | Before / after median RSS KiB |
| --- | --- | --- | --- | --- | --- |
| mixed | 200 | 104.894 / 113.088 | 20.911 / 21.487 | -80.06% | 32328 / 32320 |
| early | 300 | 6.125 / 6.439 | 7.733 / 9.021 | +26.25% | 32164 / 32192 |
| miss | 200 | 74.155 / 96.286 | 10.230 / 12.837 | -86.20% | 32168 / 32224 |
| long | 50 | 6.385 / 6.837 | 2.761 / 3.166 | -56.77% | 32104 / 32328 |
| dots | 20 | 2.154 / 2.224 | 0.813 / 0.853 | -62.24% | 32152 / 32156 |
| separators | 20 | 0.386 / 0.556 | 0.897 / 1.349 | +132.37% | 32220 / 32352 |
| backslashes | 20 | 0.543 / 0.608 | 0.860 / 1.218 | +58.43% | 32208 / 32232 |
| fallback | 20 | 0.424 / 0.539 | 6.603 / 7.189 | +1455.84% | 32172 / 32360 |

`mixed` has ten names per repetition; `early` has three; `miss` four; the other
corpora two. `fallback` includes a long basename after 4,000 leading slashes;
its full-path fallback allocation cost is intentionally measured. Long names, dots and separators reach or approach the existing
4,096-character admission. Small timed intervals and sample variation limit
extrapolation. The table includes regressions; this is not a universal speedup
or zero-overhead claim. The index retains additional small bucket structures.

In this run, mixed classification fell from 104.89 to 20.91 ms per 2,000
classifications (about 80% less timed work). Early rule matches increased from
6.12 to 7.73 ms per 900 classifications (about 26% more). The adversarial long
basename fallback after thousands of separators increased from 0.42 to 6.60 ms
per 40 classifications (about 0.154 ms additional time per item on average).
That fallback remains an optimization target; it must not be hidden by the
mixed-workload gain. Median process peak RSS changed by less than 0.7% across
these runs; no general RAM saving is established.

## Remaining work and diagnostic handling

Full selfhost catalog summaries, direct VM/frontend failure cleanup, complex
memory, native runtime/FFI, backend parity, audits/releases, PBIDE Run/Designer
and real guest operating systems remain open. Version 1.0.0 is incomplete.
The 211 unrelated working-tree changes are preserved. No EnterpriseOrg server was
used. No plugin installation, foreign-code execution or remote device test ran.

A scratch experiment printed an entire frontend result containing recursive
closure/global graphs, causing runaway diagnostic output and process abort.
Its 862,857,932-byte stdout log was deleted. The OS retained a compressed
46,554,230-byte core outside the repository; unprivileged removal failed and
administrator rights are required. It was not inspected, uploaded or published.
Future diagnostics in this slice print only the asserted output array; the
recursive-print incident is not evidence that file classification crashed.

## Final local gates

Final retained-bootstrap build and PBLang-native portable-lock publication
passed without a dependency-hash warning. Graphify update completed with exit
0; its AST analyzer does not support `.sapp`, so this is not semantic coverage
of the changed source. Generated graphs, probes, nested scratch locks and
system diagnostics are excluded from publication.
