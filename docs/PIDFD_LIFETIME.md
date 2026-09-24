# Linux process identity lifetime

`stdlib:linux_pidfd.sapp` owns two separate resources: a Linux process descriptor
and an FFI library handle. This is a trusted-code native boundary, not an
untrusted-code sandbox or a replacement for the public process admission gate.

## Corrections

`signal_once(pid, signal)` now validates both inputs before opening either
resource. Previously an unsupported signal threw inside `send`, after `open`
had already acquired the resources. A live regression with 32 rejected `USR1`
requests grew `/proc/self/fd` from 4 entries to 36 on the previous module; the
same workload remained at 4 with the corrected module. No nonzero signal was
sent by that regression.

`close(handle)` now retires the descriptor before calling the foreign boundary.
The existing `closed` flag means **retired for further use**, not proof that
every resource was successfully released. Repeated close returns false;
`exists` returns false and `send` rejects a retired identity. A successful
`close` return confirms both operations. The old numeric `libc` token remains
for compatibility; it does not retain the released library. No per-handle
diagnostic fields are added to the normal path.

An error from the descriptor close returns false after attempting library
release. A library-unload exception still propagates, but the descriptor remains
retired and its number is cleared. The library token remains in `libc` for
diagnosis; the descriptor-close outcome is not separately exposed when library
unloading throws. This module does not automatically retry
an ambiguous native close: a descriptor number may already belong to another
resource. It also cannot guarantee release when the native call was rejected
before execution. Such failures must not be reported as successful cleanup.

A deterministic old-module regression demonstrated two descriptor-close
attempts and a still-open public handle after a library-unload exception. The
new tests verify a single attempt, including reentry during the foreign call.
These are synchronous lifecycle protections, not cross-thread locking or
protection against a trusted caller deliberately rewriting the raw handle map.

## Developer choices

- Prefer [scoped process ownership](PROCESS_SCOPES.md) for whole child processes:
  its default is cancel-and-finish; explicit wait and manual lifecycle choices
  remain available. Its public backend is still unavailable pending FFI work.
- Use `signal_once` for an explicitly intended single signal to a known process.
  Input validation now precedes acquisition automatically.
- Use `open` / `exists` / `send` / `close` for deliberate manual ownership.
  `examples/pidfd_identity.sapp` demonstrates a read-only self-identity probe and
  reports only booleans, without exposing machine identities.

## Verification and remaining boundary

- `linux_pidfd_lifetime_test`: 160 rejected requests, normal open/close loops,
  unchanged live descriptor counts, no nonzero signals.
- `linux_pidfd_lifetime_fault_test`: actual module with only FFI calls replaced
  by a non-executing fixture; validation before acquisition, success, native
  error/exception, library-unload exception, repeated close and reentry.
- Existing `linux_pidfd_test`, native process and scoped-process suites exercise
  real owned children, group cancellation and cleanup in isolated Linux tests.

Final validation: all 11 targeted suites passed (the two new lifetime suites,
`linux_pidfd_test`, three process-scope suites, `process_bounded_async_test`,
`linux_process_test`, `linux_process_verified_fixed_test`,
`dist_runtime_resolution_test`, and `ffi_tuple_contract_test`). The public
facade tests exercised refusal; real execution used the explicitly isolated
native probes. Both examples, six source-file lint checks and the isolated
build plus native portable-lock publication passed. Lint reported unused loop
indices in two workload files. An initial runtime-resolution fixture failure
was caused by its placeholder manifest version; it passed with the canonical
package metadata. Graphify update exceeded its 35-second bound and was stopped;
no semantic PBLang graph coverage is claimed.

The live tests establish Linux behavior on the tested host only. The retained
FFI still accepts caller-supplied ABI signatures and does not enforce a complete
library/symbol/signature tuple. Native ABI marshalling and immutable library
loading remain unresolved; see [FFI contract](FFI_contract.md). Neither a smoke
probe nor these tests enables the public process facade. No installed PB-MCP
configuration or executable was changed.

The four active installed PB-MCP package roots were inspected: their 48 PBLang
source files contained no `linux_pidfd` module or reference. This is a targeted
dependency check, not a full audit of their transitive runtime. No installed
application was restarted, and no Live View benefit was measured.

Descriptor leaks are not zombie processes: zombies are exited children awaiting
reaping. A read-only host snapshot found no zombies during this work; that does
not exclude intermittent problems. No unrelated application was terminated.

## Cost measurement (2026-09-24)

`bench/pidfd_lifetime.sapp` opens and closes its own process identity 100,000
times. The baseline was the module from `aebf26c`; both modules were copied to
equivalent fixture-local import paths, using the same retained runtime. One
warmup per variant preceded nine alternating samples each. `/usr/bin/time`
measured user plus system CPU and peak RSS; p95 is the nearest-rank statistic
(the maximum with nine samples). Both variants released the same number of
identities. Earlier trials with mismatched import styles are excluded.

| Metric | Baseline | Corrected |
| --- | ---: | ---: |
| Median CPU | 1.51 s | 1.64 s |
| p95 CPU | 1.58 s | 1.95 s |
| Median peak RSS | 57,800 KiB | 58,564 KiB |

CPU samples, in each variant's execution order:

- Baseline: 0.96, 0.94, 1.53, 1.54, 1.49, 1.50, 1.58, 1.51, 1.51 seconds.
- Corrected: 1.10, 1.16, 1.65, 1.64, 1.65, 1.85, 1.95, 1.59, 1.62 seconds.

The measured median cost was +8.6% CPU and +1.3% peak RSS in this synthetic
workload, with substantial timing variation and worse p95. This is a correctness
fix, not a speedup or general memory optimization. It prevents the demonstrated
resource leak on rejected requests; it does not resolve previous performance
regressions, GC pauses/cycles, Windows lock publication, general FFI/polyglot
isolation, GPU/NPU execution or broad platform coverage.
