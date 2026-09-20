# Automatic local-reference cleanup

The PBLang-written compiler now emits automatic cleanup when an ordinary lexical
block ends. Programs do not import a memory helper or call `free`, `pause` or
`resume` to get this behavior. This extends beyond scheduler callbacks to local
variables holding strings, arrays, maps and other ordinary runtime values.

## What changed

Previously `codegen_end_scope` removed name-resolution records but left their
values in the frame's local slots. A long-lived frame with successive blocks
could retain every temporary array/string until its eventual return.

The compiler now stores `nil` in non-captured local slots on normal scope exit
and before a `break`/`continue` leaves their scope. Count/range/collection loops
and match subjects register their private temporaries without introducing a
source-visible name. Match arms clean both successful and failed-guard paths.
The result of a value block remains on the operand stack while the dead local
reference is removed. Aliases and module globals retain their values.

Only existing CONSTANT, SET_LOCAL and POP instructions are used. No new Rust,
polyglot block, permission, background service or bytecode opcode was added.
The self-hosted VM, optimizer, serialized self-hosted bytecode and translated
native bytecode use the existing instruction contracts.

## Captures and compatibility

Captured slots are conservatively excluded. The PBLang VM captures values,
whereas native translation can hold open references to frame slots. Clearing a
captured slot would therefore be unsafe across these routes.

The compiler records capture information across shared try/catch subchunks.
When a later closure declaration captures a slot for which an earlier
break/continue cleanup was already emitted, that store is neutralized to POP.
The preceding nil push and all branch offsets remain valid. This is needed when
a closure created in an earlier loop iteration survives a later continue path.

Slots retain their original unique indices. This is reference cleanup, not slot
reuse or object mutation. The debugger-facing `locals` array now contains nil
for cleaned-up slots; this intentional visibility change follows lexical scope.
No deep-size traversal, forced collection or destructor callback is added.

## Evidence and measurements

39 local suites passed, including the focused lifetime suite, closures, all
five loop/match/try scope suites, serialization, optimizer, scheduler and five
emulator-related suites. Thirteen additional programs were compiled by the old
and new compilers, translated and executed on the same retained native VM; each
pair produced identical output. Existing cross-runtime capture differences are
not claimed to have been resolved.

Linux measurements used the same retained executable and source, an isolated
portable-locked import closure, two warmups and seven alternating measured
process runs per version. GNU time measured total user+system CPU, wall time and
peak RSS for compilation plus self-hosted execution. P95 is nearest-rank (the
maximum of seven samples); CPU resolution is 0.01 s. No benchmark ran alongside
other test suites or Graphify.

| Workload | Version | CPU median / P95, seconds | Wall median / P95, seconds | RSS median / P95, KiB |
|---|---|---:|---:|---:|
| 16 blocks, 2 MiB temporary each | before | 0.15 / 0.16 | 0.16 / 0.17 | 56808 / 57044 |
| 16 blocks, 2 MiB temporary each | after | 0.14 / 0.15 | 0.15 / 0.16 | 26032 / 26340 |
| 5,000 numeric loop iterations | before | 0.23 / 0.23 | 0.23 / 0.24 | 24316 / 24364 |
| 5,000 numeric loop iterations | after | 0.24 / 0.28 | 0.25 / 0.29 | 24216 / 24420 |

The block workload reduced peak RSS about 54%. The small-loop CPU median rose
about 4%, with higher tail variability. This is not a universal performance
improvement or a claim of zero overhead. Additional instructions and bookkeeping
also increase compilation work and bytecode size. No GPU, NPU, disk-I/O or
battery-energy improvement is established.

To reproduce, compare the code generator from `c0a85b2` with the new one while
keeping the remaining compiler modules and executable identical. Build and
publish the portable lock outside timing. Compile and run these source strings
through `frontend_compile` and `vm.run`; print only `result.output`, never the
full cyclic VM globals or chunk objects:

- Blocks: `let total=0;`, then repeat this source block **16 times**:
  `if true {let v=str_repeat("x",2097152);total=total+len(v);}`, followed by
  `print total;`. Expected output is 33554432.
- Loop: `let total=0;for i 5000 {let v=i+1;total=total+v;} print total;`.
  Expected output is 12502500.

Example: [automatic_lifetime.sapp](../examples/automatic_lifetime.sapp).

## Remaining lifetime work

This is not full automatic memory management for every PBLang runtime:

- Caught try-body failures now clean temporary references before the handler;
  see [exception cleanup](EXCEPTION_LOCAL_LIFETIME.md). Explicit throws and
  returns now also clear their own active non-captured references; see
  [explicit exits](EXPLICIT_EXIT_LIFETIME.md). Implicit uncaught failures and
  general external-resource cleanup preserving the original error still need
  further runtime work.
- Captured slots remain until their existing frame/closure lifetime ends. Cyclic
  graphs and owner-isolated heap collection are not implemented by this change.
- Explicit `memory_alloc` handles, FFI buffers, files, sockets, device allocations
  and resource destructors are outside ordinary local-reference cleanup. Replacing
  a numeric owner handle with nil does not invoke `memory_free`.
  [Resource scopes](RESOURCE_SCOPES.md) now offer automatic cleanup of explicitly
  registered callbacks, including after catchable implicit exceptions; general
  compiler-inserted resource ownership is still unfinished.
- Values are cleaned at scope exit, not at a proven last read within a live
  scope. No arbitrary time-based deletion or suspension of live objects occurs.
- Local-slot storage is not shrunk. Additional stores can materialize nil slots
  for zero-iteration loops, and additional instructions consume existing bounded
  VM step/local quotas. Quotas are not raised or bypassed.
- This does not modify the retained Rust source compiler or independently
  implement C99, WASM, ARM64 and other backends. Native evidence concerns the
  existing self-hosted-bytecode translation route, not universal platform support.

Code generator SHA-256: `c666a1facd9313943ce87523274c6be9feffbe4b9a3ef2fcc4f7b186bd27b59e`.
Retained executable SHA-256: `7f1b97c0037736553ab39d165c13c36f3fd8497bc6aca9ca94a4c680858e6500`.

The isolated build, native portable-lock publication and six lint/self-hosted
syntax checks passed. Lint reports one advisory constant-condition warning in
the deliberately scoped example. Graphify update exceeded 35 seconds and was
terminated; semantic graph coverage is not claimed. The 212 unrelated local
files were preserved byte-for-byte. No EnterpriseOrg server was used.
