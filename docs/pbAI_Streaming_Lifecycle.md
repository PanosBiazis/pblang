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


## Read failures, mirror failover and terminal results

A confined read must return a map with an explicit boolean `ok:true` before its bytes
are considered. Missing fields, non-map responses and truthy non-boolean success flags
are failures, including when digest verification is explicitly disabled. Existing
array/length/digest checks remain in place; rejected envelopes and failed checks
expose no bytes. The trusted directory backend still owns the scalar byte-array
representation contract; this change does not add a general object validator.

Each root read is now an exception boundary. A caught backend or decoding exception
becomes the fixed `read` error `shard is not readable`, and the next configured mirror
is attempted. Raw backend error text is not retained or returned. Each root is tried
at most once per load; the existing maximum is 65 roots including the primary. When
all roots fail, the existing last-attempt error policy applies, `rejected_reads`
increases once, and healthy resident cache entries and prior caller bytes survive.
A later explicit load can recover. A successful fallback increments `verified_reads`
and `mirror_failovers` once, without counting the overall request as rejected.

After every attempted read, the session is checked again before accepting bytes or
updating read-success/rejection counters. If it closed during the backend call, the
load returns `closed` and no further mirror is tried. A late success cannot repopulate
the cache or publish bytes. Prefetch reports that item as closed, leaves the cleared
queue empty and starts no next item. For an ordinary read failure while still open,
prefetch reports the failed item and continues its remaining bounded work.

Sixty-six exact-source direct/standalone-facade fixtures cover eleven read failure
forms, healthy fallback versus exhausted mirrors, cache preservation and later
recovery, mixed failed/successful prefetch, strict success flags without digest
verification, and close callbacks during load/prefetch with multiple read outcomes.
They simulate backend responses and own no OS descriptors. The existing real-file
store suites remain separate integration checks. The paired source-hidden installed
examples corrupt the selected preferred on-disk shard, recover verified bytes from
the other real root, and confirm successful cleanup of both directory handles.

This is synchronous control-flow containment. It does not reclaim descriptors a
backend failed to return, prove real native/syscall recovery, preempt a blocked read,
add asynchronous cancellation or guarantee multi-threaded atomicity. Direct mutation
of the trusted session/config/manifest remains outside this contract. No performance,
universal platform, AGI or ASI claim follows from these fixtures.


## Literal shard filenames

The manifest's safe relative path contract now reaches the confined reader without
legacy root aliases: the store uses `linux_dirfs.read_bytes_relative`. A shard file
named exactly `root` is a normal child file. Previously that manifest was accepted,
but the legacy reader rejected the same string as a root alias and the load failed.
No path rewriting, alias fallback or normalization is added. This changes shard
paths inside an opened root; root-directory opening and its accepted-root forms
are unchanged.

Nested `root/item` and `root/root/item`, case differences, leading/trailing spaces,
Unicode names and empty files keep their existing literal behavior. This change does
not redefine those names. Empty paths, absolute paths, dot/traversal components,
empty components, trailing separators, backslashes, NUL and oversized components
remain rejected by manifest validation. A directory or symlink cannot become a shard
merely because its name is `root`.

The adopted API uses the same held directory, no-follow component traversal,
anonymous-file snapshot, byte limits and result schema as the legacy bounded reader.
Manifest size/SHA checks, cache/prefetch behavior and mirror selection remain in the
store. The dependency must provide `read_bytes_relative`; release locks bind the
shared library tree. There is no fallback to legacy alias semantics if that API is
missing. The underlying general `read_bytes` API and its other callers are unchanged.

Thirty real-filesystem scenarios per product cover exact literal bytes, a cached
`root` shard, nested names, held-root pathname replacement with RAM caching disabled,
missing files, final/intermediate symlinks to private synthetic targets, directory
rejection, twelve invalid manifest paths, mirror prefetch with a corrupted preferred
`root`, all-corrupt mirrors, and an actual file exceeding its declared read limit.
Existing fault fixtures export only the literal read API so the new call remains
covered by initialization, close and read-boundary regressions. Both source-hidden
installed examples now use the actual filename `root` for their mirror fallback.

