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
