# Local-reference cleanup on explicit exits

The PBLang-written compiler now clears active, non-captured local references
before an explicit `throw` or `return`. This includes a throw that escapes the
compiled program without a catch handler. No manual cleanup call is required.

The exit value is evaluated once and kept on the operand stack while local
references are cleared. Returned arrays, thrown values, global aliases and
escaping closures remain valid. Cleanup uses existing nil-store instructions;
it does not invoke user equality, destructors or external operations.

## Ownership and compatibility

Try bodies and handlers execute with their caller's locals. Each generated
subchunk therefore records its first owned slot. Exit cleanup visits only its
active locals at or above that boundary. Inherited variables remain available
to subsequent handlers and outer code. Already-finished scopes are handled by
the existing scope/handler cleanup. Declarations not reached before a throw do
not receive newly materialized nil slots.

Capture discovery can occur after an earlier exit was compiled. Existing
patchable cleanup stores neutralize that earlier clearing when a later closure
captures the same slot. This preserves both the PBLang VM's value captures and
the translated native VM's open upvalues. Slots are not reused or shrunk.

The change adds no opcode, serialization version, Rust or polyglot code. It
preserves the current try-subchunk return behavior; it does not redefine return
propagation through try expressions. Original explicit error payloads remain
unchanged in the tested self-hosted and translated native executions.

## Verification

- 41 local suites passed, including scope/exception lifetime, captures,
  serialization, optimizers, bounded execution, scalar ownership, scheduler and
  five emulator suites. These are not tests inside all guest operating systems.
- Seven old/new translated-native program pairs produced identical output.
- Four subprocess checks (old/new, self-hosted/native) retained `exit-original`
  and stopped before unreachable output.
- Direct caller-owned frame inspection verifies cleanup on uncaught throw and
  explicit return, not merely final output. Tests cover inherited handler slots,
  nested failures, escaped closures, aliases, skipped declarations and exactly
  once evaluation of the return payload.
- Isolated build and publication through `tools/pblock.sapp` passed. Six lint
  and self-hosted syntax checks passed; lint reported two boolean-comparison
  style advisories in explicit failure assertions.
- Focused safety review checked inherited-slot boundaries, capture patching,
  exit-payload retention and bounded-execution behavior. No cleanup privilege,
  destructor execution or quota bypass was added.
- Graphify update exceeded 35 seconds and was terminated. No refreshed semantic
  graph coverage is claimed. The 212 pre-existing dirty files were preserved.

Tests: [exit lifetime](../tests/semantics/selfhost_exit_lifetime_test.sapp) and
[uncaught diagnostic fixture](../tests/fixtures/selfhost_exit_error.sapp).

## Measurements (2026-09-20)

Reference generator: `33f2f15`. Both versions use the same retained executable,
other compiler modules and input. The old frontend explicitly imports the old
generator; six of the seven native pairs differ in bytecode and all seven have
their output parity checked.
Two warmups followed by seven alternating measured processes per version.
GNU time records compilation plus execution: CPU is user + system, P95 is the
largest of seven observations. Resolution is 0.01 seconds.

| Workload | Version | CPU median / P95, s | Wall median / P95, s | Peak RSS median / P95, KiB |
|---|---|---:|---:|---:|
| 16 retained failed frames, 2 MiB temporary each | before | 0.15 / 0.19 | 0.15 / 0.19 | 56760 / 57100 |
| Same | after | 0.14 / 0.19 | 0.14 / 0.19 | 26132 / 26284 |
| 1,000 caught numeric failures | before | 0.32 / 0.33 | 0.33 / 0.34 | 24164 / 24376 |
| Same | after | 0.33 / 0.34 | 0.34 / 0.35 | 24140 / 24308 |
| 1,000 function returns | before | 0.26 / 0.27 | 0.27 / 0.28 | 24276 / 24380 |
| Same | after | 0.27 / 0.28 | 0.28 / 0.29 | 24316 / 24524 |

Retained failed frames used about 54% less peak RAM. The two numeric workloads
cost about 3–4% more median CPU. This is not a general speedup or zero-overhead
claim. Retained frames deliberately model a caller keeping VM frame arrays;
ordinary runs need not retain them. No disk, GPU, NPU or energy savings were
measured. Bytecode grows by at most two instructions per eligible active local
at each explicit exit, before optimizer transformations.

To reproduce, use isolated old/new compiler dependency closures, publish their
locks outside timing, and run `/usr/bin/time -f '%U %S %e %M'` with identical
workloads. Print only scalar results/output, never cyclic chunk/global objects.

1. Retained frames: compile `if true {let v=str_repeat("x",2097152);throw "original";}`
   once. In the host runner, repeat 16 times: create a fresh locals array, retain
   it in a `frames` array, call `V.exec(chunk,locals,map{},[],[])` inside a catch,
   and assert failure. Print `len(frames)` (16).
2. Numeric failures: compile and run
   `let total=0;for i 1000 {let x=try (if true {let v=i+1;throw 1;v;} else {0;}) catch 0;total=total+x;}print total;`
   (output 0).
3. Returns: compile and run
   `fn f(x){let v=x+1;return v;}let total=0;for i 1000 {total=total+f(i);}print total;`
   (output 500500).

## Remaining work

This does not intercept implicit host errors, failed payload evaluation, failed
calls that bypass a caller's explicit exit, or every exception during resource
cleanup. The current catch expression does not expose the original host error
object. A generic catch-and-rethrow replacement would lose diagnostic details,
so no such wrapper was introduced. Existing bounded execution limits still
apply to the additional instructions: exhaustion can interrupt cleanup and
take precedence over the pending exit. There is no reserved cleanup budget.

Clearing a local numeric memory handle does not call `memory_free`. General
FFI/file/socket/device cleanup needs resource-specific ownership and destructors.
Cycles still need owner-isolated identity and root tracking; retained thread-wide
GC remains disabled because suspended callers' roots cannot be proven complete.
Live cyclic objects are not traversed or cleared by this implementation.

Independent C99/WASM/ARM64 and other backends, remaining PB-MCP Rust/AI parity,
polyglot replacement and the shared desktop broker are still separate open work.
The full project is not complete. No external corporate server was used.