These are Linux CPU results using local synthetic files. The change adds no network,
FFI symbols, backend implementation, general cleanup guarantee or all-platform claim.


## Prefetch batch ownership and cooperative cancellation

Only one `run_prefetch` may execute per session. A nested call returns the fixed
`busy` error before inspecting its item limit and without changing counters or the
queue. A closed session still returns `closed` first. Previously a reentrant batch
could read one shard twice, double-charge its resident bytes and drop another queued
item while both calls reported success.

The batch detaches each item before starting its load. Its active ID stays deduplicated
until the load returns. Queueing during that load preserves pending work and respects
the configured pending-queue capacity; the active item is separate from that capacity.
Normal execution may consume newly queued work within its original bounded item limit.
Batch ownership is released on ordinary completion and caught exceptions. Unexpected
execution exceptions produce the fixed `prefetch` error without raw exception text;
an already-started item is not implicitly retried.

`cancel_prefetch(store)` is an explicit, synchronous operation. It clears pending
items and returns `ok:true`, format `pblang.llm.prefetch_cancel.v1`, a detached array
`cancelled` containing those IDs in order, `running` and `queued:0`. It leaves the
session open and preserves resident cache, existing counters and caller-held bytes.
Repeated cancellation of an empty idle queue succeeds without other effects. Closed,
failed or malformed non-handles return the existing `closed` error without mutation.

If a batch is active, cancellation also asks it to stop **after its current load**.
That load retains its existing bounded mirror policy and may complete/cache verified
bytes or report a read failure. Its ID is excluded from the cancellation array.
The run result adds boolean `cancelled`; when true, overall `ok` is false while
`completed` and `failed` describe work that actually finished. New work explicitly
queued after cancellation survives, but this cancelled batch does not execute it.
A later explicit `run_prefetch` starts a fresh batch. A second nested run remains
`busy` even after cancellation. Terminal close takes precedence and still prevents
late bytes from reaching the cache.

The operation is exposed as `cancel_prefetch` on both dedicated streaming facades
and `streaming_cancel_prefetch` beside the existing central `streaming_*` wrappers.
Capabilities declare `cooperative_prefetch_cancellation:true` and
`preempts_active_read:false`; `asynchronous_io` remains false. Existing two-argument
`run_prefetch` calls remain valid; `cancelled` is an additive result field.

This is cooperative cancellation between synchronous loads, not a scheduler, native
read interruption, thread synchronization, general resource reclamation or protection
against direct rewriting of the trusted public session map. A native blocked read
still has to return before the batch can observe cancellation. The explicit operation
does not install signal handlers, launch workers, activate models or access a network.

Sixty-three simulated cases exercise the exact owner through direct, standalone and
embedded dedicated-facade calls. They cover nested rejection, queue/cache accounting,
cancellation before work, active cancellation with four read outcomes, newly queued
work, active-ID deduplication, repeated cancel, zero limit, close precedence and invalid
handles. The simulated backend owns no OS descriptors. Real-file suites and the
source-hidden shared/standalone examples separately verify normal operation and idle
cancellation before an optional real prefetch.


## Ordinary load reentry

Session ownership now covers the complete public `load` operation, from before ID
coercion through read/verification, cache admission or cache-hit copy. A nested load
on the same open session returns the fixed `busy` error without reading, pinning,
evicting, touching heat/recency, changing counters or consuming queued work. The guard
applies to the same ID, another ID, an already resident ID and malformed nested
arguments. A closed session still returns `closed` first. A different session remains
independent; this is not a global lock.

`run_prefetch` also refuses admission while an ordinary load owns the session, before
normalizing its item limit or changing the queue. The owning prefetch executor may
still call ordinary load normally, and its backend callbacks cannot start another
load or batch. Previously an ordinary callback could reload the same shard, charge
resident bytes twice, or run prefetch inside the load and publish another cache entry
for an ID already admitted by the nested batch.

