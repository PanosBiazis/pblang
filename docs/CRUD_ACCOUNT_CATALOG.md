# Persistent local CRUD accounts

The PBLang CRUD preview can retain account names and bcrypt password hashes across
starts, independently of its record snapshot. Session tokens and CSRF tokens stay
in memory and do not survive restart. This remains a serialized local Linux
x86-64 example, not public production authentication or tenant isolation.

## First use

Choose a new local private directory, not a synchronized/network/shared directory.
From the PBLang checkout:

```sh
mkdir -m 700 "$HOME/.crud-saved-demo"
export PBLANG_CRUD_FILE="$HOME/.crud-saved-demo/records.pbcrud"
export PBLANG_CRUD_ACCOUNTS_FILE="$HOME/.crud-saved-demo/accounts.pbauth"
pblang run examples/crud_website/main.sapp --allow-all -- --init-persistent
pblang run examples/crud_website/main.sapp --allow-all -- --init-saved-accounts
pblang run examples/crud_website/main.sapp --allow-all -- --serve-saved-accounts
```

The second initialization prints a randomly generated initial `editor` password
once to its trusted terminal. Store it privately; do not collect that output in
shared logs. No password is passed through command-line arguments or environment
variables. Initialization refuses existing catalogs. On later starts, run only
`--serve-saved-accounts`, with both file-path variables set. The standalone CRUD
product accepts the same switches on `main.sapp`. Existing in-memory and ephemeral
account modes remain unchanged and are not silently upgraded.

Open the local `/login` page at `http://127.0.0.1:8765`. Stop with Ctrl+C. The
reviewed example uses broad `--allow-all` runtime permissions for its existing
Linux FFI boundaries; this is not a hostile-code sandbox.

## Trusted administration API

`crud_account_catalog.sapp` exports `initialize(path, account_hashes)`, `open(path)`,
`read(path)`, `encode(snapshot)` and `decode(text)`. A fresh `read` validates the private file and returns a detached snapshot without
creating a writable handle or computing an unused CAS digest. An open handle provides:

- `snapshot()` — detached `{revision, accounts}`. This contains sensitive hashes;
  never send it to a browser, ordinary logs or telemetry.
- `save(replacement_accounts, expected_revision)` — provision, rotate, rename or
  remove accounts by replacing the map. Returns the new revision. Empty maps,
  unsupported hashes, malformed names and exhausted/stale revisions are rejected.
- `refresh()` — reread after a competing writer or uncertain result. Inspect the
  snapshot before another mutation. No automatic retries.
- `close()` — invalidate the handle and drop retained references.

Hash generation still uses the existing `password_bcrypt_linux.sapp` provider:
libxcrypt through FFI, bcrypt `$2b$` cost 12. This change adds no polyglot blocks
and no custom password KDF, but does not replace the native crypto library.
The flat, non-executable catalog accepts 1–100 accounts, ASCII names of 1–64
characters and exactly the supported 60-character hashes. Its 16 KiB input bound
covers every admitted catalog. These are application budgets, not language limits.

`crud_preview.create_saved_accounts(records_path, catalog_path, ...)` composes both
stores and exposes `reload_accounts()`. Call this from trusted local administration
**after a catalog change**, or restart the server. Reload first revokes every
current session; a corrupt/missing/inaccessible catalog or failed password provider
leaves account access disabled (503) until a successful reload or restart. Reload
is not exposed as an unauthenticated HTTP action and does not resurrect sessions
on recovery. Existing sessions are not invalidated merely by editing the file:
there is no filesystem watcher or per-request catalog read in this implementation.

## Protection boundaries

Catalogs require current-user ownership, mode 0600 and a single regular-file link
inside the existing private-directory boundary. Symlink and hard-link catalogs are
rejected. Writes use the existing sibling lock, raw-byte compare-and-swap, atomic
publication, readback and synchronization checks. Keep the sibling `.lock` file;
all writers must cooperate and serialize calls on an individual handle.

An uncertain publication blocks the handle, even if the change became visible.
This prevents blindly repeating it. The catalog is not encrypted and cannot defend
against a malicious process with the same user privileges, backups being replaced,
a privileged attacker or dishonest storage. No secure erasure of all runtime string
copies is promised. File revision numbers are conflict detection, not anti-rollback
attestation. All accounts share editor access to one record workspace. There is no
MFA, public registration, password recovery, roles or Windows/macOS backend here.

## Immediate listener restart limitation

The current bounded Linux listener can fail to rebind the same port immediately
after closing accepted connections. Account persistence is independent of this
TCP state. This slice reproduced the failure; it does not fix it. A candidate
`SO_REUSEADDR` change was discarded because this retained runtime accepts at most
four typed FFI arguments and `setsockopt` requires five. No socket reuse setting,
OS networking parameter or existing listener behavior was changed. Wait for the
OS to release the address before a same-port restart; do not repeatedly provision
or overwrite the account catalog when a bind fails.
