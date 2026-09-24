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
It allocates detached copies and adds disk I/O. No speed or RAM improvement is
claimed. Native/PHP storage and account parity, real MySQL integration and
Windows/macOS persistence remain open.
