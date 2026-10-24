# Path graph and platform validation — 2026-09-15

## Implemented slice

Added a path-preserving supplied-import graph API and an additive identity resolver wrapper. Existing compiler/linker calls and legacy graphs are unchanged. No new compiler source-size, duration, or project-count limit was added. New graph-format restrictions are documented in [CODE_GRAPH_PATHS.md](CODE_GRAPH_PATHS.md).

## Validation

- 29 suites passed: 7 source-inliner/linker suites, knowledge graph, the new path graph, 2 self-hosted module suites, C99 module globals, and 17 emulator/device/input suites.
- The path-graph regression covers 2,400 supplied modules and 1,200 resolved import edges, repeated basenames, canonical aliases, invalid inputs, exact spelling and missing modules.
- Three representative linker results were compared against the unchanged base implementation: complete result objects, including source, order and graph digest, were identical.
- Retained-bootstrap build and portable lock refresh passed. Self-hosted checks and lint passed on all three touched PBLang files; self-hosted lint returned zero diagnostics. The older bootstrap linter returned four existing warnings in source_inliner plus two false unused-variable warnings in the new graph module (the variables are read).
- Three strict entry-only security scans had zero blocking findings, completed mandatory checks and stable source snapshots. These are not dependency-wide or complete semantic security proofs. Independent security review found no blocker for the documented pure supplied-data scope.
- The first C99 sanitizer run could not reserve AddressSanitizer shadow address space under the test harness 2 GiB virtual-address ceiling. The same test passed with that harness ceiling removed; no product limit was changed.

## Real Windows workstation

Only the user-approved corporate workstation was accessed through its existing direct Tailscale/SSH route, with host-key checking and hostname/workstation-OS verification. No EnterpriseCore server was accessed for this work. No remote software was installed and no test program files were created on that PC. Generated WASM bytes were supplied in memory to its existing Node runtime.

The same artifacts ran on Linux x64 Node 18.19.1 and Windows 11 Pro x64 Node 26.8.2. Each engine also rejected a half-truncated copy of each module.

| Program | Expected result | Artifact SHA-256 |
| --- | --- | --- |
| sum | 49995000 | `5b078b08949114c6191561b5bc2388c5ec1a12127a8bd62c0ffb5d75fc1b614a` |
| recursion | 144 | `a52d6e5d38b09e7a99637a38c69067903700248ce0ad63fc18f3b4ea8d54275a` |
| strings | 300 | `fc33bdc779b2d5b1727297fea3196b5903da6b35f2ebb083ab541e8fda5d8b35` |
| multifile | 42 | `07e6024d6abc3752968e27ef90fb4f0aa142494c83857c34045ce49f9873c0f6` |

The cases are a sum from 0 through 9,999, Fibonacci(12), 100 concatenations of `abc`, and two same-named functions in distinct `left/util.sapp` and `right/util.sapp` modules returning 19 and 23. The last case was linked with `source_inliner.link_modules`; all four used the current PBLang WASM backend. This verifies these generated programs on a real Windows OS, not a native Windows PBLang installation, all libraries, GPU/NPU execution, or universal platform compatibility.

## Conversation and disk intake

Reviewed the visible task catalog (64 summaries), selected relevant detailed turns, and all indexed local user-message projections. For the 50 root tasks missing projections, available rollout user messages were examined as a fallback. Coverage was 128 cataloged local root/user tasks, 1,773 projected user-message records plus 127 fallback records. This is a bounded relevance/intake pass, not a full reading of every assistant/tool message or every cloud conversation.

Enumerated metadata for 11,231,780 regular-file entries across the mounted physical filesystems. Counts include repeated backups, dependencies and caches; they are not unique projects or content-read counts. No general file-content ingestion or pbAI training took place. Remote/network mounts, symlinks, pseudo-filesystems and an unmounted partition were excluded.

| Volume role | File entries | Coverage limitation |
| --- | ---: | --- |
| internal | 9,440,386 | 516 access/I/O errors |
| external_backup | 1,113,516 | metadata enumeration finished; contents not read |
| external_ssd | 87,385 | metadata enumeration finished; contents not read |
| external_projects | 507,148 | 298 pending directories at 600-second scan budget |
| external_boot | 81,984 | metadata enumeration finished; contents not read |
| external_test | 1,361 | 53 access/I/O errors |
| internal_efi | 0 | 1 access/I/O errors |

`internal_efi` could not be read. The test volume reported 53 I/O errors; no repair or write was attempted. The project volume scan remained incomplete. Therefore this report does not claim that every disk file was read or even enumerated.

## Ideas grounded in the intake

Selected current README sections in sibling PBOffice, PBRemote and pbAI were read alongside relevant earlier requests. These are development leads, not verified claims that those products are complete.

1. **Project identity and discovery:** distinct module paths and explicit coverage should precede cross-disk deduplication, incremental source-hash caches, or IDE dependency navigation. This slice implements the in-memory identity foundation. A confined resumable directory walker remains necessary.
2. **Document libraries and PBIDE:** PBOffice’s shared document model, reversible drafts and explicit save behavior suggest reusable document/history APIs. Many Pascal/form file entries suggest evaluating a declarative form importer; no proprietary source was copied or converted.
3. **PBRemote/platform capabilities:** reuse existing capability contracts and platform adapters rather than creating parallel implementations. Require actual backend evidence before advertising device support.
4. **pbAI:** reuse its source-catalog/proposal-admission separation for proposed improvements; conversation and file discovery must not silently become training or executable proposals.
5. **PBMedia:** earlier user requests group image, video and audio in one product. Shared streaming I/O and media metadata libraries are a useful candidate; this slice neither creates a new product nor claims codec support.

## Graph refresh and remaining work

Graphify AST-only update completed in 51.83 seconds with clustering disabled. The installed extractor still excludes `.sapp`; this is not full PBLang graph coverage. A separate native path-graph snapshot covered the 20 compiler-directory modules, with 94 lexical import records and 22 unresolved records outside the supplied set. Its `complete` flag is false. Existing aggregate graphs were preserved.

Still open: complete/resumable disk and cloud-history intake; wider encoded graph identifiers and confined CLI/MCP integration; larger multi-file compilation and incremental caching; runtime/dynamic-memory/FFI enforcement; libraries; authenticated audits/releases; PBIDE Run/Designer; broader current-artifact platform and guest-OS validation. No new compilation speedup or RAM reduction is claimed by this slice.