An ordinary active ID is excluded from new prefetch queue admission using the existing
`resident-or-queued` reason. Optional work queued before an ordinary load remains
pending until an explicit batch or cancellation. `cancel_prefetch` is still available:
it cancels pending optional work and reports whether a **prefetch batch** is running;
it does not cancel an ordinary active load. New work queued after cancellation survives,
but no nested executor may run it until the current load returns. The existing active
prefetch cancellation and terminal-close policies are unchanged.

The load guard and active ID are retired after success, unknown ID, read failure,
terminal close or a caught execution exception. Unexpected exceptions outside the
existing read boundary return the fixed `load` / `shard load failed` error without raw
text. This is not transactional rollback of arbitrary caller mutations or allocation
failures. The private operational fields are deliberately absent from the established
status snapshot, whose counters and schema remain unchanged.

This is a compatibility tightening for same-session synchronous callbacks: nested
loads (including cache hits) must be deferred until the owning operation returns or
use a separately opened session. No waiting/retry loop, worker, mutex or native-read
preemption is introduced. Direct forgery/mutation of the trusted public handle,
native crashes, OOM and genuine multi-thread races remain outside this guarantee.

One hundred thirty-two adversarial simulated cases per test execution cover direct,
standalone and embedded dedicated-facade entrypoints: nested same/different/cached/
missing/cyclic IDs with both pin choices; ordinary and prefetch owners; nested prefetch;
read errors and recovery; cancellation/requeue; close precedence; invalid handles; and
independent sessions. Exact counters, cache bytes, pinned entries, queue order, heat,
recency, prior outputs and closed snapshots are checked. Existing real filesystem and
source-hidden examples separately verify normal sequential use. These callback tests
substitute the directory backend and own no actual OS descriptors.


## Plan-before-commit cache admission

A successful verified read is distinct from cache admission. The result can remain
`ok:true`, `verified:true`, `cached:false` when pins or entry/byte budgets prevent
admission. That bypass now preserves every existing resident and does not increment
`evictions`. `loads`, `misses`, `verified_reads` and `bypasses` still record the work
that actually occurred; the attempted request is not rolled back.

Previously room-making evicted unprotected entries one at a time while looking for
space. With an 8-byte cache, a protected 4-byte resident, a warm unprotected 4-byte
resident, and an incoming verified 8-byte shard, it evicted the warm resident and
then discovered admission was impossible. The caller received a bypass and lost a
useful resident without gaining a replacement. This occurred with both manual and
learned hot pins. The real-file baseline also confirmed that failed digest reads
already preserved the cache: that earlier read-validation behavior is not a new fix.

Admission now has three synchronous phases. A bounded plan selects all victims using
the existing heat/recency score, pin eligibility, byte budget and entry limit, without
mutating cache entries or counters. An impossible plan returns a normal bypass.
For a successful plan, the independent new byte array and cache entry are prepared
before any eviction. Only then are the selected victims removed and the prepared
entry published. Zero-byte shards still require an entry slot. Manual and hot pins
remain protected; cache budget and eviction counts reflect committed residents.

Malformed, partial, thrown and integrity-failed reader results remain rejected before
admission planning. A failed mirror may still fall back to a healthy mirror; if the
verified replacement cannot be admitted, that request bypasses without evicting.
Prefetch reports the verified shard as completed, but does not increment resident
`prefetched` for a bypass. Cancellation, active-load ownership, terminal close and
caller-byte isolation retain their existing behavior.

The plan uses temporary metadata proportional to current cache entries, and preparing
bytes before eviction can increase temporary peak memory. The configured RAM budget
still describes resident payload bytes, not total process memory or caller-held results.
The existing repeated score-selection approach is retained; no speed or memory reduction
is claimed. The plan/commit sequence relies on the synchronous trusted-session contract:
it is not an OS transaction, thread mutex, crash/OOM rollback, secure erase guarantee or
defense against direct public-handle mutation.

