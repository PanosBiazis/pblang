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
GC, secure erasure or OS close-failure recovery. Disk verification policy is unchanged.

Focused byte-ownership tests cover real miss/hit loads, result element and length
changes, cyclic caller results, prefetch, pinning, cache bypass, zero-length shards,
eviction, failed corrupted reread and close. The existing paired lifecycle examples
now demonstrate independent byte results, including their source-hidden installed run.
