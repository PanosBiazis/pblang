# Cooperative idle timers

`stdlib:scheduler.sapp` puts the **scheduler's loop** to sleep until the next
active deadline or the selected polling interval. It no longer polls at 100 Hz
when no work is due. This is PBLang source using the existing host clock/sleep
boundary; it is not a new OS runtime or a general-purpose garbage collector.

**Automatic by default:** existing `set_timeout`/`set_interval` and `run_loop`
callers gain deadline-driven waiting and retirement of completed/cancelled
callbacks without adding `pause_timer` or `resume_timer`. Those two operations
are optional application controls, not requirements for the optimization.

Functions already execute only when called. A paused timer explicitly stops
scheduled calls while preserving its callback and captured values. Arbitrary
variables, open files, GPU buffers, and library background threads are not
suspended or discarded by this feature.

## API

Existing registration, cancellation, dispatch and stop functions remain available.
Timer IDs are numeric, unique for this module instance, and never reused after
cancellation or `clear_all`. Invalid IDs are harmless; finite fractional and
negative delays preserve their existing immediate/delayed interpretation.
Non-finite/non-numeric delays are rejected. ID exhaustion is checked at the
host's exact integer representation boundary, not an application size quota.

| Operation | Behavior |
|---|---|
| `pause_timer(id)` | Returns true once; preserves the remaining delay and callback. |
| `resume_timer(id)` | Returns true once; appends to active order and restores remaining delay. |
| `is_active(id)` | True for an existing, unpaused timer. |
| `is_paused(id)` | True for a paused timer. |
| `active_timers()` / `paused_timers()` | Counts without scanning historical records. |
| `retained_timers()` | Number of live or paused records; excludes cancellation/completion. |
| `clear_timer(id)` | Removes the record and drops its callback reference. |
| `clear_all()` | Cancels and drops callbacks for both active and paused work. |
| `next_delay(max_wait_ms=1000)` | Milliseconds until active work, capped by a finite positive wait. |
| `run_loop(duration_ms=0, idle_poll_ms=1000)` | Cooperative loop; positive duration caps its wait; nonpositive duration runs until stopped. |

Example: [scheduler_idle_demo.sapp](../examples/scheduler_idle_demo.sapp).
A GUI should normally call `run_pending()` from its own event loop, using
`next_delay()` to schedule its next wakeup, rather than block its UI thread.

## Lifecycle and ordering

Active work runs in registration/resumption order, at most once per pending pass.
New work and resumed work wait for the next pass. Cancellation from an earlier
callback prevents later cancelled callbacks from running. Pause does not produce
a burst of catch-up calls. A repeating callback that pauses itself preserves a
full interval; a completed one-shot is retired even if it pauses itself.

Recursive `run_pending()` returns zero; nested `run_loop()` is rejected. Ordinary
callback exceptions remain contained, with repeating callbacks kept scheduled as
before. Stop prevents another loop sleep; it does not cancel remaining timers or
interrupt a currently executing callback. `clear_all()` is separate from Stop.

Queue entries are allocated at registration/resumption, not on every dispatch.
Stale entries are compacted when they outnumber active timers, with an empty
queue when no active work remains. Cancelled entries have their callback removed
immediately, even if a currently executing pass still holds the entry. No
unbounded timer history is kept. Paused callbacks deliberately retain their data;
this is necessary for continuation.

## Limits and compatibility

- Cooperative, single-threaded execution; no preemption, parallel workers or
  thread-safe mutations. A long-running callback can still monopolize CPU.
- Host `clock()` is wall time, not monotonic. Clock corrections can change timing.
- Default idle polling is at most one second; the parameter is explicit and
  caller-selectable. `sleep_ms` rounds waits upward to a millisecond. Ready
  zero/negative intervals retain approximately 10 ms pacing in `run_loop`.
- Host sleeping still requires its existing permission. Sleep failure resets
  loop state and reports `scheduler: host sleep failed`; this does not preserve
  the original host error detail. No new capability is granted.
- The tested self-hosted frontend supports pause/resume and manual pending passes,
  but does **not** provide `sleep_ms`. A loop requiring sleep fails clearly.
