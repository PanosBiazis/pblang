# Lockfile contract (pblang.lock)

## What the lockfile guarantees

- **Pinned dependency resolution**: `pblang.lock` records the exact dependency graph used for a project. Each dependency is identified by a **path** (resolved from `pblang.toml` at lock time) and a **sha256** of that dependency’s unpacked contents (directory hash).
- **Lockfile v2 (optional fields)**: Each locked dependency may include optional **name**, **version**, **source** (e.g. `registry`, `git`, `path`), **resolved**, **fetch_url**, **archive_sha256**. Old lockfiles (path + sha256 only) still load; new fields are used when registry/git support is implemented. See [Registry_MVP.md](Registry_MVP.md).
- **Manifest binding**: The lockfile stores `manifest_sha256` (hash of `pblang.toml`). If you change `pblang.toml`, the lock is considered stale and resolution may recompute (e.g. on `pblang install` or `--lock`).
- **Reproducible resolution**: For the same `pblang.toml` (same manifest hash) and same filesystem layout, `generate_lockfile` produces the same lockfile. So rerunning `pblang --lock` in the same tree yields the same `pblang.lock`.

## Registry deps security

- **Mandatory checksum verification**: Every registry release must provide a
  well-formed 64-hex `sha256`. Missing or malformed digests and mismatches are
  hard errors before unpacking. `--require-registry-sha256` remains accepted
  only for CLI compatibility; it can no longer weaken or strengthen this rule.
- **Bounded transactional extraction**: Registry reads are capped while
  streaming. Archives are limited to 64 MiB compressed, 4,096 entries, 64 MiB
  per regular file, and 256 MiB total declared content. Only regular files and
  directories are admitted; links and special entries fail closed. A failed
  extraction removes the partial cache destination.
- **Import-time lock validation**: When a dependency is resolved through `pblang.lock`, pblang re-hashes the unpacked dependency directory and compares it to the locked `sha256`. If the contents were modified after install, import resolution fails with a content hash mismatch instead of silently loading tampered code.
- **Lockfile-based rehydration**: For registry and git dependencies, if the locked cache/checkout path is missing but the lockfile still has enough source metadata (`source`, `resolved`, `fetch_url`, `archive_sha256`), pblang can recreate the dependency root from the lock entry and then re-verify it against the locked hash instead of failing immediately on the missing absolute path.
- **Legacy registry lockfiles**: Older registry lockfiles stored the downloaded archive checksum in `sha256`. New lockfiles store the unpacked content hash in `sha256` and the archive checksum separately in `archive_sha256`. Legacy registry entries still load only if pblang can positively revalidate them against the original archive; otherwise rerun `pblang install` to refresh the lockfile.

## Git deps

- **Pinned rev contract**: Git dependencies use `{ git = "...", rev = "..." }` and resolve to a cached checkout keyed by URL + rev. The minimal supported contract requires `rev`, so installs stay deterministic.
- **Lockfile metadata**: The lockfile records `source = "git"` and `resolved = "<git-url>#<rev>"`, while the dependency `sha256` still comes from hashing the checked-out working tree contents.
- **Install-time validation**: Third-party git packages are validated at install time with the same package-entry/module presence contract used for registry packages, so broken packages fail before a usable-looking lock entry is written.

## What it does *not* guarantee (yet)

- **Cross-machine reproducibility**: Paths in the lockfile are still absolute or environment-dependent. Rehydration makes registry/git entries more portable when their cached paths disappear, but the lockfile is currently a runtime-aware portability format rather than a fully path-independent one.
- **Publisher identity**: A digest proves the downloaded bytes match the index;
  it does not authenticate who published the index. Publisher signatures and
  trusted-key rotation remain a separate release requirement.

## Commands

- **`pblang --lock [--registry-index <url>] [--require-registry-sha256] [path]`**: (Re)generate `pblang.lock` from `pblang.toml` in the given directory (default current). Overwrites existing lockfile.
- **Import resolution**: When the VM resolves `import(...)`, it can use the lockfile (when present) to resolve dependency paths and validate hashes so that the same lock implies the same dependency contents.

## CI / reproducibility

To assert lockfile reproducibility in CI:

1. Check out the repo (including `pblang.lock`).
2. Run `pblang --lock` (or the equivalent that regenerates the lockfile).
3. Diff the new lockfile against the committed one (or compare a hash). If they differ, the lockfile was not reproducible (e.g. manifest or dependency contents changed, or non-determinism in generation).

This gives a “lockfile reproducibility” test as suggested in the v0.3 roadmap.

In this repository, an integration test target exists for this contract:

```bash
cargo test --test lockfile_repro
```
