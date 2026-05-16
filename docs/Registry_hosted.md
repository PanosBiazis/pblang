# Hosted registry — design note (M3-gamma)

Design-only document. No server implementation in this repo.

## Purpose

Define the HTTP API and auth model for a **hosted** pblang package registry (as opposed to the existing local file-based registry and `pblang publish --registry-dir`).

## API (target shape)

- **GET** `/<base>/index.json` — Registry index: list of package names and versions (and optional `sha256` per tarball). Same JSON shape as the local index used by `pblang install --registry-index`.
- **GET** `/<base>/tarball/<name>/<version>` — Download package tarball (e.g. `name-version.tar.gz`).
- **GET** `/<base>/tarball/<name>/<version>.sha256` — Optional sidecar: HMAC-SHA256 of the tarball (for verify-on-install when `PBLANG_SIGNING_KEY` is set).
- **POST** `/<base>/publish` — Publish a new package version. Body: tarball bytes or multipart; auth required. Idempotent for same name+version+content.
- **GET** `/<base>/search?q=<query>` — Search packages by name/description (optional; not required for MVP).

## Auth model

- **Publish**: Bearer token in `Authorization` header. Token supplied via env (e.g. `PBLANG_REGISTRY_TOKEN`) or future CLI flag (e.g. `--registry-token`). No auth for read (index + tarball) in the base design.
- **Optional**: Registry could support private packages later (read with token); out of scope for this note.

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
  - Auth: Bearer token via `Authorization: Bearer <token>` header, where the token is read from
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
