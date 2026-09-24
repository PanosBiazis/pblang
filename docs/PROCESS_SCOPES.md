# Scoped child-process ownership

`stdlib:process_scope.sapp` adds `with_process(candidates, argv, options, body,
cleanup=nil)`. It uses the existing bounded process facade and does not enable
an unavailable backend. **The public Linux process facade currently reports
unavailable because its FFI contract is not fully verified.** Public calls fail
closed before executing work. Real execution below was tested only in a separate
Linux/x86-64 fixture; this is not a deployment or global availability change.

The synchronous `body(child)` returns `{ok:true,value:...}` or
`{ok:false,error:...}`. Its controller provides `poll()`, `cancel(grace_ms=250)`
and `finish(wait_ms=nil)`. It exposes no raw PID, descriptor or supervisor map.
An explicitly finished child is not closed twice; an escaped controller refuses
calls after the scope ends and releases its output/handle references.

| Developer choice | Behavior |
| --- | --- |
| Omit `cleanup`, or `{on_exit:"cancel"}` | Default: request cancellation and finish the owned child on scope exit. |
| `{on_exit:"wait"}` | After successful work, wait for normal completion. Work errors, exceptions and malformed results still request cancellation. |
| `grace_ms` | Default 250; integer 0–10000, matching the existing cancellation backend. |
| `wait_ms` | Default 1500; integer 0–10805000, matching the existing finite finish backend. Zero allows a nonblocking finish attempt. |
| Existing `process.start_fixed_bounded` API | Manual lifecycle management, including debugging; the caller must retain and finish the handle. |

These are child-cleanup choices, not a maximum lifetime or program-size limit
for PBLang applications. Existing APIs and programs retain their behavior.
Unknown policy keys and invalid values are rejected before launch.

The result separates `body_result`, `body_status`, `error`, `cleanup_ok`,
`cleanup_errors`, and `process_result`. A pending/throwing cleanup sets
`recovery_required:true` and returns `recovery_handle`, for a deliberate later
`process.close_bounded`/`finish_bounded` call. Never discard that handle while
recovery is pending. A failed start that already owns a child is also cleaned;
a failed start with no child does not call the body. Returned primary errors
survive cleanup failure. A host exception is reported as `body_status:"exception"`;
the retained runtime cannot expose its original message through this API.
`ok` describes the body and cleanup, not the child's exit code. Inspect
`process_result.command_ok` when normal command success is required; a correctly
cancelled child can have a nonzero exit code and still be fully cleaned up.

`examples/scoped_process.sapp` demonstrates explicit completion inside automatic
ownership and reports backend unavailability honestly. The implementation uses
direct calls in `try` expressions rather than conditional blocks nested inside
them; the latter lost local bindings in retained-runtime experiments. That
compiler defect is not fixed globally by this library.

## Verification and limits

- `process_scope_test`: production availability refusal, and lifecycle cases
  whenever the public backend is available.
- `process_scope_fault_test`: deterministic failures, partial starts, failed
  cleanup, zero-wait recovery, malformed results, mutable public controllers and
  no double finish. Only the process boundary is substituted.
- `process_scope_native_probe_test`: isolated Linux/x86-64 operational probe. It
  copies the facade into a disposable fixture and changes only its availability
  admission to require current descriptor and pidfd probes. Real start/cancel/
  finish operations remain unchanged. This does **not** establish a verified FFI
contract or enable any installed application. The native suite exercises
  successful return, returned error, thrown error, malformed return, wait,
explicit finish, failed start and registry release.

Validation on 2026-09-24: seven PBLang suites and four PBRemote suites passed,
plus both scoped-process and private-account examples. The two public facade
suites exercised unavailability/refusal, not production execution. Native process
and descriptor-handoff suites ran real Linux children. Eight touched PBLang
source files passed lint (with existing analyzer-style/unused-variable warnings),
and isolated builds plus native portable lock publication passed. Graphify
completed for PBRemote and timed out after 35 seconds for PBLang; it does not
provide semantic `.sapp` coverage.

The scope is for synchronous trusted callbacks. It does not preempt a hung
callback, survive process death or power loss, provide thread-safe shared
controllers, remove arbitrary orphan processes, or implement global runtime/FFI
collection. No installed PB-MCP process was restarted or reconfigured. No general
CPU/RAM or Live View speedup is claimed.

The underlying [Linux pidfd lifecycle](PIDFD_LIFETIME.md) now rejects invalid
one-shot signals before resource acquisition and retires descriptors before
foreign cleanup, including library-unload failure. This correction does not
change the public backend's admission status.

A read-only laptop observation found no zombies in repeated `/proc` snapshots.
Five existing PBLang processes belonged to three MCP connections and the desktop
operator/worker. Over one ten-second sample they recorded no storage reads or
writes, stable descriptor counts, and 0%, 2.2%, and 0.3% CPU respectively (percent
of one core; the three MCP workers were idle). This short observation cannot
exclude intermittent leaks or prove whole-system health. No existing process was
killed. The corporate Windows computer and EnterpriseOrg servers were not accessed.
