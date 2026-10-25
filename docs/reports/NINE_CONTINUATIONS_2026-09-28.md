# Nine-task continuation — 2026-09-28

The nine existing product/library tasks were resumed and their latest delivery
reports reconciled with remote `main`. This supersedes the older statements that
no newer delivery was available in [the continuation ledger](CONTINUATION_STATUS_2026-09-27.md).
It is a bounded delivery record, not completion of the whole runtime audit or
qualification of PBLang 1.0. All production changes in these deliveries are PBLang.

## Delivered and verified

| Task area | Concrete result | Recorded verification | Delivered revisions |
| --- | --- | --- | --- |
| pbAI | Explicit training continuation from a detached student checkpoint, preserving the parent and frozen base; standalone/embedded parity | 33 checks, fresh-process file continuation, negative input/ownership tests | PBLang `765c96e`; pbAI `a4424c0` |
| PBQuiz / PBLesson | Failed commands return nonzero status with the existing JSON; GUI normal-close remains successful | 120 real CLI cases (45 + 75), two GUI suites, builds and four existing app/file suites | PBQuiz `f4499fc`; PBLesson `4609aa0` |
| PBDeckMix / PBStudy | Consistent success/operation-failure/usage statuses, including Web exports, preserving source and history | 103 real CLI cases (32 + 71), four existing suites, four real Linux GUI close tests and failed-write preservation | PBDeckMix `3fe4d16`; PBStudy `3bd32cd` |
| PBPortions | CLI and Web-export failure statuses; bounded rejection of nonfinite and oversized row arguments | 72 real CLI cases, 17 validation groups, allocation regressions and fresh remote-copy checks | PBPortions `db24ca2` |
| pbOS | Exact bounded ELF program/segment inspection with overflow, alignment, overlap, entry and W+X checks; no loading/execution | 34 suites, eight examples, 640 oracle cases, 12 detected code mutations, privacy probes | pbOS `29442d0` (PR 7) |
| Functional sorting | Replace private keyed maps with `[key, result]` pairs while preserving stable ordering and callback traces | Seven suites, 486 differential traces, two runs of 24 benchmark workloads | PBLang `a7968b6` |
| PBWorkdays | CLI/Web failures are visible to callers; private summaries and stale-export protection retained | 61 real CLI cases, five existing suites, Web failed-write preservation and real Linux GUI close | PBWorkdays `27358bb` |
| Discovery / PBIdeas | Pure bounded snapshot-manifest comparison and four synthetic recipes from the existing proposal catalogue | 40 kit commands, 22 PBIdeas checks, five native checks and post-integration checks | PBLang `48b0872`; kit `1118384`; PBIdeas `0c518a0`; catalogue `50daffd` |
| Graphics | Use contextual accessible labels on actual Web buttons/checkbox inputs; validate and preserve shared metadata | 11 suites, two post-rebase checks, real PB browser names/Tab/Enter/checkbox checks in desktop and narrow views | PBLang `bf1e127` (PR 30, feature `d23c9a8`) |

The six product CLI suites account for 356 subprocess cases. Their full suites
used each product's declared dependency snapshot; they are not a claim that every
product was retested against every newer library revision. Hosted and local
results are separate. The coordination review independently verified all listed
revisions on remote `main` and the private visibility of the 12 involved repos.

## Measurements and behavior limits

- Sorting: the 12 cheap-key workloads improved in both runs (about 7–14% less
  elapsed time initially, 5–20% in confirmation). One computed-key workload was
  0.6% slower. No byte-level RAM reduction or general speedup was established.
- pbAI: the fixed synthetic checkpoint continued from 1,024 to 1,280 steps;
  paired held-out loss changed from 0.769047 to 0.733383, with accuracy unchanged
  at 8/16 and zero previously-correct regressions in that fixture. This is not a
  representative language benchmark or a model activation decision.
- Snapshot comparison checks caller-supplied identifiers, sizes and digest claims.
  It does not read/hash actual files, authenticate hashes or merge projects.
  Limits are 256 entries per input and 512 output rows. The catalogue still has
  43 proposals; recipe/library counts are not proof of complete applications.
- Web accessible names were exercised in a browser. Native metadata retention
  and Android generation are not evidence of a native screen-reader bridge,
  TalkBack, iOS execution or physical-device qualification.
- ELF inspection describes a restricted executable profile. It neither boots
  nor executes arbitrary binaries.

See [sorting evidence](FUNCTIONAL_SORT.md),
[student continuation](pbAI_Student_Continuation.md),
[snapshot comparison](DISCOVERY_SNAPSHOT_COMPARE.md) and
[accessible controls](ACCESSIBLE_CONTROLS.md).

## Installation and integration scope

The local PBLang 0.6.0 launcher was updated to a versioned package containing
the committed standard library at `48b0872f153006a0580281865b8d3fc1c6c0f953`.
Twenty recorded candidate checks passed before switching the launcher, including
the four new shared modules, checked-source diagnostics, static PBCK bundles and
success/failure cleanup paths. Expected negative exits were asserted as such.
The prior package remains available for rollback. The retained binary itself
was not replaced with a newly implemented compiler.

Product source delivery and installation are distinct. PBPortions' launcher uses
its verified isolated checkout and pinned dependency; its original dirty product
folder was not replaced. The original development checkouts retain unrelated
changes. Local lock reconciliation is recorded separately from published locks;
local dependency hashes must not be copied into release commits.

The stale shared local lock was then reconciled with the actual development
sources using native lock generation and PBLang-native portable publication.
Eighteen integration checks passed: four lock refresh/publication pairs,
help/demo/preview and invalid-command behavior in both local PBQuiz/PBLesson,
and the two local pbOS ELF inspection/native-emitter suites. File snapshots
confirmed that only the PBLang and pbOS lock contents changed; the refreshed
PBQuiz/PBLesson locks were already identical. Other captured source files were
unchanged. This repairs that local startup obstruction without claiming a full
product regression run against the entire dirty development tree.

## Security/privacy, tools and remaining work

Reviews covered bounded inputs, preservation of existing files, generic errors,
explicit training and checkpoint ownership, output escaping, publication contents
and selected source-integrity gates. Fixtures are synthetic. No new telemetry,
automatic training ingestion, remote scan or company-server access was added.
The active Astra sessions performed the reviews; Daybreak was not invoked.
These focused checks do not constitute a complete repository or dependency audit.

PB-MCP isolated browser tools were used for the graphics interaction checks and
the synthetic snapshot report. Compiler, CLI, file and repository checks used
local PBLang/Git tooling where PB tools did not expose the required operation.
Graphify updates succeeded for several small products and standalone pbAI;
bounded PBLang/discovery updates encountered timeouts. A timeout is not a
successful graph refresh. Owned test/browser/display processes were cleaned up.

GitHub Actions jobs that were inspected failed before starting because of account
billing/spending limits. Matching local integrity jobs were run where applicable;
hosted CI did not pass. No billing settings, protections or reviews were bypassed.

Still open: the direct retained-source nested `p:run` defect, universal host/FFI
cleanup preserving the original failure, shared composite ownership/reference
cycles, GC pause bounds, prior async regressions, broad backend/platform parity,
application conversion, remaining polyglot/device work and full product coverage.
The explicit PBCK cleanup boundary does not handle initialization failures,
native crashes, forced exits or arbitrary hung cleanup callbacks. The separate
desktop PB-MCP bundle was not updated and no Live View acceleration was measured.
