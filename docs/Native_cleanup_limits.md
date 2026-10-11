# Native VM cleanup, GC, and instruction limits

This contract applies to the retained native VM with the `cleanup_roots_and_limits`
regression suite. A `0.6.0` version string alone does not identify this behavior;
record the actual executable hash/revision. It does not establish PBCK/frontend
support for `defer` or upgrade product-bundled runtimes.

## Traced roots

Pending unwind closures, registered frame defers, and saved return values are GC
roots, alongside the operand stack, active closures, modules, and upvalues. A
cleanup action may call `gc_collect()` without destroying values needed by a later
cleanup action. This is reachability correctness, not a general memory-use or
physical-RAM reclamation guarantee.

## Terminal instruction exhaustion

When `VmLimits.max_instructions` is configured, exhaustion ends normal execution.
The VM closes captures and schedules already registered cleanup in inner-to-outer,
LIFO order. It bypasses existing catch handlers, so catching the budget failure
cannot restart the computation.

Cleanup shares a single allowance of **10,000 additional bytecode instructions
per run**. Nested calls, nested exceptions, and cleanup registered during this
allowance use the same counter. The allowance is never renewed. Cleanup can catch
its own ordinary errors, but cannot catch exhaustion of this allowance.

If the main budget expires while an exception is already unwinding, that original
exception remains the reported failure. Otherwise the reported failure is
`instruction limit exceeded`. A cleanup error cannot replace the primary failure.
An active cleanup interrupted at the main budget boundary is not retried; its
registered defers and remaining older cleanup actions are considered next.

If the additional allowance expires, the VM abandons the remaining actions and
returns the primary error. Therefore this contract guarantees a bounded cleanup
attempt, **not successful execution of every defer under every failure**. Never
rely on an infinite, blocking, or failing closer to release a resource successfully.

These limits bound bytecode execution. They do not interrupt native/FFI calls,
force a blocking system API to return, guarantee wall-clock deadlines, or handle
process termination. With no instruction limit configured, this change does not
introduce a time or instruction limit. Spawn/import/eval may use separate VMs;
this is a per-run limit, not a global multi-VM resource budget.

## Regression checks

`cargo test -p pblang-vm --test cleanup_roots_and_limits` covers list/map captures,
nested cleanup, saved return values, terminal budget failures, original-error
preservation, finite cleanup with nested caught errors, and VM reuse after abort.
