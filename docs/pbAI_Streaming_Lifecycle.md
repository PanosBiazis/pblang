# Streaming session lifecycle

The shared `stdlib:llm_tiered_store.sapp` owns the lifecycle used by standalone
pbAI's `streaming_*` facade. An explicit `create` / `streaming_open` produces a
session; closing it prevents future operations on that session. A new open produces
an independent session and does not revive an older one.

Previously only loading checked whether the session was closed. Route observation
and prefetch queueing could mutate a closed session, rebalancing could report success,
and operations on a failed creation result could throw. All operational entry points
now check the lifecycle before inspecting or coercing their operation arguments.

| Operation after close or failed creation | Result |
| --- | --- |
| load, route observation, queue/run prefetch, hot rebalance | `ok:false`, `code:"closed"` |
| expert prediction | empty array |
| preferred-root query | `nil` |
| unpin, repeated close | `false` |
| status after successful creation then close | detached status with `ok:false` and empty resident cache/queue |
| status on failed creation, nil or non-handle values | `ok:false`, `code:"closed"` |

A creation error remains unchanged; passing it to another operation does not replace
or mutate its original error code/message. Valid active-session return shapes are
unchanged. Rejection does not update counters, heat, recency or the prefetch queue.

Status now copies all eleven scalar statistics into a new map. Old snapshots do not
change when the session runs, and caller edits cannot modify internal counters or
other snapshots. Closing releases the session's cache references without altering
successful byte-array results that a caller already holds.

The handle is still a mutable in-process map intended for trusted callers. Lifecycle
checks are not authentication, a sandbox, or a defense against a caller deliberately
forging or rewriting a complete handle. This change does not fix general garbage
collection, guarantee secure erasure or successful OS resource cleanup, add async
cancellation, or provide universal platform support.
No benchmark or performance improvement is claimed for this correctness change.

The focused lifecycle tests use private synthetic four-byte shards. They cover active,
closed, repeated close, failed creation, malformed non-handles, cyclic rejected
arguments, snapshot mutation and independent reopen. Existing cache/eviction/mirror
and pbAI streaming suites remain part of verification. The runnable lifecycle examples
require a private `PBAI_STATE_ROOT`; source-hidden installation tests run copied
examples from a foreign caller directory with a decoy dependency manifest.


## Supported shard read limit

The default `max_shard_bytes` is now 67,108,864 bytes (64 MiB), matching the existing
bounded reader. Previously the default was 256 MiB, which that reader rejected even
for a four-byte shard because its requested read limit was unsupported. A default
session now reads a small shard successfully without an override.

`create` rejects a configured read limit above 64 MiB with `code:"config"` before
opening any root. It also rejects malformed direct configuration limits. Larger
explicit budgets are not silently downgraded. The existing `config` normalization of
other inputs is unchanged. Applications needing larger individual shards must split
them into supported parts or wait for a separately verified reader expansion.

Boundary tests use the exact 64 MiB limit with a four-byte file, then reject 64 MiB+1,
256 MiB and 1 GiB configurations before an unavailable root is touched. They do not
allocate a 64 MiB fixture, and no throughput or memory-performance claim is made.


## Caller-owned shard bytes

A successful load now returns byte arrays that are independent of resident storage
and of previous load results. On a cache miss that is admitted, the store retains a
copy of the verified file bytes and transfers the original read result to the caller.
On a cache hit, it returns a fresh copy of the resident bytes. A bypassed read is
already independent and does not create an unnecessary cache copy.

Previously returned bytes shared the cache array: changing a byte or appending to a
result could poison future results while they still reported `verified:true`, and
could make the actual array length disagree with its declared size/resident count.
The new ownership boundary applies to ordinary, pinned, prefetched and empty shards.
Caller edits, including cyclic replacements, cannot reach resident bytes through a
load result. Failed integrity reads expose no bytes and do not replace a healthy
resident entry. Eviction and close leave earlier caller-owned results unchanged.

This adds a linear copy and one additional array on each cached load. The configured
cache budget still counts resident payload bytes; it does not bound caller-held
results, temporary copies, VM representation overhead or total process RAM. No
speedup or memory reduction is claimed. The byte-copy helper is deliberately limited
to scalar bytes read by this module; it is not a general recursive object copier.

