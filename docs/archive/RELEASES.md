# Releases

PBLang is experimental. Named development snapshots identify reproducible source revisions; they do not certify every backend, operating system or application built on the language.

## Development snapshot · 2026-10-11

### Bounded document libraries · corrected snapshot

`snapshot-2026-10-11-brief-r2` adds the [brief and excerpt APIs](docs/Document_Brief.md) and a dependency lock reproduced from tracked source. Three semantic suites, lint without warnings, manifest validation, strict source build and portable-lock publication passed against the clean source tree using the retained `379e783b1547` runtime. Installed runtimes were not replaced. Graphify update reached its time limit; this is not a successful graph refresh.

The earlier `snapshot-2026-10-11-brief` is superseded: its lock included ignored local state in the dependency digest. Its immutable tag and draft release are retained for traceability. The corrected stdlib tree-v2 digest is `e29159f375346501c70b3c1a53f2dca5e291bb7525be4b2b7ebe16cf6ffd10fd`.

Build and validate release sources in a clean checkout or extracted source archive. Ignored files under a dependency directory can affect the package-tree digest even when Git reports a clean working tree. Preserve such local state separately; do not delete it or silently regenerate a release lock from it. The corrected lock deliberately matches the published source, not a working dependency tree containing extra local files.

### Initial snapshot

This source snapshot includes [PR95](https://github.com/PanosBiazis/pblang/pull/95), merged at `379e783b1547dacd8b3a6181dc1ff8b9dba3bbad`, plus documentation and editor workspace organization. The tag `snapshot-2026-10-11` identifies the exact published revision. The package version remains `0.6.0`; a version string alone does not establish compatibility.

### Included runtime corrections

- Pending deferred closures, frame defers and module results remain reachable during garbage collection.
- Instruction exhaustion enters terminal cleanup with one shared allowance of 10,000 additional bytecode instructions. Nested cleanup cannot renew that allowance.
- The original error survives secondary cleanup failures; exhausted cleanup remains bounded.

Read the [native cleanup contract](docs/Native_cleanup_limits.md) before relying on these guarantees. They concern the retained native VM; they do not add `defer` to the self-hosted PBCK frontend, interrupt blocking native/FFI calls, or guarantee successful release of a resource whose close operation fails.

### Validation provenance

The runtime correction was previously checked on Linux with 38 selected Rust tests, including 11 new cleanup regressions. Those results apply to the PR95 source base. The later documentation/editor changes are checked separately for JSON validity, link targets, whitespace and intended scope; they do not constitute another runtime test run. Hosted checks blocked by billing are not passes.

No universal speed, memory-saving, platform-support or production-readiness claim is made. The [README](README.md) documents startup and testing; individual documentation pages define their own supported subsets and limitations.

### Getting the source

Use an authenticated checkout of the release tag in a separate directory, then follow the README build instructions. GitHub's generated source archives contain tracked source only. They do not include local toolchains, caches, sibling projects or a tested installation. This prerelease adds no downloadable binary packages and does not alter installed runtimes.

Record the source commit, actual runtime binary SHA-256, dependency lock and backend when reporting a result. Product-bundled runtimes need their own qualification before replacement.

### Workspace organization

The checked-in Cursor/VS Code settings hide generated build, dependency, graph and scratch-directory output from the Explorer and reduce watching of build caches. They do not delete files, suppress Git changes or hide source/tests. Git's large-change warning remains enabled. To inspect generated output, adjust `files.exclude` in the workspace settings.

Keep unfinished work on a separate branch or recovery worktree before synchronizing `main`. A clean `main` does not mean that every preserved experimental change has been integrated. Never discard a recovery snapshot until its contents have been reviewed and incorporated or deliberately retired.