- Dropping scheduler references does not prove immediate RSS release or general
  cycle collection. The retained runtime's wider memory limitations remain.
- Tested locally on Linux; this change is not evidence of Windows/macOS/mobile
  guest execution, energy savings, GPU/NPU savings or universal compatibility.

## Local measurements (2026-09-20)

Reference: `eac90fd` scheduler. Both versions used the same retained executable,
identical workloads, two warmups and seven measured process runs per workload
and version, interleaved in alternating order. GNU time collected user+system
CPU, elapsed time, peak RSS and voluntary context switches. P95 below uses the
nearest-rank definition (the largest of seven samples). CPU resolution is 10 ms;
zero does not mean no CPU use. Normal desktop activity adds noise.

| Workload | Version | CPU median / P95 (s) | Wall median / P95 (s) | RSS median / P95 (KiB) | Voluntary switches median / P95 |
|---|---|---:|---:|---:|---:|
| idle | before | 0 / 0 | 1 / 1.01 | 9620 / 9700 | 101 / 101 |
| idle | after | 0 / 0 | 1 / 1 | 9828 / 9856 | 3 / 3 |
| churn | before | 0.38 / 0.39 | 0.39 / 0.4 | 19912 / 20068 | 2 / 2 |
| churn | after | 0.06 / 0.06 | 0.07 / 0.07 | 11676 / 11872 | 2 / 2 |
| active | before | 0.07 / 0.11 | 0.07 / 0.12 | 10240 / 10520 | 2 / 2 |
| active | after | 0.1 / 0.14 | 0.1 / 0.15 | 11884 / 12228 | 2 / 2 |

- Idle: one-second loop with no timers. Fewer voluntary switches indicate less
  polling, not a measurement of CPU wakeups, battery energy or GPU activity.
- Churn: create/cancel 12,000 immediate callbacks, then perform 100 pending
  passes. CPU median decreased about 84%; RSS median decreased about 41%.
- Active: 1,000 zero-interval callbacks over 100 pending passes. CPU median
  increased from 0.07 to 0.10 s (about 43%) and RSS from 10,240 to 11,884 KiB
  (about 16%). This active-work overhead remains; this is **not** a universal
  speed/memory improvement. Registration validation and lifecycle tracking have
  costs. No GPU/NPU/storage-I/O or power savings have been established.

Reproduce the three bodies in `tests/benchmarks/scheduler_{idle,churn,active}.sapp`
against separate old/new module copies, with the same executable and isolated
project/import lock. Build and publish the portable lock before measuring, not
inside the measurement. Run two warmups and seven alternating measured runs
using `/usr/bin/time -f '%U %S %e %M %w %c'`. Do not run other test suites or
Graphify concurrently with benchmarks. The measured harness imported sibling
module copies so both versions used identical import handling.

Measured scheduler source SHA-256: `680b5e8492d9b0aab3b251642fb30980f5d53d0128de809742d5e83bf46e54d7`.
Retained executable SHA-256: `7f1b97c0037736553ab39d165c13c36f3fd8497bc6aca9ca94a4c680858e6500`.

## Verification and remaining work

Twenty-nine local suites passed, including five scheduler suites and five
emulator-related suites. Scheduler coverage includes 800 deterministic
model-based state transitions, registration order, pause/resume, invalid and
non-finite inputs, callback failures, nested dispatch, one-shot cleanup,
clear-all during a callback, idle deadlines and Stop. The example completes
without manual pause/resume. A focused security review checked capabilities,
input validation, cancellation and callback retention; it is not a security
certification.

Automatic last-use reclamation for general variables/closures, lazy module
initialization with side-effect compatibility, worker/FFI suspension and
owner-isolated cycle cleanup are separate compiler/runtime tasks. This change
does not implement them. Persistent references must not be deleted merely
because they have not been read recently.

All nine added/changed PBLang sources passed self-hosted syntax checks and lint
returned success. Lint still reports six advisory warnings: unused loop indices,
a collapsible conditional, and a false unused-variable report for the checked
`slept` result. The isolated build and portable lock publication passed. The
Graphify update exceeded its 35-second bound and was terminated; no graph
coverage is claimed. All 212 unrelated dirty files were preserved byte-for-byte.