Thirty-six real-filesystem cases per product cover manual/hot pins, failed integrity
reads after cache hits, repeated uncached bypasses, independent caller bytes, prefetch
bypass, complete multi-victim admission, failed reread after prior eviction, and both
successful and blocked zero-byte entry-slot admission. One hundred eight additional
simulated cases per product cover seven invalid reader outcomes, healthy fallback or
exhausted mirrors, cache preservation before and after actual admission, reentry,
cancellation/requeue and terminal close. The callback fixtures own no OS descriptors.
The shared/standalone source-hidden lifecycle examples now also demonstrate that a
verified large-shard bypass preserves two actual resident shards and a later warm hit.


## Cache admission cost benchmark

The reproducible benchmark is `tests/benchmarks/pbai_cache_admission_bench.sapp`.
Run it from the product repository with a trusted `PBLANG_BIN` and private
`PBAI_STATE_ROOT`, using the sibling PBLang/pbAI source layout of the regression
fixtures. The runner copies the exact common owner and actual dedicated facades;
only the directory reader is synthetic. The child forbids network, FFI, exec and
spawn. Temporary copies are removed on success or controlled failure.

Each entrypoint (direct, standalone, embedded) runs the same three workloads:
16 four-byte resident shards plus a prior warm hit, followed by (1) one four-byte
shard that exactly fits a 68-byte / 17-entry cache, (2) a 64-byte shard blocked by
one protected resident in a 64-byte cache, or (3) a 64-byte shard requiring all
16 unprotected victims. Every batch uses 128 independent sessions, two warmup
batches and seven recorded batches. Setup, full result/accounting assertions and
close are outside the timed interval. The incoming public load, digest check,
cache operation and retained result append are inside it. Identical source hashes
bind baseline and candidate workloads; neither drops verification work.

`gc_stats` deltas count process-wide cumulative allocations, not live objects,
bytes, peak RAM or GC pause time. The string probe contributes one string allocation
per batch. `now_ms` is a wall clock; a backwards interval fails the fixture.
The recorded before/after evidence contains all samples, median and nearest-rank
p95, with baseline/candidate order reversed in a second round. These are bounded
Linux CPU measurements with tiny synthetic shards, not disk throughput or model
inference benchmarks. Concurrent machine activity and thermal state are not isolated.

An experimental capacity guard skipped the key snapshot, selection map and victim
array whenever both resident bytes and entry slots allowed immediate admission.
It retained the owned-byte copy and existing planner under pressure. All 42 measured
fits samples saved exactly two arrays, one map and sixteen strings per public load;
the two pressure workloads retained identical allocation counts. Fits median time
was lower in all six comparisons. However, pressure-path timings were inconsistent
and mostly higher, including a 49.3% median increase in one multi-victim comparison.
The experiment cannot distinguish environmental noise from a latency regression.

The candidate was therefore **not retained**. Production remains the previously
verified plan-before-commit implementation. This delivery adds the benchmark and
honest evidence, not a speedup or RAM-saving claim. The non-applied candidate patch
is retained with the evidence solely to reproduce and investigate the comparison;
applying it is a separate explicit experiment, not normal product activation.

With the two variables set, the final baseline can be rerun using:

```sh
"$PBLANG_BIN" run tests/benchmarks/pbai_cache_admission_bench.sapp --allow-all --deny-net --deny-http
```

Reproducing the discarded candidate requires a separate disposable PBLang checkout
and the non-applied patch in `docs/evidence/pbai_cache_fast_path_candidate-2026-10-04.patch`.
Keep the same workload/backend/facade hashes and runtime; rebuild and publish portable
locks with `tools/pblock.sapp` before running the same benchmark. The comparison JSON
records the exact owner hashes and both run orders. Do not apply the patch to an active
installation or interpret it as an approved product optimization.
