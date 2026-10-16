# Bounded read parent cleanup

`secure_file.read_bytes` and its `read_text` caller attempt to close their acquired
parent exactly once after a normal backend return or catchable backend exception.
Only boolean `true` confirms close. False, nil, other types and catchable close
exceptions produce a fixed failure map without bytes/text. No retry occurs.
Successful reads and backend error maps are unchanged when close is confirmed.
Backend exceptions now become `ok:false` with a fixed error rather than escaping.

This contract starts after `_open_parent` returns an owned successful handle.
It does not cover partial acquisition, VM termination, process death, native crashes,
or arbitrary cleanup inside the backend. An attempted failed close is not evidence
of physical release. Other facade operations have separate ownership paths and are not changed by
these fixes. Prefix and optional reads are covered below.

The semantic matrix substitutes a counted, resource-free backend into a complete
copy of the facade: 36 read/close combinations plus four admission cases. The tooling
runner uses the existing bounded process/isolation harness. NetVis adds independent
real-descriptor probes and installed-product acceptance. Neither modeled faults nor
Linux acceptance establishes Windows/macOS native behavior.

## Prefix reads

`read_text_prefix` now applies the same owned-parent rule independently. After a
catchable prefix backend exception it attempts one close and returns the fixed
`secure prefix backend failed` error if close succeeds. Any close result other
than boolean `true`, including a catchable exception, returns only `ok:false`, a
fixed diagnostic, `cleanup_uncertain:true`, and `parent_close_confirmed:false`.
The facade neither retries close nor returns bytes/text under cleanup uncertainty.
A normal backend error map is retained after a confirmed close.

Valid prefix results, default arguments and the byte ceiling are unchanged.
Only a proven incomplete UTF-8 scalar suffix at a truncated byte boundary can be
trimmed; malformed UTF-8 or an incomplete scalar at actual EOF still fails.
`polyglot_web.browser_plan` uses this prefix API and now receives a normal rejected
plan on a catchable backend exception after cleanup, without launching a browser.

The focused runner is `tests/tooling/secure_file_prefix_cleanup_check.py`. Supply
`--runtime` with an existing materialized release directory, `--baseline-source`
with the unchanged old facade, and a fresh `--report` path. Before any fixture runs,
the runtime facade must exactly match the reviewed source; a mismatch leaves an
explicit rejected report and exits nonzero. Run serially under the
coordinator's shared test lock. It checks 18 modeled read/close combinations, six
admission cases, real UTF-8/limit/consumer reads, unchanged fixtures, and parent
file-descriptor counts. The baseline negative control exposes parent accumulation
under a synthetic backend exception; no kernel/native fault is induced. The
runner uses streaming pre-append process-output limits and source-hidden,
network-isolated Linux fixtures. A real successful close followed by synthetic
false/throw tests projection only. This does not prove physical release on a
native failed close, Windows/macOS execution, or resilience to process termination.

## Optional reads

`read_optional_text` now closes its successfully acquired parent exactly once,
including catchable listing, reading and reply-interpretation failures. Only
boolean `true` confirms that close. Every other close result or catchable exception
returns a fixed error with `cleanup_uncertain:true` and
`parent_close_confirmed:false`; neither text/bytes nor a successful missing result
is returned. Close is never retried. Catchable operation exceptions become a fixed
`secure optional read backend failed` diagnostic after a confirmed close.

On Linux/macOS an absent name is considered missing only when the returned listing
explicitly has boolean `truncated:false`. An unseen name in a truncated listing,
or one whose completeness is unspecified/malformed, now produces an error. A
found name still uses the same descriptor-bound read, including when completeness
is unspecified. Listing replies must have boolean `ok`, and successful entries
must be an array of at most 4096 strings. The legacy backend may allocate its full
listing before clipping; this facade change does not establish a preallocation
bound or change that listing API.

The Windows branch retains its backend-provided missing result: only a rejected
read with boolean `missing:true` is converted to successful absence. Ordinary
read error maps are preserved after confirmed close. Successful backend reads
must contain a byte array and a nonnegative integral size equal to its length and
within the requested limit, as all three existing backends already return.
Malformed replies, including nonfinite or mismatched sizes, fail rather than
producing a misleading success. Empty files and strict UTF-8 decoding are unchanged.
Defaults and normal success maps remain unchanged. Missing-file results describe
the observation at the held-parent operation, not an atomic guarantee about future
filesystem changes.

