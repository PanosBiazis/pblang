# Package archive verification

This contract describes the retained package-manager boundary, not a new Rust
product feature. PBLang remains the implementation language for new development.
The repair addresses finding P1 in the comparative readiness audit.

## Assurance and compatibility

`verify_tarball_detailed` returns `StructureOnly` or `HmacVerified`. The existing
`verify_tarball` unit-result API remains a compatibility wrapper. The CLI reports
the assurance returned by that same operation, without a second sidecar lookup.

- An absent sidecar permits structure-only verification. Absence is not evidence
  of authenticity: an attacker able to delete the sidecar can remove this
  optional integrity check. Callers requiring authentication must reject
  `StructureOnly`; the legacy unit-result wrapper cannot enforce that policy.
- The canonical sidecar is the complete archive filename plus `.sha256`, e.g.
  `package.tar.gz.sha256`. Pack, standalone verification and local-registry
  verification use one filename helper. Non-UTF-8 filename bytes are preserved.
- A present sidecar requires a nonblank `PBLANG_SIGNING_KEY`; the actual key is
  not trimmed. Sidecars contain 64 hexadecimal digits with at most one terminal
  LF or CRLF. Other whitespace, malformed content and oversized input fail.
- Sidecar reads admit only regular non-symlink files, open a no-follow descriptor
  on Unix/Windows, recheck the opened file, and read at most 129 bytes to enforce
  a 128-byte ceiling. Unsupported platforms reject present sidecars. Unix opens
  are nonblocking to prevent a substituted FIFO from blocking the open.
- HMAC comparison uses the existing `ring` dependency's verification function.
  This establishes shared-key integrity, **not publisher authentication**.
- Local registry URLs are decoded and admitted by the same file-URL helper as
  fetching. Percent-encoded filenames cannot silently skip their sidecar.

## Archive entry admission

Verification does not extract or execute package content. Verification and
extraction share an escape-resistant path guard: UTF-8, at most 4096 bytes,
relative paths only, no parent components, control characters, backslashes or
colons. Harmless `./` components and directory roots remain supported. Duplicate
normalized paths are rejected, including `x` versus `./x`.

Only regular files and directories are admitted. A verified package must have
an exact root **regular file** `pblang.toml` and a regular file with one of the
existing module extensions (`sapp`, `sap`, `bpx`, `pbx`, `pb`, `bp`). Nested or
suffixed manifest names and module-looking directories do not satisfy these
requirements. This is structural validation, not manifest parsing or compilation.

Existing compressed-size, entry-count, regular-file-size and aggregate-size
ceilings remain. Accounting uses effective entry sizes, including PAX overrides,
not just the original header. Directories must have zero payload bytes,
preventing their bodies from bypassing regular-file accounting. Extraction rejects an entry that
the tar library reports was not unpacked within the destination. On failure,
the existing cleanup removes only a destination newly created by that call.
Extraction into an existing directory is not transactional.

A shared reader limits decompressed bytes before tar parsing, including GNU/PAX
metadata consumed before logical entries are yielded. Its allowance is 256 MiB
plus 4096 times (4096 + 1024) bytes plus 1024 bytes. Exhaustion raises an error
instead of returning a misleading clean EOF. Reading exactly the allowance
succeeds, but any subsequent nonempty read fails even if the underlying stream
would have ended there; this boundary is intentionally conservative. Supported
GNU long names remain accepted, but unusual metadata-heavy archives can exceed
the allowance despite satisfying the logical-entry limits.

## Explicit remaining limits

These checks do not provide immutable archive/sidecar handoff, trusted parent
directory pinning, publisher trust, revocation, rollback protection, snapshot
consistency, or a hostile-code sandbox. The archive itself retains its existing
pathname-open behavior; this is not a general safe filesystem capability API.
Pack's existing sidecar-write failure handling and atomic signed publication
remain separate work. Remote registries do not acquire local-sidecar HMAC policy.

The path policy does not prove complete Windows naming portability: reserved
device names, case-fold collisions and trailing-dot/space aliases need additional
policy. Windows no-follow behavior and macOS have not been physically tested in
this slice. One GNU/PAX metadata allocation can still approach the global
decoded-byte budget (about 276 MiB): this is not a small per-extension allocation
limit or a process-memory ceiling. Gzip trailer/trailing-data policy and parser
CPU time are not qualified. There is no VM-wide hard heap or wall-clock guarantee.
The existing package writer also still rejects some long paths that the GNU tar
reader supports; this reader repair does not expand the writer's path support.

## Focused verification

```sh
cargo test --offline --locked -j 1 -p pblang-pm --lib archive_verify_tests -- --test-threads=1
cargo check --offline --locked -j 1 -p pblang --bin pblang
```

Tests use synthetic keys and exclusively owned temporary directories; they do
not mutate process environment variables. Coverage includes signed/unsigned
status, canonical names, missing/blank/wrong keys, malformed/oversized/non-file
sidecars, Unix symlinks, encoded URLs, malicious archive paths, exact manifest
identity, duplicate entries, directory payloads, all existing module extensions,
verification without extraction and new-destination cleanup after rejection.
The retained release executable must be rebuilt before its CLI gains this fix.
