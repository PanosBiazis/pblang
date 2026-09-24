# CRUD website example

A native PBLang record editor with create, read, update, delete, search and
pagination. It shares `stdlib:crud_records.sapp` and `stdlib:crud_preview.sapp`
with the standalone [CRUD product](https://github.com/PanosBiazis/crud-website-pblang).

From the PBLang checkout, refresh the local dependency lock with `pblang install`
after updating the stdlib. Then:

```
pblang run examples/crud_website/main.sapp
pblang run examples/crud_website/main.sapp --allow-all -- --serve
```

Open `http://127.0.0.1:8765` on this computer. Stop with Ctrl+C. The default command
runs a synthetic CRUD demonstration and exits; it opens no listener.

The UI and logic are PBLang source. HTML/CSS are generated strings, not executed
polyglot blocks. The retained host still provides runtime primitives; the bounded
listener currently supports Linux libc only and refuses unsupported platforms.
The non-network model can be tested separately on other runtimes.

The default is an **in-memory local preview**, not a multi-user production service.
Restarting erases records. There is no account authentication; other local
processes can access it. It checks Host, Origin, CSRF, duplicate fields, revisions,
input budgets and output escaping. It has no telemetry or remote assets. Use
synthetic data, not confidential records. 10,000 records is the preview capacity;
the core constructor accepts an explicit capacity. These are application budgets,
not PBLang language limits. Concurrent callers must serialize access.

The standalone repository also holds an optional PHP/JS/CSS/HTML edition in
`.sapp` polyglot source containers. It is never executed by this native example.

The current retained runtime requires an explicit FFI library/symbol policy.
The commands above use `--allow-all` only for this reviewed local example; this
is broad runtime permission, not a sandbox. Do not reuse it for untrusted code.

## Optional private persistence (Linux)

The default `--serve` remains in memory. To retain records, create a dedicated
**local, current-user-owned mode-0700 directory**, then choose an absolute file
path within it. Do not use a shared, synchronized, removable or network directory.
For example, from the product checkout:

```sh
mkdir -m 700 "$HOME/.crud-demo-state"
export PBLANG_CRUD_FILE="$HOME/.crud-demo-state/records.pbcrud"
pblang run main.sapp --allow-all -- --init-persistent
pblang run main.sapp --allow-all -- --serve-persistent
```

Initialize once. Initialization refuses to overwrite an existing file. Start with
`--serve-persistent` on later runs. For the PBLang example replace `main.sapp` with
`examples/crud_website/main.sapp`. The UI displays **Saved on this device** and
reloads disk state on a home-page refresh. Record files are plaintext, mode0600;
CSRF tokens, private paths and account credentials are not stored in snapshots.
No account authentication or encryption is added. Continue using synthetic data.

`crud_persistent.initialize/open(path, max_records=10000, max_bytes=1048576)` expose
explicit application budgets (up to 1,000,000 records / 64MiB per snapshot), not
language-wide limits. Unsupported platforms fail closed. `open` never creates or
repairs a missing/corrupt file. The flat versioned format rejects malformed rows,
duplicate IDs, invalid UTF-8 and impossible revisions without recursive parsing.

Every write validates a detached candidate, holds the existing secure-file sibling
lock, compares the raw-byte digest, publishes atomically and verifies readback,
synchronization and cleanup before reporting success. All cooperating writers
must use this API and preserve the `.lock` file. It is not protection against a
malicious process with the same OS user privileges or a replaced state directory.
Concurrent calls on the **same handle** must be serialized by the caller.

An unconfirmed save blocks that handle. No automatic retry occurs, because the
write may already be visible. Use `refresh()` and inspect the records before any
new mutation; reopening is also possible. The web response gives this warning.
Readers can hold an older snapshot until refresh, while stale writes are rejected.
A successful disk write is not proof against failing hardware or dishonest storage.

This snapshot implementation rewrites and hashes the entire dataset per mutation;
it is intended for small local examples, not high-throughput database workloads.
It allocates detached copies and adds disk I/O. Pure-PBLang SHA-256 compression
and internal serialization avoid redundant operations while retaining public
input validation and all digest/CAS/readback/synchronization checks. This is
still more expensive than memory-only storage. Native/PHP storage and account
parity and Windows/macOS persistence remain open. The separate product has
repeatable real-MySQL integration tests for its explicit PHP polyglot edition.

## Optional native accounts (Linux x86-64)

```
pblang run examples/crud_website/main.sapp --allow-all -- --serve-account
```

This separate mode starts with a random one-time `editor` password, shown once on
its launching terminal. Do not redirect that output into shared logs. Open the
local `/login` page. There is no fixed/default password, public registration or
credential persistence. All records and sessions are forgotten at exit.
`accounts.sapp` is an equivalent standalone example.

The account policy, forms, sessions and routing are PBLang, with no polyglot blocks.
Password hashing uses **system libxcrypt via FFI**, bcrypt `$2b$` with cost 12 and
128-bit random salts. It is not an entirely self-hosted cryptographic runtime.
Missing bcrypt/libxcrypt fails closed; Windows/macOS and C99 are not validated.
Hash creation accepts 15–72 UTF-8 bytes; verification rejects more than 72 bytes
and embedded NUL instead of allowing bcrypt truncation. Existing encoded hashes
must use exactly this algorithm/cost. No custom SHA-based password KDF is used.
See the [libxcrypt format contract](https://github.com/besser82/libxcrypt/blob/develop/doc/crypt.5).

The library API `CRUD.create_accounts(account_hashes, port=8765,
max_records=10000, lifetime_seconds=900, max_sessions=16)` accepts a trusted map of
account names to bcrypt hashes; it copies that catalog, not passwords. Up to 100
accounts, 1–256 sessions and 1–86400 seconds are explicit preview budgets. Every
configured account is an editor in one shared workspace, not an isolated tenant.
There is no persisted account manager, password reset, role administration or MFA.
The existing `--serve` and `--serve-persistent` modes remain unauthenticated.

Login requires a browser-bound anonymous session and CSRF token. Success rotates
both session identifier and CSRF. Cookies are HttpOnly, SameSite=Strict, host-only,
with expiry; logout is POST+CSRF and revokes the server-side session. The exact
loopback HTTP origin is required for writes. Cookies intentionally omit Secure
for this HTTP-only local example; never expose it via a proxy or external bind.
Cookies do not isolate local ports, and same-user processes are still trusted.
The memory-only catalog never persists tokens, passwords or account names.

The app allows ten valid-CSRF login attempts per minute globally, including
unknown accounts, with one bcrypt verification per attempt. There is no automatic
retry. A hostile local client can exhaust this budget/session slots and cause
availability loss. Sessions have an absolute lifetime, not sliding renewal;
a backwards wall-clock change revokes all sessions and requires restart.
Application close clears sessions/catalog references and closes its FFI handle.
The runtime does not guarantee secure erasure of string copies from RAM.
Calls across all bcrypt providers must be serialized because libcrypt uses static
native storage; this example's single request loop satisfies that contract.
No claim of thread-safe use or production authentication is made.

## Account-protected saved records (Linux)

After initializing a private snapshot as described above, use:

```sh
pblang run examples/crud_website/main.sapp --allow-all -- --serve-persistent-account
```

`PBLANG_CRUD_FILE` must identify that existing private snapshot. This combines the
same authenticated routes with durable record storage. Each start generates a new
one-time editor password on the trusted terminal; accounts and sessions are still
ephemeral, records are retained. Never collect that terminal output in shared logs.
The ordinary `--serve-persistent` remains unauthenticated.

`CRUD.create_persistent_accounts(path, account_hashes, port=8765,
max_records=10000, max_bytes=1048576, lifetime_seconds=900, max_sessions=16)` accepts
trusted caller-provisioned bcrypt hashes. `serve_persistent_accounts(path, hashes,
port=8765, max_connections=nil)` starts the bounded local listener. This does not
implement a permanent account manager/password reset or tenant isolation.
Password hashing still depends on Linux libxcrypt through FFI.

Session expiry now avoids scanning the session map until its earliest deadline.
Clock rollback and absolute expiration remain checked. Login pages use same-origin
referrer policy so real browser form submissions preserve their Origin header;
cross-origin referrers are suppressed and `Origin: null` is not trusted.

## Saved account catalog

For account credentials that survive restart, use `--init-saved-accounts` once and
`--serve-saved-accounts` on later runs. Both the native standalone product and this
example use the same implementation. See [setup, trusted administration, session
revocation and limitations](../../docs/CRUD_ACCOUNT_CATALOG.md). The older account
modes above intentionally keep their ephemeral behavior.
