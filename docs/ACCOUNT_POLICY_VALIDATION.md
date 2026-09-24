# Saved accounts and configurable rejection budgets: validation

This slice extends the PBLang libraries, native CRUD example and standalone CRUD
entry. It does not complete Pascal conversion, spatial/multidimensional interfaces,
public authentication, cross-platform crypto or the general runtime/FFI/GC backlog.

## Retained behavior

- Private, versioned account catalogs persist only names and supported bcrypt
  hashes. Initial provisioning refuses an existing catalog. No plaintext password,
  CSRF token or session identifier is stored there.
- Trusted replacement supports provision/rotation/removal with optimistic revision
  checks and the existing private compare-and-swap publisher. Unconfirmed writes
  block the handle until refresh. Reads reject symlinks, hard links and non-0600
  files. This assumes local private state and cooperating writers, not a hostile
  same-user process or a cryptographically authenticated/anti-rollback store.
- Explicit reload invalidates old sessions before loading and stays closed on
  failure. External catalog changes are not automatically watched. All accounts
  retain the existing shared editor role; there is no MFA/password recovery.
- Rejected-request throttling remains ON in the form preview, with independent
  `rejected_request_limit`, `rejection_window_ms`, `rejection_cooldown_ms` settings.
  Defaults stay 8 / 60000 / 30000. Opt-out never disables Host/Origin/CSRF checks.
  The rejection window is independent of the crawler window. This is not pentest
  identification or a guarantee of blocking attacks. It is a shared local budget,
  not per-user enforcement and not a retrofit of all existing web applications.

## Checks

Fourteen targeted semantic scripts passed, covering catalog codecs, real private
storage, competing writes, stale handles, detached snapshots, malformed reload,
revocation, existing CRUD/authentication/persistence, form checks and rejection
budgets, plus the existing secure-file transaction tests. The standalone native
product test also passed. Focused tests were repeated after the read-path changes.

Real HTTP validation provisioned a random initial password, signed in, stopped the
listener, restarted with the same catalog/password and rejected the previous
session cookie. Test credentials stayed in memory; no real accounts were used.
The listener had to wait about one minute before reusing its port (see below).

Lint/build, five entry-only static security checks and native portable-lock
publication were checked in isolated fixtures. Static checks are not a full
security audit. Pre-existing working-tree files were excluded from the fixtures
and preserved. The retained loader required installed absolute dependency roots
for sibling-package execution after relative-lock publication; this was recorded
as a tooling limitation, not silently treated as portable execution evidence.
No Windows/macOS, GPU/NPU, desktop Live View or full self-hosted runtime claims are
made. No Specisoft server was used.

## Measurements

Same retained executable; local isolated fixtures; one warmup and seven samples
per version, alternating order. `/usr/bin/time` measures complete process CPU,
wall time and peak RSS. Nearest-rank p95 is the maximum of seven samples and does
not establish a production tail-latency bound. Raw samples are retained locally.

The rejection-policy comparison uses parent PBLang commit
`b5fc836efdc854d1f143ba96fe5c3f7bab4e2358`: create an empty canonical form with
crawler discouragement OFF and rejection throttling ON, then make 500 successful
GET requests through the handler, asserting status 200 each time.

| Case | Median CPU s | Sample p95 CPU s | Median peak RSS KiB |
| --- | ---: | ---: | ---: |
| Parent rejection policy | 0.36 | 0.42 | 31688 |
| Configurable rejection policy | 0.37 | 0.42 | 31712 |
| Inline account catalog | 0.71 | 0.76 | 31380 |
| Saved account catalog, final read path | 0.74 | 0.76 | 31756 |

The account workload includes startup, one real bcrypt login and 1000 authenticated
in-memory gate calls. Both cases use the same synthetic account/hash and identical
requests. Saved catalogs add one private validated read at startup, not a disk read
on every request. A dedicated `read` path avoids unused CAS hashing/writable-handle
construction, and write-only modules load only for administration. The final saved
case still costs about 4.2% more median CPU and 1.2% more RSS here. The policy case
costs about 2.8% more CPU (one 0.01-second measurement tick). Neither is a general
speedup or memory-saving claim. Persistent writes and reloads have additional costs
not represented by this authentication workload.

## Discarded experiment and open blocker

The existing bounded Linux TCP listener failed immediate rebinding after a served
connection. A minimal test reproduced `parent_immediate_rebind:false` against the
parent version. The standard SO_REUSEADDR approach was examined using installed
Linux socket documentation and headers. A candidate was discarded: `setsockopt`
needs five arguments, while this retained host's typed FFI accepts at most four.
No socket reuse/OS networking setting or native TCP source change was retained.

Final validation must not be read as an immediate-restart fix. Same-port HTTP
restart was tested only after waiting for the OS to release the port. Account
persistence itself succeeded, and old sessions stayed invalid. Typed-FFI expansion
needs its own calling-convention, cleanup, denial and platform tests before this
can be fixed through the native boundary. The installed PB-MCP was not updated.

Graphify refresh exceeded the 15-second bound and its process group was stopped
and reaped. The graph refresh did not complete.
