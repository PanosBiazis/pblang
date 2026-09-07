# PBLang host-PBCK runtime: cleanup and admission

This contract applies to `stdlib/host_pbck_vm.sapp`, including the
`host_pbck_frame_vm.sapp` bridge that delegates execution to it. It does not
describe the retained Rust VM or every other PBLang execution backend.

## Guest exceptions and cleanup

Explicit guest `throw`, failed result unwrap (`?`), and propagated guest call
exceptions run the exiting frame's defers in LIFO order. A same-frame handler
does not exit the frame or run its defers early. If a deferred guest function
throws, its own defers execute; its exception is then ignored so older defers
can run and the original return value or exception is preserved. Injected
adapter exceptions that this VM converts to guest exceptions follow this rule.

This implements the guest-exception part of the frozen defer contract. It does
**not** catch direct engine faults: malformed/non-callable values and exhausted
step/allocation/frame budgets stop execution. Direct engine faults in a function
body still bypass guest-exception unwind. Therefore universal cleanup after any
runtime failure, process exit, or host callback failure is not established.

## Frame and parameter admission

| Limit | Contract |
| --- | --- |
| `max_frames` | Maximum simultaneously live guest frames, including the root. A root-only program fits 1; an ordinary child call needs 2. Returned frames release their admission. A deferred guest function also needs an additional live frame. |
| Parameter `max_stack` | Fixed parameter cells plus one cell for a varargs array must fit before cells are charged or constructed. Defaults occupy their normal cells; even empty varargs occupy one cell. |
| Parameter `max_locals` | The same parameter-cell shape is independently checked against this limit before materialization. This is not a claim that every other allocation path is now preflighted. |
| `max_steps` / allocation units | Existing cumulative counters also apply during guest cleanup. They are not reset for a handler, child call, or tail reuse. Exhaustion remains fatal. |

Tail calls reuse the current admission only when the frame has neither live
defers nor live exception handlers. Otherwise the VM uses an ordinary call to
preserve cleanup and catch behavior. Handler-free and defer-free tail calls
continue to work within a one-frame budget.

Compatibility note (2026-09-07): the old implementation accidentally admitted
N+1 live frames for `max_frames=N`. That budget escape is intentionally removed.
Callers genuinely requiring two live frames must request at least 2. No opcode,
bytecode format, source syntax, or public function signature changed.

## Evidence and remaining limits

Focused tests: `host_pbck_defer_unwind`, `host_pbck_frame_admission`,
`host_pbck_vm_full`, `host_pbck_vm`, and `host_pbck_frame_vm`. They exercise
synthetic in-memory programs and explicitly injected recording callbacks,
including exact-N and N-1 boundaries and no callback effects after denied
admission. Source ordering establishes pre-materialization admission; these
tests are not native allocator telemetry or physical-device qualification.

Deterministic guest accounting is not VM-wide hard heap enforcement, wall-clock
preemption, or hostile-code isolation. An injected host callback cannot be
preempted merely by the guest instruction counter. The broader 1.0.0 readiness
decision remains in the comparative audit report.
