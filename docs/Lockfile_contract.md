# Lockfile contract (pblang.lock)

## What the lockfile guarantees

- **Pinned dependency resolution**: `pblang.lock` records the exact dependency graph used for a project. Each dependency is identified by a **path** (resolved from `pblang.toml` at lock time) and a **sha256** of that dependency’s contents (directory hash).
- **Lockfile v2 (optional fields)**: Each locked dependency may include optional **name**, **version**, **source** (e.g. `registry`, `git`, `path`), **resolved**, **fetch_url**. Old lockfiles (path + sha256 only) still load; new fields are used when registry/git support is implemented. See [Registry_MVP.md](Registry_MVP.md).
- **Manifest binding**: The lockfile stores `manifest_sha256` (hash of `pblang.toml`). If you change `pblang.toml`, the lock is considered stale and resolution may recompute (e.g. on `sapphire install` or `--lock`).
- **Reproducible resolution**: For the same `pblang.toml` (same manifest hash) and same filesystem layout, `generate_lockfile` produces the same lockfile. So rerunning `sapphire --lock` in the same tree yields the same `pblang.lock`.

## Registry deps security

- **Checksum verification**: For registry dependencies, when the index entry provides a `sha256` value, the resolved tarball is verified against it before unpacking. A mismatch yields a hard error (e.g. "registry: checksum mismatch for …").
- **Optional strict mode**: Setting **`REQUIRE_REGISTRY_SHA256=1`** (or `true`) forces the registry index to include a `sha256` for each release; if it is missing, resolution fails with "sha256 required …". Use this in CI or trusted supply-chain setups to avoid installing unchecksummed registry packages.

## What it does *not* guarantee (yet)

- **Cross-machine reproducibility**: Paths in the lockfile are currently absolute or environment-dependent. Same project on another machine may have different paths; content hashes still identify the same dependency content.
- **Registry versions**: Registry support adds version pins and checksums in the lockfile; semver solving and publish flow are separate.

## Commands

- **`sapphire --lock [path]`**: (Re)generate `pblang.lock` from `pblang.toml` in the given directory (default current). Overwrites existing lockfile.
- **Import resolution**: When the VM resolves `import(...)`, it can use the lockfile (when present) to resolve dependency paths and validate hashes so that the same lock implies the same dependency contents.

## CI / reproducibility

To assert lockfile reproducibility in CI:

1. Check out the repo (including `pblang.lock`).
2. Run `sapphire --lock` (or the equivalent that regenerates the lockfile).
3. Diff the new lockfile against the committed one (or compare a hash). If they differ, the lockfile was not reproducible (e.g. manifest or dependency contents changed, or non-determinism in generation).

This gives a “lockfile reproducibility” test as suggested in the v0.3 roadmap.
