# Native CRUD example and product

The canonical runnable example is [examples/crud_website](../examples/crud_website/README.md).
The standalone [crud-website-pblang](https://github.com/PanosBiazis/crud-website-pblang)
product shares the exact same stdlib implementation.

`crud_records.sapp` offers validated detached records, configurable capacity,
numeric-ID ordered pagination/search, revision-checked writes and explicit close.
`crud_preview.sapp` adds generated responsive HTML/CSS, loopback request guards,
strict form decoding and the bounded Linux listener. No polyglot code executes
in the native path. Source being PBLang does not eliminate the retained host
runtime or existing libc FFI boundary.

This is a single-user local learning preview, volatile by default. It is not a production
identity, database or access-control service. Other local processes are trusted.
There are no analytics or remote assets. Persistence is explicit and optional. Private data
must not be placed in it. Public deployment, account parity with PHP, adversarial multi-user access and
non-Linux listeners remain pending.

Focused validation covers malformed/duplicate fields, Unicode, output escaping,
Host/Origin/CSRF, forbidden methods, record aliasing, capacity and stale writes.
A real browser submitted the native form. Same-origin referrer policy preserves
Origin for browser form POSTs without sending referrers to other origins.
No performance improvement is claimed; sorting a page currently scans and sorts
the bounded record set. Existing VM/FFI/GC compatibility limits remain applicable.

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
