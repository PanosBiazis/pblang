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

Other secure-file methods retain their separate cleanup contracts. The exclusive
Linux/POSIX backend changes below also apply to direct DirFS callers. macOS and
Windows outer close implementations do not aggregate every native close result;
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


## Exclusive backend failures retain possible output

Linux `linux_dirfs.write_new_exclusive` and portable
`posix_dirfs.write_new_exclusive` no longer call `unlinkat` after a failed write.
The held parent descriptor identifies a directory, not the file currently named
by its child entry. Another writer can rename the newly created file and replace
its name before cleanup; unlinking that name would remove the replacement.
A separate identity check followed by unlink would still allow this race.

Both backends attempt the created file close and their own parent close once,
independently. Only boolean `true` confirms a close. A returned or thrown close
failure does not suppress the other owned close, trigger a retry, or erase the
fixed primary write diagnostic. The POSIX change uses an exclusive-write-only
close helper; other portable cleanup operations are not certified by it.

Successful writes with both closes confirmed retain their existing success maps.
After creation, any write or close failure returns these fields:

| Field | Meaning |
| --- | --- |
| `ok:false` | The operation is not confirmed successful |
| `write_completed` | The byte-writing call returned normally; no durability claim |
| `file_close_confirmed` | One close of the created descriptor returned boolean true |
| `dirfs_parent_close_confirmed` | One close of the backend-owned parent returned boolean true |
| `output_may_exist:true`, `publication_may_have_occurred:true` | Partial or complete output may remain; current path identity is not known |
| `rollback_attempted:false`, `rollback_complete:false` | No implicit pathname rollback was attempted or verified |
| `cleanup_uncertain:true`, `retry_safe:false` | Stop automatically repeating effects or removing names |

`cleanup_uncertain` also covers retained possible output when both descriptor
closes succeed. It is not solely a close-error flag. The fixed write diagnostic
is `exclusive destination write failed; output may remain`; after a completed
write with an unconfirmed close it is `exclusive destination cleanup could not
be verified; output may remain`. No exception payload, file content, or path is
included in these new diagnostics.

The facade preserves backend fields on a normal return. Its separate
`parent_close_confirmed` describes the outer root handle, so it does not overwrite
`dirfs_parent_close_confirmed`. Exclusive-open rejection still never writes or
removes anything. If closing that already-owned parent is unconfirmed, its
original rejection gains `cleanup_uncertain`, `dirfs_parent_close_confirmed:false`
and `retry_safe:false`, without claiming that this attempt created output.

The existing complete `polyglot_web.bundle` and `polyglot_build` callers now stop
on this backend uncertainty before their next retry or output-directory removal.
Document adapters already reject `ok:false`. Partial files can remain even when
unlink authority is available. Callers must not implement a pathname deletion
fallback. This fixes replacement-entry data loss and false cleanup/success
reporting; it does **not** implement atomic output, verified rollback, crash
recovery, general opaque-exception preservation, or cleanup for named staging in
other APIs. The facade exception limits above still apply.

### Paired backend qualification

```sh
python3 tests/tooling/dirfs_exclusive_failure_check.py \
  --runtime /path/to/retained-runtime \
  --baseline /path/to/immutable-original-source \
  --report /path/to/private-report.json
```

Run serially under the existing coordinator heavy lock with a finite acquisition
deadline. The harness uses complete private module copies, resource limits,
Bubblewrap and synthetic scratch files. Its Linux replacement fixture writes
16 real bytes, pauses at a bounded rendezvous, renames that inode, creates a
distinct replacement, and then forces the write failure. The original cleanup
removes the replacement; the changed cleanup preserves both inode identities and
bytes. Native cleanup itself is unchanged in that fixture. A separate real
`RLIMIT_FSIZE=16` case exercises a kernel write failure with the existing
15-symbol profile and with test-only unlink authority. That extra authority is
never added to a product or installation.

Whole-module Linux/POSIX models cover success/failure and boolean, nil, numeric,
and thrown close outcomes, plus admission rejection. Real Linux close failures
are not injected. POSIX/macOS qualification is source/model evidence only; no
macOS hardware or operational availability is implied. Complete caller models
use the unchanged facade and changed backend to verify one write, no retry and
no subsequent remove. Sources, baseline bytes, private harness and reports must
be frozen together for independent review.
