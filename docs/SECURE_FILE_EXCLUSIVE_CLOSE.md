# Exclusive-write parent cleanup

`secure_file.write_new_exclusive(path, bytes, create_parent=true,
private_parent=false)` and its text wrapper retain their signatures, admission
rules and exclusive/no-replace creation behavior. After a normal backend return,
they attempt the parent close once and accept only boolean `true` as confirmation.

| Backend return | Parent close return | Public result |
| --- | --- | --- |
| Success | `true` | Original success map, unchanged |
| Failure | `true` | Original failure map, unchanged |
| Success | `false`, nil or non-boolean | `ok:false`, fixed cleanup error, original operation metadata, `cleanup_uncertain:true`, `parent_close_confirmed:false`, `write_backend_succeeded:true`, `publication_may_have_occurred:true` |
| Failure | `false`, nil or non-boolean | Original error value and extra fields preserved, with `cleanup_uncertain:true`, `parent_close_confirmed:false` |

On unconfirmed cleanup, the facade shallow-copies the backend result rather than
changing it. Cleanup fields are reserved for the facade. Error values are never
stringified or logged. Backend publication/durability fields remain evidence of
the backend operation, not of parent cleanup. An inherited retry hint does not
override `cleanup_uncertain`: callers must stop automatically repeating effects.

`write_backend_succeeded` means only that the backend reported success. Complete
or partial output can exist even when the public operation fails. This is not
atomic content visibility, crash durability, verified deletion or proof of physical
resource reclamation. No write, close, reopen or removal is retried by this change.

## Exceptions remain distinct

No catch was added around either backend write or close. A backend exception still
propagates before the subsequent parent close is reached. A close exception still
propagates after a returned backend result and can mask a returned primary failure.
The new preservation guarantee covers **returned** cleanup failures only. Ordinary
source catch does not expose the opaque exception payload needed for general
original-error-preserving cleanup. That limitation is not fixed here.

## Effectful callers

`polyglot_web.bundle` stops on `cleanup_uncertain` before its next temporary-name
attempt. Its existing retry behavior for other failures is unchanged and is not
certified by this change. `polyglot_build` stops on an uncertain session-marker
close before attempting to remove its output directory. It retains the unconfirmed
namespace and raises a fixed diagnostic; this is not a rollback implementation.

Existing document/product adapters already reject `ok:false`; their generic
unconfirmed-copy messages remain applicable. Batch completion counts confirmed
prior results, not every file that may exist. Pinned products need a separate
stdlib dependency/lock update and installation before receiving the behavior.

Other secure-file methods and direct DirFS callers are unchanged. The Linux
exclusive backend still has separate lower-level close/unlink outcome limits.
macOS and Windows close implementations do not aggregate every native close result;
checking their return boolean does not establish full native cleanup correctness.

## Reproducible qualification

Use an existing retained Linux runtime release and an immutable source snapshot
containing the three original modules before this change:

```sh
python3 tests/tooling/secure_file_exclusive_close_check.py \
  --runtime /path/to/runtime-release \
  --baseline /path/to/original-source \
  --report /path/to/private-report.json
```

Run serially under the coordinator's **existing** heavy lock. The driver does not
create or replace a lock. It requires Python 3.11+, Bubblewrap and the Linux x86-64
libc profile. Only disposable scratch directories are writable inside the runtime
sandbox; network, GUI, environment access and spawning are denied. Model cases
also deny FFI and process execution. Real I/O uses the existing 15-symbol profile.

The semantic helper accepts a complete facade copy and a counted resource-free
backend. The paired baseline/candidate matrix covers both public APIs, success
and known failure crossed with `true`, `false`, nil and `1`, exact UTF-8 bytes,
extra metadata, string/map error preservation, unchanged backend maps, argument
and parent-admission rejection. Separate uncaught-throw cases verify unchanged
backend/close exception messages. Complete caller modules execute through public
`bundle` and hash-authorized `build_language` entrypoints with synthetic adapters;
no compiler is launched. Real I/O checks new files and existing/hardlink/symlink/
missing-parent refusals, with unchanged input hashes and metadata.

Optional product qualification uses each product's existing synthetic CLI cases
and unchanged pinned dependency, mounted read-only:

```sh
python3 tests/tooling/secure_file_exclusive_close_check.py \
  --runtime /path/to/runtime-release --baseline /path/to/original-source \
  --product /path/to/pbtablecheck --product /path/to/pbtaskscope \
  --product /path/to/pbsmooth --report /path/to/private-product-report.json
```

Each product must sit beside its own `pblang` dependency copy. The four cases per
product are pinned-normal, pinned-close-false, candidate-close-false and
candidate-close-throw. Each calls the unchanged save adapter, checks the original
expected document, preserves pre-existing inputs/metadata and retains exact full
output bytes and mode 0600. The false/throw models first perform one successful
real close, then inject the outcome in a private full-facade copy. They verify
result propagation, **not** an actual kernel close failure. Old product fixtures
and their expectations are not modified.

Qualification on retained runtime72: the paired model/caller/real-I/O checks and
all twelve product cases passed. Original failed harness attempts are retained in
the private execution evidence. No macOS/Windows live tests, rollback acceptance,
opaque-exception cleanup solution, product adoption or installation is claimed.