`tests/tooling/secure_file_optional_cleanup_check.py` accepts `--runtime`,
`--baseline-source`, a fresh `--report` and optional `--consumer-root` pointing to
PBJobTracker sources. It checks counted ownership matrices, modeled Windows
routing, source/runtime admission, unchanged-baseline negative controls, actual
Linux descriptors and 4096/4097-entry directories. Consumer checks use the full
PBJobTracker load/service/policy source with only its secure-file import redirected
to the fixture facade. The existing bounded isolation helper supplies streaming
process-output caps and source-hidden/network-off execution. Run under the shared
coordinator lock. These tests do not prove native Windows/macOS operation, physical
release after a failed native close, complete heap bounds, or cleanup after VM
termination. No consumer state is saved during these checks.

## Directory listings

`list_directory` now closes its successfully acquired directory root handle (`fs`)
exactly once, including catchable backend listing and reply-validation failures.
Only boolean `true` confirms that close. Every other close result or catchable
exception returns a fixed error (`secure directory root cleanup could not be verified`)
with `cleanup_uncertain:true` and `root_close_confirmed:false`; `entries` are never
exposed under cleanup uncertainty and close is never retried. Catchable backend
exceptions become a fixed `secure directory listing backend failed` diagnostic
after a confirmed close.

Successful backend listings must have boolean `ok:true`, boolean `truncated`, and
`entries` as an array of at most `max_entries` strings. Malformed replies fail
closed with `secure directory listing result is invalid` while ordinary backend
error maps (`ok:false`) are preserved after confirmed close.

`tests/tooling/secure_file_list_directory_cleanup_check.sapp` is a native PBLang
qualification runner (`pblang run --allow-all tests/tooling/secure_file_list_directory_cleanup_check.sapp -- --baseline-source <path> --report <path>`)
that verifies 156 modeled list/close combinations, 16 admission cases, baseline
rejection, real Linux directory listings, real descriptor leak elimination on
synthetic listing exceptions, and post-close uncertainty projection.

## Atomic writes and entry removals

`write_atomic`, `write_text_atomic`, `remove`, and `remove_file` now close their
successfully acquired parent handle (`parent.fs`) exactly once, including catchable
backend operation and reply-validation failures. Only boolean `true` confirms that
close. Every other close result or catchable close exception returns `ok:false`,
`cleanup_uncertain:true`, and `parent_close_confirmed:false` without retrying close.
Catchable backend exceptions become fixed diagnostics (`secure atomic write backend failed`,
`secure remove backend failed`, `secure file remove backend failed`) after a confirmed
close. Malformed non-map or non-boolean-`ok` backend replies fail closed with fixed
invalid-result diagnostics (`secure atomic write result is invalid`,
`secure remove result is invalid`, `secure file remove result is invalid`).

When the backend operation succeeded (`ok:true`) before an unconfirmed parent close:
- `write_atomic` and `write_text_atomic` return `ok:false`,
  `error:"secure atomic-write parent cleanup could not be verified"`,
  `cleanup_uncertain:true`, `parent_close_confirmed:false`,
  `write_backend_succeeded:true`, and `publication_may_have_occurred:true`, while
  retaining non-`ok` metadata without mutating the backend map.
- `remove` and `remove_file` return `ok:false`, fixed cleanup diagnostics
  (`secure remove parent cleanup could not be verified` or
  `secure file remove parent cleanup could not be verified`), `cleanup_uncertain:true`,
  `parent_close_confirmed:false`, `remove_backend_succeeded:true`, and
  `removal_may_have_occurred:true`. `remove_file` attaches `namespace_absent:true`
  and `absence_evidence:"descriptor-held file-only unlink"` on a fresh copy only when
  both backend removal and parent close succeed.

