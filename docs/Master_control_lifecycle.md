# Master-control request lifetime

An approved request authorizes **at most one executor invocation**. The runtime
removes its pending action and approval before the policy recheck and before
calling the executor. A recursive call with the same ID is rejected. An executor
exception continues to propagate, but its approval cannot be retried: the
executor may already have performed part of its action. A caller that wants to
try again must create a new request and obtain a new local approval.

`lock()` and `kill()` clear pending and approved requests. Unlocking after a
lock, or resetting and unlocking after a kill, never restores old requests.
Observing an expired session also clears these queues. Renewing a still-active
session preserves its outstanding requests. Request IDs continue increasing;
they are not reset by revocation. Completed requests are removed rather than
retained as nil-valued map entries, so `status().pending` counts live requests.

```pblang
let control = import("stdlib:master_control.sapp");
// The trusted local operator performs unlock and approval separately.
let pending = control.request(map{ kind: "screen" });
// After local approval, the host may invoke its own fixed executor once.
// If that invocation throws, request a new approval before any retry.
control.lock(); // Also discards pending, unexecuted requests.
```

The change fixes reproduced cases where an executor that raised an error could
run again under the same approval, and where lock/unlock retained an old approval.
It also prevents synchronous re-entry while the first executor is running.
The baseline already rejected ordinary replay after successful execution through
its policy check on a nil action; this change makes consumption explicit and
removes the retained map entries.

## Scope and limits

This is a PBLang library change used by the input-control/PBRemote path. It does
not expose desktop tools in the installed seven-tool PB-MCP offline profile.
PBRemote control leases remain a separate, locally approved mechanism with their
own owner, expiry and event quotas. Screen-sharing revocation is also separate
from master-control lock/kill; an integrated desktop Stop must revoke both.

At-most-once invocation does not mean atomic or reversible OS actions. It does
not cancel an executor already running, provide cross-thread atomicity, or
establish OS sandboxing. Command-pattern checks do not prove arbitrary keyboard
input safe. Caller-owned mutable action maps/executor binding and the existing
audit history require further review before broad desktop exposure. The audit
history is not bounded by this change; no general memory bound is claimed.

## Verification

`tests/semantics/master_control_lifecycle_test.sapp` launches its fixture with a
private test-owned nonce directory and an empty inherited environment. Child
execution, network, FFI, graphics and spawning are denied. Its executors only
increment counters; no screen is captured and no keyboard/mouse event is sent.
It checks success, failure propagation, replay, re-entry, lock, kill/reset,
policy recheck, unknown IDs, queue cleanup and absence of fixture credentials
in lifecycle audit records. Session-expiry cleanup was reviewed but is not
covered by a wall-clock-expiry integration test in this suite.

The existing master-control policy, input-control and input-event suites are
also run, with PBRemote integration validation recorded separately. These are
focused checks, not a complete security audit or universal platform validation.

Measurement: 2,000 harmless request/approve/execute cycles per fresh process,
identical retained runtime, two warmups and five samples per variant in
alternating order. Includes startup and the existing local audit writes.

| Metric | Before median / p95 | After median / p95 |
| --- | --- | --- |
| Elapsed seconds | 0.07 / 0.07 | 0.07 / 0.08 |
| User CPU seconds | 0.05 / 0.06 | 0.05 / 0.06 |
| System CPU seconds | 0.01 / 0.02 | 0.01 / 0.02 |
| Peak process RSS, KiB | 17048 / 17124 | 16412 / 16524 |

The workload is short relative to timing resolution. Five samples are not a
reliable tail estimate. The lower observed RSS is specific to this workload;
no general speedup or memory saving is claimed.
