# Hosted registry — design note (M3-gamma)

The hosted server remains a separate project, while the client-side trust and
install contract described below is implemented in this repository.

## Purpose

Define the HTTP API and auth model for a **hosted** pblang package registry (as opposed to the existing local file-based registry and `pblang publish --registry-dir`).

## API (target shape)

- **GET** `/<base>/index.json` — Registry index: list of package names and versions with a mandatory 64-hex `sha256` per tarball. Same JSON shape as the local index used by `pblang install --registry-index`.
- **GET** `/<base>/index.json.sig.json` — Mandatory detached publisher identity
  signature for production installs. JSON fields are `schema`
  (`pblang-registry-signature-v1`), `publisher`, `key_id` (SHA-256 of the
  Ed25519 public key), `algorithm` (`ed25519`), `index_sha256`, and base64
  `signature`. The signature covers the exact `index.json` bytes.
- **GET** `/<base>/tarball/<name>/<version>` — Download package tarball (e.g. `name-version.tar.gz`).
- **GET** `/<base>/tarball/<name>/<version>.sha256` — Legacy optional HMAC-SHA256 sidecar. It is not publisher identity and does not replace the signed index.
- **POST** `/<base>/publish` — Publish a new package version. Body: tarball bytes or multipart; auth required. Idempotent for same name+version+content.
- **GET** `/<base>/search?q=<query>` — Search packages by name/description (optional; not required for MVP).

## Auth model

- **Publish**: Bearer token in `Authorization` header. Token supplied via env (e.g. `PBLANG_REGISTRY_TOKEN`) or future CLI flag (e.g. `--registry-token`). No auth for read (index + tarball) in the base design.
- **Optional**: Registry could support private packages later (read with token); out of scope for this note.

## Publisher trust and unsigned development mode

Production resolution is fail-closed. Set `PBLANG_REGISTRY_TRUSTED_KEYS_FILE`
to an operator-owned JSON file mapping publisher identities to 32-byte Ed25519
public keys encoded as 64 hexadecimal characters:

```json
{
  "pblang-foundation": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
}
```

On Unix the trust store must be a regular non-symlink file, owned by the current
user or root, and not group/world writable. The client verifies the index digest,
the trusted publisher/key-id binding, and the Ed25519 signature before parsing
package metadata or downloading an archive. The existing archive SHA-256 then
binds the authenticated metadata to the downloaded bytes.

Legacy/local unsigned registries work only when the operator explicitly sets
`PBLANG_ALLOW_UNSIGNED_REGISTRY_DEV=1`. The switch is compiled out of release
builds and accepts only `file://`, `localhost`, `127.0.0.1`, or `[::1]` index
URLs. Remote lookalikes fail before any network request. This mode is for local
fixtures and migration only; it must not be used for releases, CI publication,
or multi-user installations.

Loopback classification uses a standards-compliant URL parser and rejects URL
credentials and backslashes. Registry HTTP requests do not follow redirects and
accept only `2xx` responses, so an approved local URL cannot transfer its trust
to a remote redirect target.
Unsigned `file://` fixtures must resolve to absolute local paths; Windows
UNC/device/network paths and file URLs with hosts, credentials, query strings,
or fragments are rejected.

Verified archives are extracted into a unique private staging directory. The
cache root must belong to the current user and is tightened to mode `0700` on
Unix. An existing archive-digest directory is reused only when its validated
content hash matches a fresh extraction; mismatches fail without overwrite.
Tree digests cover hidden files, `target/`, and directory identities. Symlinks,
special files, manifest-entry traversal, and entries outside the package root
are rejected.
The versioned digest encoding domain-separates entry types and length-prefixes
every UTF-8 path and content payload, avoiding structural ambiguity from NUL or
other arbitrary file bytes.

Generated lockfiles explicitly record
`content_hash_algorithm = "pblang-package-tree-v2"`. Missing, legacy, or unknown
algorithms require an operator-reviewed `pblang install` refresh; they are never
silently reinterpreted. Git dependencies likewise require a full commit ID and
are cloned into private staging before atomic cache comparison/publication.

## Current non-release security boundaries

This hardening is a private alpha checkpoint, not production release evidence.
Before registry/Git manifests or lockfiles can be treated as hostile input, the
resolver must additionally derive dependency roots exclusively from validated
source provenance instead of serialized lock paths; propagate every declared
bare-import lock failure instead of falling back to local lookup; reject and
redact credential-bearing Git origins; and run Git network/process/filesystem
work inside hard time, process-tree, transfer, disk, file-count, depth, and
memory ceilings. Windows cache ownership/DACL evidence and signed-index
rollback/expiry/namespace delegation are also still required for production.

## Future client changes (deferred to M3-delta)

- **Publish from client**: `pblang publish --registry-url <url> [--registry-token <token>]` to POST to a hosted registry instead of writing to a local directory.
- **Search from client**: `pblang search <query> [--registry-index <url>]` to call GET search (if the server implements it).
- **Manifest**: Optional `[registry]` section in `pblang.toml` to set default registry URL/token per project; not required for first hosted rollout.

## M4-beta: Remote publish + search (client)

### `pblang publish`

- Reads **`PBLANG_REGISTRY_INDEX`** (env) as the registry base URL.
- For **file-based registries**, `PBLANG_REGISTRY_INDEX=file:///path/to/registry_dir`:
  - `pblang publish` behaves as a hosted-style wrapper around the existing local registry:
    it packages the current manifest dir, validates that the package is self-contained and
    has at least one pblang module entry, and delegates to the same logic as
    `pblang publish --registry-dir /path/to/registry_dir`.
  - This updates `<registry_dir>/index.json` and writes a tarball named
    `<name>-<version>.tar.gz` under the registry directory.
- For **hosted registries** (http/https), `PBLANG_REGISTRY_INDEX=https://registry.example.com`:
  - `pblang publish` builds a tarball from the current package root (same file set as
    `create_package_tarball`), computes its sha256, and POSTs a multipart/form-data body
    to `PBLANG_REGISTRY_INDEX + "/publish"`.
  - Multipart fields:
    - `tarball` — raw tar.gz bytes, filename `<name>-<version>.tar.gz`, `Content-Type: application/gzip`.
    - `sha256` — hex-encoded sha256 of the tarball.
    - `metadata` — JSON object `{ "name": "<name>", "version": "<version>", "description": "<desc>" }`.
  - Auth: bearer token via the request auth header, where the token is read from
    `PBLANG_REGISTRY_TOKEN` or optional CLI flag `--registry-token=<token>`.
  - Server is expected to enforce immutability per `<name> + <version>` and may reject
    mismatched content.

### `pblang search <query>`

- Reads **`PBLANG_REGISTRY_INDEX`** (env) as the registry base URL.
- For **file-based registries** (`file://`):
  - Reads `<registry_dir>/index.json` directly and returns a list of matches by simple
    substring search over package names and versions.
  - Output format:
    - `<name> @ <version>` when description is not available in the index.
- For **hosted registries** (http/https):
  - Issues **GET** `PBLANG_REGISTRY_INDEX + "/search?q=<query>"`.
  - Expects a JSON array of `{ "name": string, "version": string, "description": string }`.
  - Prints one line per result:
    - `<name> @ <version> — <description>`
  - If the array is empty, prints `No results for '<query>'`.

## Non-goals (this document)

- Implementing the server (separate service/repo).
- Defining a formal OpenAPI spec (this is a design note only).

## Verification in this repo

- Registry client flows should be validated with integration tests in `tests/install_cli.rs`.
- Lockfile determinism and reproducibility are validated separately via:

```bash
cargo test --test lockfile_repro
```