`tests/tooling/secure_file_remove_write_cleanup_check.sapp` is a native PBLang
qualification runner (`pblang run --allow-all tests/tooling/secure_file_remove_write_cleanup_check.sapp -- --baseline-source <path> --report <path>`)
that verifies 144 modeled write/remove/close combinations, 12 admission cases,
baseline rejection, real Linux atomic writes and removals, real descriptor leak
elimination (`0` vs baseline `+6`) on synthetic backend exceptions, and post-close
uncertainty projection.

## Immutable readers and private directories

`open_immutable_reader`, `close_immutable_reader`, `publish_immutable_reader_range`,
`publish_immutable_reader_range_private`, `publish_immutable_reader_range_private_temp`,
`create_private_directory_new`, `_temporary_container_status`, and `private_directory_status`
now enforce single-close parent/root cleanup, catchable backend exception redaction,
and reply validation:
- `open_immutable_reader` validates that a successful snapshot reply is a map with
  boolean `ok:true`, `active:true`, `immutable:true`, and an integer `size` within
  `[0, max_bytes]`. If snapshot creation fails, throws, or returns an invalid shape,
  the acquired parent handle is closed once (`parent_close_confirmed:false` and
  `cleanup_uncertain:true` when parent close is not boolean `true`).
- `close_immutable_reader` requires boolean `true` from both `close_child_descriptor`
  and `close` before reporting `ok:true`, `snapshot_close_ok:true`, and
  `parent_close_ok:true`.
- `publish_immutable_reader_range` and `publish_immutable_reader_range_private`
  close their acquired destination parent handle once after normal return or
  catchable backend exception, never mutate backend maps in place, and report
  `publish_backend_succeeded:true` and `publication_may_have_occurred:true` when
  backend publication succeeded before an unconfirmed parent close.
- `create_private_directory_new`, `_temporary_container_status`, and
  `private_directory_status` wrap post-acquisition backend calls in `try ... catch`
  and verify boolean `true` on every parent/root close.

`tests/tooling/secure_file_immutable_private_cleanup_check.sapp` is a native PBLang
qualification runner (`pblang run --allow-all tests/tooling/secure_file_immutable_private_cleanup_check.sapp -- --baseline-source <path> --report <path>`)
that verifies 180 modeled immutable/private/close combinations, 14 admission cases,
baseline rejection, real Linux immutable reader and private directory operations,
real descriptor leak elimination (`0` vs baseline `+8`) on synthetic backend
exceptions, and post-close uncertainty projection.

## Locked transactions

`append_text_bounded_locked`, `_inspect_private_file`, `_finish_locked`,
`private_compare_and_swap`, and `consume_nonce_once` now enforce single-release
sibling lock cleanup, single-close parent/root cleanup, catchable backend exception
redaction, and reply validation:
- `append_text_bounded_locked` catches lock acquisition, new-file creation, metadata,
  bounded read, and atomic replacement failures at the owned parent/lock boundary,
  releases the acquired lock once and closes the parent once with boolean `true`
  verification, never mutates backend maps in place, and reports
  `cleanup_uncertain:true` (plus `append_backend_succeeded:true` when the backend
  write succeeded) if lock release or parent close cannot be verified.
- `_finish_locked`, `private_compare_and_swap`, and `consume_nonce_once` catch
  post-acquisition policy, lock, readback, and write exceptions, ensure both
  `release_lock` and `close` are attempted once with boolean `true` verification,
  and fail closed with `cleanup_uncertain:true`, `effect_admitted:false`,
  `effect_completed:false`, and `evidence_indeterminate:true` (plus
  `transaction_backend_succeeded:true` when the transaction succeeded before an
  unconfirmed release/close).

`tests/tooling/secure_file_locked_transaction_cleanup_check.sapp` is a native PBLang
qualification runner (`pblang run --allow-all tests/tooling/secure_file_locked_transaction_cleanup_check.sapp -- --baseline-source <path> --report <path>`)
that verifies 216 modeled locked-transaction/release/close combinations, 12 admission
cases, baseline rejection, real Linux locked appends, CAS, and nonce burns, real
descriptor and flock leak elimination (`0` vs baseline `+12`) on synthetic post-lock
exceptions, and post-release uncertainty projection.