The public session map is still mutable. Directly rewriting its cache/config/manifest
or forging a handle is outside this trusted-caller contract. The fix does not add
handle authentication, snapshot consistency against concurrent disk writes, general
GC or secure erasure. Disk verification policy is unchanged.

Focused byte-ownership tests cover real miss/hit loads, result element and length
changes, cyclic caller results, prefetch, pinning, cache bypass, zero-length shards,
eviction, failed corrupted reread and close. The existing paired lifecycle examples
now demonstrate independent byte results, including their source-hidden installed run.


## Terminal close and unconfirmed cleanup

`close` still returns a boolean. A first close returns true only if every held directory
handle returns an explicit boolean true from the underlying close operation. False,
exceptions and non-boolean results are treated as unconfirmed cleanup; the result is
false. Repeated or reentrant close also returns false without another cleanup attempt.

The session becomes closed, drops its resident cache references and clears queued
prefetch work before the first close effect. Each owned handle is detached before its
attempt; later handles are still attempted if an earlier one fails or throws. No retry
is performed because an ambiguous descriptor might have been released and reused.
This is terminal failure handling, not proof of OS resource release or a retry mechanism.

`status().cleanup` is an independent snapshot containing `attempted`,
`confirmed_handles`, `unconfirmed_handles`, `first_error` and `retry_supported:false`.
Before close the counters are zero and first_error is nil. The first failure remains
one of `close_unconfirmed`, `close_threw`, or `close_invalid_result`; raw exception text,
paths and descriptor identities are not retained. An earlier operation error or byte
result remains unchanged. An unconfirmed count must never be interpreted as released
resources. Existing read/load status and output formats otherwise remain unchanged.

Deterministic fixtures substitute only the directory backend and copy the exact owner
and standalone streaming facade. Fourteen direct/facade cases cover success, false,
throw, nil, numeric/string truthy returns and mixed failures across three handles.
Callbacks attempt reentrant close, cache reads, queueing and status-snapshot mutation.
The fixtures own no OS descriptors; they demonstrate control flow, not real syscall
failure recovery. Normal local and source-hidden installed examples separately check
successful real directory-handle cleanup. Direct mutation/forgery of the public session
or underlying handle structures is still outside the trusted-caller contract.


## Failed initialization and bounded acquisition

`create` validates the mirror container before opening the primary directory. A
non-array container returns the existing `config` error without acquiring a handle.
There is now a hard maximum of **64 optional mirrors** (`MAX_MIRRORS`), plus one
primary root. A larger list returns `config` before any acquisition. This is a
resource-bound compatibility change: callers previously supplying more than 64
mirrors must select a smaller set; entries are never silently truncated.

The acquisition plan copies scalar mirror roots and normalized weights before the
first open. Backend callbacks cannot extend or rewrite that plan by changing the
original caller list. Normal unavailable or malformed optional mirror records still
follow the existing skip policy. An unavailable primary still returns the original
root error. This snapshot covers initialization only; the published session remains
a trusted mutable map, not an authenticated or immutable configuration object.

A thrown or malformed backend acquisition aborts initialization with `code:"root"`
and fixed message `shard root initialization failed`. Every earlier known directory
handle receives exactly one cleanup attempt using the same retirement helper as
terminal close. A failed cleanup never replaces this primary initialization error
and does not stop later known handles from being attempted. No usable partial
session or handle identity is returned.

Acquisition failures add a detached `cleanup` diagnostic with the same fixed fields
as session status, plus `acquisition_uncertain`. The latter is true when acquisition
threw or returned a malformed value: the backend may have acquired a resource it
never returned. Zero `unconfirmed_handles` describes only **known returned handles**;
it must not be interpreted as recovery of such an unknown resource. No retries are
performed. Passing the failure result to close/load/status remains inert and does
not change its original code/message or repeat cleanup.

Seventy exact-source direct/standalone-facade fixture cases cover preflight before
side effects, the 64/65 boundary, skipped malformed/unavailable optional roots,
caller-list mutation during acquisition, thrown/nil/numeric acquisition outcomes,
and successful/false/throw/nil/numeric/string/mixed rollback outcomes. They own no
real OS descriptors. Separate actual examples and installed source-hidden checks
exercise invalid-plan rejection and successful real directory cleanup.

This is bounded synchronous initialization cleanup, not a guarantee against native
crashes, process termination, allocation/OOM failure, malicious raw session mutation,
or unreturned backend owners. No general cross-platform or performance claim is made.
