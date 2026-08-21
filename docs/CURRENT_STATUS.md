# pblang — Current Status

**Last Updated**: 2026-08-13
**Status**: v0.5.7 — Production-Candidate Core with AI Integration, Security Hardening + Concurrency + Memory Safety + 369 top-level pblang stdlib files + **Runnable self-hosted compiler pipeline (tokenize→parse→check→compile→RUN in pblang) with active parity hardening** + Multi-Backend Codegen + blocks/code-blocks programming + machine/device optimization planning + resource-aware test profiles + cross-device play/run planning + cross-language convenience helpers

`stdlib:speech_catalog.sapp` adds deterministic bounded discovery of installed
Whisper model artifacts and eSpeak voice data. It reads only fixed system roots
through held directory descriptors, distinguishes callable engines from
data-only installations, caps results at 512 entries, and exposes neither
absolute paths nor model/voice file contents. It is inventory infrastructure,
not a speech recognition or synthesis engine.
The latest complete pblang semantic run on 2026-08-02 passed **748/748 tests**
with zero failures, ignored tests, or filtered tests. The separate release Rust
semantics harness passed 203 tests with zero failures and one documented
security ignore; these runner counts are not combined.
Product suites also passed: pbAI 23/23 (the two training-heavy tests required
more than 90 seconds), pbOS 17/17, and pb-mcp-server 11/11. The Rust
`cargo test --test semantics` harness keeps exactly **one** ignored test —
`os_env_set_pass` (`env_set` was disabled in the v0.5 security audit;
`std::env::set_var` is unsound in multi-threaded programs).

## 2026-08-13 - Native bounded Linux serial I/O

`stdlib:linux_serial.sapp` provides a PBLang-owned Linux serial backend without
new Rust or serial subprocesses. It accepts only explicit `/dev` serial-device
families, resolves devices relative to a held directory descriptor with
`O_NOFOLLOW`, verifies the opened descriptor with `isatty`, supports an exact
bounded baud-rate set, uses raw nonblocking termios plus poll deadlines, and
caps reads and writes at 65,536 bytes. Closing restores the original termios
state and reports the real restoration result.

The semantic contract performs exact binary round trips in both directions on
a test-only PTY, checks timeout and restoration behavior, and passed. The MCP
consumer exposes four independently capability-gated serial aliases; their
isolated focused release gate also passed. Windows and macOS remain honestly
unsupported until equivalent native handle-based confinement is implemented.
No physical serial hardware was accessed by automated tests.

## 2026-08-13 - Native bounded MQTT 3.1.1

`stdlib:mqtt311.sapp` adds a PBLang-native binary MQTT 3.1.1 codec and client
over `stdlib:native_tcp.sapp`. It validates canonical packet lengths and topic
grammar, bounds packets/payloads/messages/deadlines, and implements complete
QoS 0/1/2 publish and receive acknowledgement state machines. Semantic tests
exercise real loopback broker dialogues for publisher and subscriber paths,
including QoS 2. The pb-mcp-server consumer adds DNS pinning and intentionally
restricts plaintext MQTT to explicitly approved private trusted networks or
authenticated tunnels. This is not an MQTT-over-TLS implementation.

## 2026-08-13 - Bounded available-byte TCP reads

`stdlib:native_tcp.sapp` now exposes `read_available_bytes()` for bounded
passive protocol/banner reads. It returns already received bytes after an idle
window without requiring EOF, while retaining total deadline, byte ceiling,
nonblocking and pending-buffer semantics. The loopback contract verifies a
banner returned while the peer remains connected. pb-mcp-server uses this
primitive for its single-host quick service scanner; the stdlib primitive does
not itself perform network discovery.

## 2026-08-02 - Canonical exact-i32 call-frame lowering

`stdlib:host_pbck_frame_lowering.sapp` converts a frozen verified call graph
into one deterministic dense image made only of bounded scalar fields and
primitive arrays. Function IDs, constants, direct function references,
defaults, captures and entry arguments are remapped once. The builder
re-executes the source verifier and compares its value, reachable graph and
resource observations before publishing a checksum-bound frozen image.

The independent validator rejects fractional table values, duplicate function
IDs, non-canonical operands or capture ranges, invalid arity, references or
signed-i32 payloads, quota inconsistencies and checksum tampering. Tests cover
three active frames with a shared local-to-nonlocal cell, direct functions,
defaults, arguments and negative mutations. The runnable example is
`examples/host_pbck_frame_lowering/main.sapp`; the benchmark is
`bench/host_pbck_frame_lowering_stages.sapp`, with results in
`bench/results/2026-08-02-host-pbck-frame-lowering/REPORT.md`. Median cost was
0.466 ms/image to reverify and build and 0.179 ms/image to validate for the
three-function, ten-instruction fixture. This is a backend input contract, not
an executor by itself. The complete semantic suite passed 748/748. No foreign
source was added.

## 2026-08-02 - Bounded call-graph selection

`host_pbck_frame_calls.find_i32_call_probe()` now accepts positive bounded
`min_reachable_functions` and `max_reachable_functions` options. This lets a
platform lowering request an exact graph-size class without relying on the
first smaller compatible candidate. Inverted limits and values outside
`1..4096` fail closed. A nested three-function/non-local-capture regression
selects `[0, 1, 2]` deterministically and returns `73`; the complete semantic
suite passed 747/747. No Rust source was added.

## 2026-08-02 - Verified portable i32 call graphs

The existing generic flat call-frame engine now exposes
`verify_i32_prepared()` and `verify_i32()` as platform-lowering gates. They run
the selected reachable graph under caller-supplied step/frame/stack/local and
closure quotas, then validate all reachable constants, defaults, local and
upvalue operands, closure targets and captures, call arities and jump targets.
The frozen result records the exact reachable functions/instructions/opcodes,
capture requirements and observed resource peaks for a backend to consume.

Upvalue requirements are derived from the actual instructions and non-local
captures instead of trusting legacy `max_upvalue` metadata as a count. Tests
cover shared mutation with ordinary plus tail calls, deterministic revalidation,
a nested three-function non-local capture and malformed/fractional rejection.
The runnable example is `examples/host_pbck_frame_calls/main.sapp`; the staged
benchmark is `bench/host_pbck_frame_calls_stages.sapp`. Verification of the
15-instruction example measured 0.195 ms. Warm execution remained 0.122 ms/run
for the flat engine versus 0.099 ms/run for the generic VM, so no speed claim is
made for this slice. The complete semantic suite passed 747/747. No Rust source
was added.

## 2026-08-02 - Complete PBFP execution bridge

`stdlib:host_pbck_frame_vm.sapp` now turns a validated `PBFP-v1` frame plan back
into the canonical recursive PBCK program once and delegates execution to the
single complete `host_pbck_vm.sapp` implementation. This provides semantic
execution for all 48 opcodes, nested functions, defaults, captures, exceptions,
dynamic values and explicitly injected host adapters without maintaining a
second opcode engine. Prepared programs are frozen and retain VM step, frame,
stack, local, output and collection limits.

The bridge rejects malformed plans, configured resource excess, cyclic function
graphs, closure references outside a function's constant table, foreign value
slots and invalid prepared objects. Per-function slot and closure indexes make
preparation O(constants + instructions), which is required for large generated
applications. Reconstructed functions retain compact opcode/operand arrays and
allocate capture maps only for closure instructions, avoiding one map allocation
per instruction. Its semantic test checks deterministic
PBFP reconstruction across every canonical opcode and adapter error redaction.
The runnable example is `examples/host_pbck_frame_vm.sapp`; the reproducible
staged benchmark is `bench/host_pbck_frame_vm_stages.sapp`, with results in
`bench/results/2026-08-02-host-pbck-frame-vm/REPORT.md`. The prepared bridge
measured 0.144235 ms/run versus 0.125993 ms/run for direct VM execution on the
four-step fixture. This completes the
shared PBLang execution oracle for flat Android assets. Generated Android code
still requires equivalent dynamic-value lowering and Android capability
bindings before whole-program device execution can be claimed. No Rust source
was added.

---

Canonical status is the top summary and `Test Results` section in this file. Older milestone snapshots below are kept for historical context and may reflect prior counts/plans.

## 2026-08-02 - Bounded flat PBCK call frames

`stdlib:host_pbck_frame_calls.sapp` adds a PBLang-native execution contract for
30 flat PBCK-v1 opcodes: the existing scalar/local/control subset plus closure
creation, local and non-local captures, shared mutable upvalue cells, normal
calls, close-upvalue stack cleanup and a tail-call trampoline. Runtime limits
bound steps, active frames, per-frame and aggregate locals, operand stacks and
closure allocations. `prepare()` validates and decodes constants once into a
frozen flat program; `run_prepared()` avoids repeated plan validation on the hot
path, while `run()` remains the validate-and-execute convenience API. Optional
exact-i32 execution rejects fractional values and overflow instead of inheriting
target-specific wrapping behavior.

The call-frame engine and the full `stdlib:host_pbck_vm.sapp` now use the
canonical PBCK frame model: arguments and local declarations occupy stack slots,
`GetLocal` addresses those slots, and closures retain the original slot cell.
`SetLocal` may replace an existing slot or initialize exactly the next slot, so
older explicit-store fixtures remain valid without permitting sparse locals.
This fixed a real compatibility defect hidden by hand-authored tests. The
compiler-emitted PBRemote probe now executes in the flat engine as function 1918
with result 42, 21 steps, two frames, two closures and one tail call. The
function ID is artifact-specific and selected at build time, not hard-coded.

The module rejects unsupported opcodes before execution and also rejects varargs
and typed parameters until their portable semantics are implemented. The test
`tests/semantics/host_pbck_frame_calls_test.sapp` checks generic-VM parity,
nested captures, shared mutation, compiler-emitted stack locals, unused
unsupported functions outside the selected reachable graph, deterministic probe
selection, defaults, arity, frame/step/allocation quotas and exact-i32 overflow.
A runnable example is in
`examples/host_pbck_frame_calls/main.sapp`; the reproducible comparison with the
generic VM is `bench/host_pbck_frame_calls_stages.sapp`. This is the completed
PBLang call-frame prerequisite for generated Android execution; it does not yet
bind Android host capabilities or execute arbitrary whole PBRemote artifacts.
No Rust source was added.

On the correctness-gated 21-step compiler-shaped closure workload, 9 samples of
500 warm runs measured a prepared flat median of 0.124265 ms/run and a generic
VM median of 0.103290 ms/run. The flat path is therefore still 20.31% slower on this small
workload; the result and limitations are recorded without a performance claim in
`bench/results/2026-08-02-host-pbck-frame-calls/REPORT.md`.

## 2026-08-02 - Prepared scalar PBCK frame execution

The exact-i32 contract now has explicit regression coverage for signed remainder,
fractional and overflowing division, add/subtract/multiply/negate overflow,
positive power overflow and exact negative powers. These cases define the
portable behavior required by generated JVM runtimes instead of relying on
Java's wrapping integer operations or platform-specific `Math` APIs.

`stdlib:host_pbck_frame_scalar.sapp` provides a bounded flat-frame execution
contract for 24 scalar, local, stack and control-flow PBCK-v1 opcodes. It
preserves the generic VM's truthiness, arithmetic, comparison, default argument,
conditional-jump peek and return semantics. Unsupported globals, calls,
collections, exceptions, closures and host capabilities are detected before
execution and fail closed instead of silently changing behavior.

`prepare()` performs full frame-plan validation once and copies a compact frozen
program; `run_prepared()` retains per-step opcode, local, jump, stack and
instruction-budget guards. `run()` remains the validate-and-execute convenience
path. `find_i32_probe()` validates a whole plan once and deterministically selects
a zero-argument function whose executed values remain exact signed i32 values;
fractional arithmetic, overflow, non-scalar values and resource failures are
rejected. The semantic test checks parity with `host_pbck_vm.sapp`, all 24 declared
opcodes, defaults and negative resource/corruption cases. The runnable example is
`examples/host_pbck_frame_scalar/main.sapp`; the reproducible benchmark is
`bench/host_pbck_frame_scalar_stages.sapp`. This completes the shared scalar
frame subset, not calls, platform capability bindings or Android whole-program
execution. No Rust source was added.

On the correctness-gated 21-instruction sum workload, 9 samples measured a
prepared scalar median of 2.858 ms/run versus 4.048 ms/run for the generic VM,
29.38% lower. One-time preparation measured 0.129 ms/run. The complete method,
p95 values and limitations are recorded in
`bench/results/2026-08-02-host-pbck-frame-scalar/REPORT.md`.

## 2026-08-02 - Exhaustive host-PBCK frame dispatch contract

`stdlib:host_pbck_frame_dispatch.sapp` defines the canonical structural dispatch
contract for all 48 PBCK-v1 opcodes. It provides allocation-free integer family
and operand-kind lookup on the scan hot path, fail-closed metadata access and a
deterministic bounded whole-plan scan with fixed-size counters and checksum.
This is execution-shape coverage, not a claim that platform semantic execution
is complete. `tests/semantics/host_pbck_frame_dispatch_test.sapp` exhaustively
checks every opcode plus invalid IDs and positive/negative frame-plan scans.

## 2026-08-02 - Bounded host-PBCK frame-program access

`stdlib:host_pbck_frame_plan.sapp` now exposes checked function, instruction,
capture, constant, default and parameter metadata accessors. Every public index
must be a non-negative integer inside the validated local range; malformed
function IDs, value slots and nested table indices fail closed. The semantic
test covers positive closure/default/parameter lookups and negative type/range
cases. This completes the shared platform frame-access contract, not opcode
execution or platform host adapters. No Rust source was added.

## 2026-08-01 - Integrity-bound host-PBCK frame binary

`stdlib:host_pbck_frame_binary.sapp` serializes a validated canonical frame plan
to deterministic `PBFP-v1` bytes for generated platform runtimes. The bounded
decoder rejects bad magic/version, truncation, trailing data, invalid UTF-8,
count/range violations and integrity failures before returning a plan. Function
metadata, instructions, captures, all supported constant kinds, constant/default
slots and parameter types survive byte-identical encode/decode/re-encode tests.

The envelope uses SHA-256 over the canonical Base64 representation of the payload.
This permits the existing optimized runtime primitive to hash arbitrary binary
without adding a native implementation. The small-fixture benchmark measured
0.430 ms encode and 0.201 ms decode per iteration over 100 iterations. This is a
portable frame transport prerequisite, not an Android whole-program executor.
No Rust source was added.

## 2026-08-01 - Canonical host-PBCK frame plan

`stdlib:host_pbck_frame_plan.sapp` now converts bounded compact PBCK-v1 input
into a stable flat IR for platform code generators. It preserves every function
signature, local instruction range, constant/default/parameter range and closure
capture while normalizing recursive function constants to integer function IDs.
The builder and explicit validator reject invalid opcodes, jumps, local slots,
constant slots, captures, signatures, nesting and configured resource budgets.

The semantic contract includes nested mutable-upvalue metadata, compact decoding
parity and corruption tests. The microbenchmark measured 0.354 ms per compact
decode+flatten and 0.00465 ms per safe instruction lookup on an 18-instruction
fixture. This module is a platform-lowering prerequisite; it is not itself an
Android JVM executor.

## 2026-08-01 - Complete PBLang-native PBCK-v1 opcode runtime

`stdlib:host_pbck_vm.sapp` now executes every opcode in the canonical 48-entry
PBCK-v1 contract. The completed surface adds exports, LIFO deferred calls,
exponentiation, safe attribute/index access and result unwrapping. Result
failures use the existing guest exception path, invalid safe-access key types
fail closed, and pending deferred calls prevent tail-frame reuse so cleanup is
not skipped. The VM remains bounded by the existing instruction, frame, stack,
local, output and collection limits.

`tests/semantics/host_pbck_vm_full_test.sapp` verifies the complete registry,
positive and negative behavior for every newly implemented operation, LIFO
cleanup, return-value preservation and defer-aware tail calls. The runnable
example is `examples/host_pbck_full_runtime.sapp`; the reproducible warm phase
benchmark is `bench/host_pbck_full_runtime_stages.sapp`. This completes the
generic PBLang host VM contract, not the platform-specific Android adapters or
whole-program PBRemote execution on Android.

The final clean-load warm benchmark ran 9 samples of 1,000 executions: median
108.533 ms per sample (0.108533 ms per execution) and p95 113.079 ms. The full
semantic suite passed 740/740 after the final native-compatibility correction.
These are local host measurements and do not claim Android or cross-language
performance.

## 2026-08-01 - Bounded pure host-PBCK runtime adapters

`stdlib:host_pbck_pure.sapp` now provides 65 deterministic, PBLang-native
adapters for collections, maps, strings, numbers, bit operations, JSON,
Base64, hashing/HMAC, constant-time comparison, URL encoding and pure HTTP
message helpers. Every registry entry has a real dispatch path and a
table-driven semantic test. Arity, collection growth, expansion and output
limits fail closed. Pure builtin failures enter the guest exception path so a
PBCK `try` handler can process them without terminating the host VM.

Filesystem, process execution, FFI, sockets, graphics, clocks, entropy and OS
state remain explicit injected capabilities. They are not silently exposed by
the pure runtime. Against the current PBRemote portable artifact this reduces
the statically unresolved host surface from 128 to 67 names while retaining
100% coverage of its 147,653 instruction instances. This is a shared host
runtime improvement; generated Android still needs platform adapters and
whole-program dynamic-value lowering before it can execute the complete core.

## 2026-07-31 - PBLang-native host-PBCK decoder and VM

`stdlib:host_pbck_vm.sapp` now executes explicitly injected host adapters
instead of only recognizing their names. Adapters receive one argument array,
must be callable, and fail with redacted adapter-name-only diagnostics. Runtime
metrics include the exact host-call count. `stdlib:host_pbck_link.sapp` adds a
bounded, non-executing whole-artifact analysis of global definitions, reads,
exports and unresolved host dependencies. This closes the generic adapter
dispatch correctness defect; it does not claim that platform-specific Android
adapters have been implemented.

`stdlib:host_pbck_decode.sapp` now decodes native PBCK-v1 without the retained
Rust loader and rejects malformed versions, counts, nesting, strings, captures,
constants, jumps, spans and trailing bytes under explicit budgets.
`stdlib:host_pbck_vm.sapp` originally executed the 40 opcodes used by the current PBRemote
portable artifact, including shared mutable upvalues, nested calls, cross-frame
guest exceptions and trampolined tail calls. Compact decoding reduced the full
PBRemote benchmark from 391,140 to 201,092 KiB peak RSS and from 819.470 to
742.830 ms median decode latency. This is host-side PBLang evidence; it does not
claim that generated Android DEX already executes the complete portable core.

## 2026-07-21 - PBLang-native SHA3-256

`stdlib:sha3.sapp` implements FIPS 202 SHA3-256 entirely in PBLang. It stores
each Keccak lane as exact low/high 32-bit halves and performs potentially large
left shifts in masked 8-bit steps so no intermediate exceeds the exact integer
range used by the runtime value model. Empty, `abc`, 200-byte multi-block and
byte-array vectors pass. PBRemote consumes this primitive for cryptographic Tor
v3 onion-address checksum validation; this does not by itself constitute a full
Tor client.

## 2026-07-21 - Android TLS 1.3 capability accuracy

`stdlib:android_emulator.sapp` no longer labels every API 21+ network plan as
TLS 1.3. `tls13_status(...)` models the Android platform guarantee at API 29+;
older modeled runtimes fail closed, and `network_plan(...)` rejects secure plans
when that provider is unavailable. Plans expose their provider and state that no
external transport installation is required. Current, legacy-negative and
cleartext-negative semantic cases pass as part of the 723/723 suite.

## 2026-07-21 - Typed JVM emitters and Android control-flow VM

`stdlib:jvm_classfile.sapp` now validates and emits compact integer/reference
local loads and stores, signed `iinc`, and primitive `newarray` instructions.
PBRemote consumes these APIs in its generated Android PBCS-v2 VM, whose packaged
231-byte loop exercises arithmetic, locals, globals and forward/backward control
flow with explicit resource limits. PBLang semantics passed 719/719 after the
change. This remains a bounded JVM emitter, not a claim of a complete JVM backend.

## 2026-07-20 - Bounded JVM branch assembly and Android VM consumer

`stdlib:jvm_classfile.sapp` now has a PBLang-native label assembler for signed
16-bit JVM branches. It resolves forward and backward edges deterministically
and fails closed on invalid opcodes, duplicate/unresolved labels, corrupt patch
metadata, invalid targets and bytecode-size overflow. PBRemote uses it to emit a
bounded PBCS-v2 numeric/global decoder and executor directly as Android DEX
input. The complete PBLang semantics suite passed 719/719 in 44.08 seconds on
the verification host; this is a correctness result, not a universal JVM or
Android-runtime claim.

## 2026-07-20 - Bounded Android userspace emulator and PBCS execution

`stdlib:android_emulator.sapp` now composes the PBLang platform/device models,
self-hosted PBCS v2 container and bounded PBLang VM into an executable Android
userspace model. It verifies application/package contracts, create/start/resume
and pause/stop/destroy lifecycle transitions, system-owned permission grants,
explicit exported-component policy, per-package private storage, protected
services, foreground input, TLS-only network plans and budgeted PBCS execution.
Lifecycle mutations preflight their event capacity so quota failure cannot leave
an application in a partially advanced state. Uninstall removes package-private
data, and service payloads fail closed when they are not bounded JSON values.
The device signal model also now
accepts the canonical runtime `bool` type instead of the nonexistent `boolean`
type label.

The semantic suite passed 719/719. A nine-sample PBLang benchmark measured a
0.240 ms median for an install plus complete Activity lifecycle and 42.183 ms
for a 1,000-iteration PBCS integer workload; the latter had 2.15% CV and used a
238-byte artifact. These are userspace-emulator measurements on this host, not
ART, kernel, GPU-driver or physical-device performance claims. PBRemote 0.4.2
uses the model as a mandatory pre-signing runtime-probe gate while preserving an
explicit `full_protocol_runtime=false` boundary for the Android package. No Rust
source was added.

The physical-device verifier now distinguishes asleep and securely locked
devices before accessibility work. A semantic test that previously compressed
the growing 86 MiB standard-library tree was replaced by a deterministic
recursive fixture: isolated runtime fell below the 30-second timeout to 0.43 s,
and the full 719-test suite fell from 176.76 s to 62.60 s on this host.

## 2026-07-19 - Lazy direct-codegen sessions and bounded source macros

The canonical smart-worker semantic run passed 719/719 tests with zero failed,
ignored, measured, or filtered tests. A fresh `cargo audit` of the retained Rust
bootstrap found `crossbeam-epoch 0.9.18` and `anyhow 1.0.102`; the dependency
lock was updated to the patched `0.9.20` and `1.0.103` releases without adding
Rust source. The refreshed advisory scan reports zero known vulnerabilities and
zero unsound advisories. Four unmaintained and one yanked transitive dependency
warnings remain migration work and prevent an unqualified production-ready
security claim.

The refreshed phase benchmark measured 1.480 ms process startup, 37.404 ms
backend import, 1.652 ms in-process emission, 42.357 ms full uncached
publication, and 9.342 ms verified warm publication. Six-artifact sessions
measured 249.950 ms for independent processes, 55.485 ms for one cold session,
and 10.653 ms for a warm session (1.776 ms per artifact and 23.46x faster than
independent processes). These are compiler pipeline measurements, not generated
program runtime.

`stdlib:build.sapp` now replaces its old shell-string/fake-C compile path with
a bounded adaptive builder. It exposes deterministic plans, manifest-confined
project entry discovery, direct x86-64 plus hosted-C99 selection, explicit PBCK
runtime fallback, content digests, argv-only subprocesses, and reusable smart
sessions. The companion `tools/pbbuild.sapp` supports dry-run plans and explicit
goal/backend/target policies. A session benchmark measured 13.01 microseconds
per plan, 5.084 ms setup, 35.700 ms cold direct compile, 0.752 ms warm verified
publication, and 4.566 ms for a six-artifact warm batch (0.761 ms/artifact).
Warm numbers require the same session snapshot and are not cross-process claims.
Hosted C99 builds resolve the current command directly on Linux before checking
`PATH`, with `PBLANG_BIN` as an explicit operator override. This prevents a
stale system installation from silently compiling a project with different or
slower compiler behavior.

The new bounded `stdlib:jvm_classfile.sapp` emitter generates deterministic JVM
class bytes directly from PBLang and is used by PBRemote's Android 14 companion
packager. Its current strict ASCII constant-pool and small-launcher scope is
documented rather than presented as a complete JVM backend. The defensive
secret scanner also now rejects common artifact-name false positives while
retaining named-token and 24+ character high-entropy credential detection.

`stdlib:android_adb.sapp` adds bounded real-device verification without adding
foreign application code. It validates device serials and authorization,
invokes the official ADB tool with fixed argv and external deadlines, separates
post-install launch from cold restarts, scans package-scoped crashes, and
collects memory, accessibility interaction and GPU frame metrics without screen
capture. PBRemote passed this gate on Android 14/API 34 ARM64.

The fixed 24-row competitor matrix completed with 22 available successful rows,
two explicit Zig-unavailable rows, and no failures. The integer-kernel direct
x86-64 artifact ran in 1.963 ms median with 264 KiB RSS and 507 bytes on disk,
versus C at 3.973 ms, 1,496 KiB and 14,472 bytes. PBLang direct warm compilation
remained slower at 42.760 ms versus C at 25.005 ms. The separate 90-row
interpreter category matrix completed with 54 available successes, 36 explicit
unavailable rows, and no failures; its integer loop remained slower than Python
and Lua. No universal fastest-language claim follows from these workloads.

## 2026-07-29 - Precompiled direct x86-64 driver

The PBLang source linker now scans compiler-sized ASCII syntax through UTF-8
byte indexes. A dedicated PBLang-only cache links the direct x86-64 tokenizer,
parser, ELF writer and code generator once, freezes them to verified PBCK and
binds reuse to the exact compiler binary plus dependency graph.

The current phase suite measured 43.804 ms median source-import publication and
7.169 ms precompiled publication (6.11x), with peak RSS reduced from 19,472 to
12,720 KiB (34.68%). Initial cache publication validates PBCK structure and a
generated ELF before atomic rename. The cached driver occupies 716,463 bytes,
and host `emit-pbck` remains a documented bootstrap boundary. Focused linker,
cache-contract and phase tests pass; no Rust source was added.

The PBLang-only direct x86-64 compile session now imports its 270 KB backend
only on the first actual cache miss. On this host, six-artifact warm builds
moved from 98.504 ms to 22.392 ms median (4.40x faster, 77.27% lower latency),
or from 16.417 to 3.732 ms per artifact. A cold session measured 119.208 ms
versus 531.297 ms for six independent compiler processes (4.46x faster).

The phase benchmark now separates process startup (3.569 ms), backend import
(78.930 ms), in-process machine-code emission (3.823 ms), full uncached
publication (88.861 ms), and verified warm publication (18.367 ms). The emitter
is 4.30% of the uncached latency; cold source import remains the single-file
bottleneck and is not mislabeled as machine-code generation time.

`stdlib:macro.sapp` adds bounded callable expanders, recursive expansion budgets
and an inspectable `@record` convenience inspired by Crystal metaprogramming.
`stdlib:freeze.sapp` provides deterministic import linking and atomic verified
PBCK publication for normal-sized projects. Compiler-sized self-hosted
flattening remains too slow in the current interpreter and is not claimed as a
production cold-compiler cache. No Rust code was added.

## 2026-07-19 - Verified compiler caches and batch direct codegen

PBCS v2 now persists optimizer fused operands, reads v1 artifacts, validates
resource budgets and all references, rejects trailing bytes, and fixes u32
encoding for chunks above 255 entries. Public loader entry points use the
bounded self-host VM by default.

The self-hosted frontend has an opt-in persistent imported-module cache keyed by
processed source plus the current compiler/VM revision. Hits verify metadata,
SHA-256, byte length, and the full PBCS structure. The direct x86-64 backend has
a PBLang-only multi-file compile session that imports/fingerprints once, compiles
and validates all jobs before output publication, and reports cache failures
without changing compile correctness.

Six independent compiler processes measured 445.675 ms median versus 102.972 ms
for one cold-artifact session (4.33x) and 81.758 ms for verified warm artifacts
(5.45x). A 300-function imported module measured 128.271 ms uncached, 84.549 ms
warm (1.52x), and 271.464 ms for its first verified store. The cold store is
therefore an explicit cost, not hidden as an improvement. Build, all phase
suites, 712/712 PBLang semantics, and 203/0/1 release Rust oracle tests pass.
Checked-in bootstrap snapshots and their freshness CI gate remain unimplemented;
the runtime cache is not represented as a substitute. No Rust code was added.

## 2026-07-18 - Compile/import, C99 modulo, and interpreter dispatch

The direct Linux x86-64 driver now has a PBLang-written, content-addressed
artifact cache. Its key includes source and backend dependencies; hits validate
metadata, SHA-256 bytes, size, and ELF64/Linux/x86-64 structure before atomic
publication. The initial same-turn uncached median moved from 120.057 to
78.954 ms (34.24% lower). An independently saved run measured 97.660 ms
uncached and 23.998 ms for a verified warm hit. Cold backend import remains the
dominant phase and cache hits are not presented as cold-compiler results.

Hosted C99 integer-profile modulo now exposes safe nonzero constants to the C
optimizer and uses a checked helper for dynamic zero and `LONG_MIN % -1`.
Literal zero rejects during transpilation. A paired 19-sample run measured
14.675 ms for the floating profile and 5.915 ms for integer mode (59.69% lower)
with identical 16,008-byte artifacts. Integer mode remains explicit because it
does not preserve fractional-number semantics.

The PBLang-written tree interpreter prepares compact safe leaf-binary plans and
consumes them directly in plain assignments and while conditions. The saved
same-process loop control measured 170.536 ms optimized versus 223.439 ms with
plan metadata disabled (23.68% lower). Generic nested expressions, strings,
short-circuit operations, and control flow retain the prior fallback. This does
not modify the Rust host VM represented by the broad interpreter benchmark.

The refreshed fixed matrix completed 22 rows, with two explicit Zig-unavailable
rows and zero failures. Direct x86-64 compile remained 1.32x slower than C in
that run; runtime medians and all load-sensitive reversals are reported without
a universal winner claim. PBLang build, phase suites, focused tests, 203/0/1
release Rust semantics tests, and the canonical 695/695 PBLang suite pass. Raw
evidence and limitations are under
`bench/results/2026-07-18-compile-c99-interpreter/`.

## 2026-07-18 - Reproducible cross-language benchmark core

The new pure-PBLang `stdlib:repro_benchmark.sapp` runner uses a fixed internal
registry for the PBLang interpreter, hosted C99, direct Linux x86-64 codegen,
C, C++, Rust, Go, Java, C#/Mono, Python, Node.js, and Zig. It bounds repetitions,
timeouts, output, and temporary state;
uses random UUID workspaces; rejects nonzero exits and checksum mismatches; and
records first/cold runs separately from warmed median/p95/raw samples. Release
artifacts are stripped where supported. Environment, toolchain, wall, CPU,
peak-RSS, artifact, and correctness evidence remain separate metrics.

The local i7-1195G7 matrix produced 22 successful rows, 2 explicit unavailable
Zig rows, and no failures. On the one-million-iteration integer fixture, direct
PBLang x86-64 codegen measured 5.361 ms median versus C at 4.011 ms and hosted
C99 at 15.386 ms. The 487-byte generated ELF was 96.6% smaller than the
14,472-byte C artifact. The PBLang interpreter measured 277.461 ms versus Python
at 126.454 ms. These are measured strengths and optimization gaps, not a
fastest-language claim. The 22-category ledger explicitly identifies
partial, conditional, and not-yet-comparable areas instead of fabricating rows.
The full methodology and raw samples are under
`bench/results/2026-07-18-reproducible-core-matrix/`.

The direct row is correctness-gated by its PBLang safety certificate, ELF header,
process result, output bound, and exact checksum. Unsupported hosts remain
explicitly unavailable. A working example is in
`examples/repro_codegen_benchmark/`; the project now also has the PBLang-native
`src/main.sapp` entry declared by `pblang.toml`, so self-hosted `pblang build .`
has a real entrypoint. All focused benchmark contracts and the canonical
semantic suite pass: 689/689, with zero failed, ignored, or filtered tests, in
205.21 seconds and 1,801,672 KiB whole-runner peak RSS. The clean 24-row matrix
completed in 16.06 seconds with 95,236 KiB process peak RSS. The additional
six-category interpreter matrix produced 54 passing, 36 unavailable, and zero
failed rows. No Rust code was
added.

## 2026-07-18 - Cost-aware hardware and bounded storage routing

The pure-PBLang compute scheduler no longer sends work to CPU threads or a
registered accelerator from item count alone. Versioned policies now carry
bounded work, transfer, thread-launch, device-launch, and minimum-useful-work
hints. Arithmetic saturates at the exact-number ceiling, CPU worker count adapts
to useful work, and GPU/NPU/SIMD/distributed routes require both a compatible
registered runtime and enough work to amortize launch plus transfer cost.
Accelerator failures use the same CPU decision instead of maximum parallelism.

Device arrays, weights, task counts, policy mutation, transfer budgets, and
storage paths fail closed under hard limits. `storage_read_controlled` keeps
small batches serial and uses bounded contiguous chunks for larger batches;
the existing result-only helper remains source-compatible. It bounds path and
worker metadata, not whole-file bytes returned by the current `fs_read` builtin.

Focused compute/control tests, the new hardware-routing semantic test, runnable
example, and two correctness-gated benchmarks pass. Nine warm, interleaved local
samples of the cheap 128-item kernel measured 42.397 ms median for cost-aware
serial execution and 43.533 ms for forced four-worker execution (about 2.6%
lower median), with 52.884 and 53.404 ms p95 respectively. These measurements describe this VM/thread
workload on the recorded i7-1195G7 only; no general language-speed claim is made.
The canonical eight-worker semantic run passed 685/685 in 265.61 seconds with
1,727,616 KiB whole-runner peak RSS. No Rust code was added.

## 2026-07-18 - Bounded pure-PBLang thread supervision

The previous `thread.supervisor()` retained timeout values without enforcing
them, reported every retained handle as alive, let child VMs mutate parent-owned
maps, and cleared records while claiming workers had been killed. The replacement
keeps lifecycle state in the parent VM, caps retained records at 1,024, validates
and clamps deadlines, distinguishes completion/failure/timeout/cancellation, and
supports deterministic status, waiting, late cleanup, and reaping.

Cooperative cancellation now crosses the VM boundary through a buffered channel
instead of a non-transferable atomic handle. The child-local token latches the
signal and reports whether it actually observed cancellation. A successful task
that ignored a request stays `completed`; a failing task stays `failed`; neither
is mislabeled as cancelled. `kill_all` remains source-compatible but reports
`preemptive: false`, while `unreaped` and `detached` expose workers still needing
cleanup. Blocking native calls still require timeout support or disposable
process isolation at their own boundary.

The design was informed by timeout/watchdog and supervisor references elsewhere
in AAP, then reimplemented in PBLang. The focused semantic test, 15 adjacent
concurrency tests, runnable example, and correctness-gated phase benchmark pass.
The final canonical suite passes 684/684 in 328.52 seconds with 1,766,092 KiB
whole-runner peak RSS. No Rust code was added.

## 2026-07-18 - Controlled polyglot, FFI, parallel, and device execution

The PBLang-written thread pool now bounds workers and queued tasks, rejects
submission after shutdown, creates transferable worker closures through
top-level factories, and applies immutable worker outcomes to parent-owned task
state. Parallel successes, failures, status, and priority-ordered results are
therefore counted consistently. `race` no longer depends on exec-gated sleep
and terminates when every candidate fails.

`compute.sapp` now distinguishes detected hardware from a registered,
executable generic runtime. Its versioned policy supports deterministic dry-run
routing, serial/parallel thresholds, bounded CPU workers, workload-compatible
accelerators, strict no-fallback mode, and observable CPU fallback.
`accelerator.sapp` validates runtime kinds and hooks and contains runtime
failure in a structured dispatch envelope.

Polyglot policies require callable execution runners, enforce language and
source-size limits in every mode, and retain runner-free report mode. The safe
FFI control plane uses exact library/symbol allowlists, validates arity/types,
bounds argument count, and denies raw pointers unless explicitly enabled. It
only narrows the native host `FfiPolicy`. Focused tests, a runnable example, and
the correctness-gated benchmark pass. The canonical eight-worker semantic run
passed 682/682 in 163.68 seconds with 1,698,940 KiB process peak RSS.

## 2026-07-17 - Bounded compiler suggestions and cross-file name resolution

The PBLang-written type checker no longer silently turns an unbound identifier
into `any`. It emits `undefined-name`, `unknown-field`, or `unknown-variant`
diagnostics and uses the shared pure-PBLang
`compiler/diagnostic_suggestions.sapp` engine for optional fix guidance. The
engine caps names at 128 characters, candidates at 4,096, and edit distance at
three, de-duplicates candidates, exits distant rows early, and resolves equal
matches lexically so map iteration order cannot change the result.

Generic builtin lookup now reuses one registry map per type environment instead
of reconstructing the full map at each identifier. Across seven local samples,
5,000 isolated legacy rebuild lookups had a 388.6189 ms median and cached
lookups had a 1.9385 ms median (200.48x). The 128-statement generic-builtin
typecheck phase measured 6.4327 ms/check median. One full measured process used
21,056 KiB peak RSS. These figures describe this compiler workload only.

Project compilation now performs a bounded declaration prepass and passes the
result through the explicit `extern_names` typecheck option. Valid cross-file
calls remain accepted, but a name absent from every translation unit still
fails closed. Missing globals inside a protected `try` body remain catchable at
runtime, and parser-desugared recursive class tables are predeclared before
their methods are checked. Frontend lint and `stdlib:diagnostics.sapp` share the same bounded
suggestion policy and fix-it shape. Focused suites, project/parallel compilation,
and the canonical semantic suite pass, the latter at 681/681. Evidence and raw samples are in
`bench/results/2026-07-17-bounded-diagnostics/`.

## 2026-07-17 - Bounded one-line application facades

The pure-PBLang `stdlib:app.sapp` facade now exposes compact CLI, GUI, web,
REST, desktop, full-stack, game, multimedia, database, networking, automation,
AI, and ML entry points. REST reuses the hardened web framework and desktop
reuses the real GUI form builder. The new `stdlib:app_domains.sapp` lazy-loads
the existing game, image processing, database, networking/HTTP, workflow, AI,
ML, and metrics modules and gives advanced callers their complete APIs through
`ctx.modules`.

Domain previews are plan-only unless `preview_execute: true` is explicit.
Actions, names, options, input sizes, step counts, and collected output are
bounded; only game and automation profiles may repeat. Database, networking,
and AI execution fails closed without `allow_io: true`. The facade does not
claim audio/video support: image processing is the only current multimedia
provider.

Focused adapter/domain tests and the terminating PBIDE-catalog example pass;
the complete eight-worker semantic regression passed 680/680 with zero
failures, ignored tests, or filtered tests.
The seven-sample internal benchmark reduced median plan cost for all seven
domain profiles from 0.6806 to 0.3296 ms per batch (-51.6%), whole-process time
from 11.91 to 8.19 seconds (-31.2%), and peak RSS from 207,088 to 149,388 KiB
(-27.9%) after removing duplicate validation and profile-array allocation. The
desktop result remained within noise and image p95 regressed, both recorded
without a performance claim under
`bench/results/2026-07-17-app-domain-facade/`.

## 2026-07-17 - Executable PBLang-only migration policy

The new pure-PBLang `stdlib:migration_policy.sapp` and
`tools/pbmigrate.sapp` replace the previous file-extension evidence with a
bounded, fail-closed change-manifest gate. New Rust files and every added Rust
line are rejected without an override path. Necessary non-Rust bootstrap,
system API, ABI shim, and generated-artifact changes require an exact path,
owner, line cap, verification checks, removal plan, and non-expired review
date. Unsafe paths, malformed/oversized manifests, duplicate exceptions, and
invalid migration evidence block the gate.

Migration records enforce the ordered `reference -> parity -> verified ->
retired` lifecycle. The legacy fallback remains present through verified
parity and can be removed only in a separate retirement record backed by
behavior and full regression evidence. This does not claim that the existing
compiler bootstrap is already Rust-free; it prevents new drift while ports are
verified incrementally.

Focused policy, lifecycle, CLI, example-manifest, and vision-ledger checks pass.
The complete eight-worker semantic run passed `679/679` with zero failures,
ignored tests, or filtered tests. No Rust implementation was added.

## 2026-07-17 - Recursive type grammar, generics, and typed ADTs

The PBLang-written parser now handles canonical, bounded `array<T>`,
`map<K,V>`, `result<T,E>`, legacy `[T]`, `T?`, and `T!` annotations. Nested
`>>` tokens are consumed without mutating tokenizer output, so the same token
array can be parsed repeatedly. Type text is capped at 4,096 bytes and nesting
at 64 levels; extra generic delimiters produce parser diagnostics.

The self-hosted checker now models `bigint`, `rational`, `decimal`, result,
non-null, struct, enum namespace, and enum instance types. Struct fields and
enum payload types survive parser desugaring; generic call arguments and return
types are substituted recursively; ADT names are predeclared for forward
struct/enum cross-references. Unknown fields/variants, payload mismatches,
conflicting generic arguments, non-null violations, and non-exhaustive enum
matches have structured diagnostics. Field/variant metadata is capped at 4,096
members before scanning.

All `44/44` self-host tests and the complete `677/677` semantic suite pass with
zero failures, ignored tests, or filtered tests. Seven fresh CPU-pinned legacy
samples measured a final median of `0.266436 ms/check`, 13.32% below the first
recursive-type draft and 7.94% above the prior bounded checker; the latter is
reported as feature cost. The dedicated 30-sample benchmark reports p50 values
of 7.1533 ms for 112-statement recursive-type parsing, 7.6194 ms for valid typed
ADTs, and 6.7209 ms for 96 statements producing 48 deterministic diagnostics,
at 27,136 KiB peak RSS. Evidence is under
`bench/results/2026-07-17-selfhost-type-system-parity/`.

This does not establish general performance leadership. Postfix Result
propagation, richer typo suggestions, generic constraints, and full differential
parity with the legacy checker remain active work.

## 2026-07-17 - Bounded self-hosted type checking and real frontend analysis

The PBLang-written type checker now tracks lexical bindings and inferred or
declared types, hoists function signatures, understands scalar and nested-array
annotations, checks assignments, default values, call arity/arguments, and
declared returns, and traverses the core collection/access/control-flow AST.
Malformed ASTs, including recognized nodes with missing/invalid required fields,
and configurable node/depth exhaustion produce structured diagnostics instead
of host exceptions. `frontend_compile` and file compilation no longer convert
checker failures to an empty diagnostic array.

`frontend_analyze` now uses the actual self-hosted tokenizer/parser. The removed
whitespace/semicolon heuristic falsely rejected `selfhost_frontend.sapp` itself
because its source contained the string fixture `"= ;"`; self-check now passes
with 35 top-level statements and 5,045 tokens. The expanded suite now has 44
self-host tests and the complete 677-test semantic suite passes.

Seven pinned samples on the existing four-statement phase fixture changed from
the old shallow checker's 0.033049 ms/check median to 0.246829 ms/check for the
final fail-closed checker, an explicit 0.213779 ms/check cost. Replacing temporary
field-name arrays with arity-specific validators reduced the first safety draft
from 0.269739 to 0.246829 ms/check (-8.49%). The dedicated fixture reports
0.248713 ms/check for the legacy-compatible AST, 4.519775 ms/check for 128 typed
statements, and 4.857402 ms/check while deterministically producing 64
diagnostics. This is a
compiler-phase measurement, not a cross-language performance claim. Evidence is
under `bench/results/2026-07-17-selfhost-typecheck-parity/`.

## 2026-07-17 - Conservative builtin resolution for the tree interpreter

The pblang-only tree interpreter now scans supported lexical bindings during
AST preparation and marks a builtin call as statically direct only when no
source binding can shadow that builtin name. Programs that bind the same name
anywhere retain dynamic scope lookup, preserving global/local/function,
parameter, function-literal, and loop shadowing plus call-before-declaration
behavior. Prepared metadata is versioned and stale ASTs are rebuilt.

Seven fresh CPU-pinned paired-control processes measured median reductions of
7.87% (`len`), 6.95% (`map_has`), 7.42% (`str_starts_with`), and 11.52%
(mixed collection calls). The extra conservative preparation scan measured a
0.0308 ms median cost on the small `fib18` compile fixture. These are local
workload results, not a general language-performance claim; absolute process
comparisons were discarded due visible frequency drift. Full method and raw
evidence are in
`bench/results/2026-07-17-tree-interpreter-builtin-resolution/REPORT.md`.

## 2026-07-16 — Bounded project module linking for hosted C99 and PBIDE

`stdlib:source_inliner.sapp` now exposes `link_modules(...)`, a pblang-only,
in-memory project linker with canonical relative/`stdlib:` virtual paths,
dependency-first ordering, per-module namespaces, collision isolation, and no
filesystem/network access. It rejects traversal, absolute/backslash/foreign
paths, cycles, duplicate canonical paths, nested imports, bare module values,
reserved generated names, and invalid input types. Hard ceilings cover supplied
and reachable module count, depth, per-module bytes, total project bytes, linked
output bytes, and path length, including unused supplied modules.

The hosted C99 backend, shared execution pipeline, and PBIDE source-build facade
now accept `modules` plus `entry_path`, return compact link metadata, and build
multi-module projects including module `let`/`const`, dependency-ordered
initialization, cross-module closures, mutable module state, and global named
function values. Hosted top-level values now have C translation-unit storage and
are initialized in source order inside generated `main`, so module functions see
the same persistent state instead of an invalid local. Generated-runtime symbol
collisions fail before an artifact is returned. Native differential and
ASan/UBSan regression execution produced `41` then `42`; the updated local staged
baseline measured 100 two-module links at 2.40 ms/link, subsequent C99 lowering
at 1.23 ms, a 16,752-byte executable, and 0.37 ms/process run. These are
host-specific phase measurements, not cross-language leadership claims.

## 2026-07-16 — Bounded self-hosted VM and loop superinstructions

The pblang-written VM now has opt-in bounded execution with validated hard
ceilings for steps, frames, operand stack, locals, captured output, imported
modules, and individual string/container values. One sticky budget follows
recursive calls, closures, operator overloads, and try/catch chunks, preventing
source-level exception handling from bypassing a resource failure. Existing
`run` and `run_seeded` remain compatible; product previews use
`frontend_run_bounded` through the shared execution pipeline.

The self-hosted optimizer now deep-copies constants, recursively optimizes
nested function and try/catch chunks, and emits semantics-preserving local-add
superinstructions without crossing jump targets. The 20,000-iteration fixture
changed from 25 to 19 total instructions and 260,016 to 140,016 dispatch steps;
two measured local runs changed from 643-653 ms to 405-418 ms, with 13,728 KiB
peak RSS for the latest whole benchmark process. PBIDE, pbAI and pbOS expose
bounded source previews, while pb-mcp-server remains read-only and only reports
the limits and target plan.

## 2026-07-16 — Bounded multi-backend execution pipeline

`stdlib:execution_pipeline.sapp` now provides one lazy-loaded, pblang-only
contract for bounded register-IR evaluation, x86-64/i386/ARM64 ELF builds,
portable C99, self-hosted bytecode, WebAssembly, and validated platform plans.
Source, IR, artifact, evaluator-step, stack, and locals budgets have finite hard
ceilings; syscalls are deny-by-default and native artifacts are never executed
implicitly. PBIDE, pbAI, pbOS, and the pblang MCP server publish this same
planning/safety boundary instead of maintaining divergent target claims.

The shared IR optimizer now folds known `add/sub/mul`, removes statically zero
loops, reports each optimization stage, and avoids the previous invalid
negative-`seti` result. On the permanent known-arithmetic fixture, shared IR
changed from `9→4` instructions, x86-64 ELF `181→153 B`, i386 ELF `134→111 B`,
ARM64 ELF `180→152 B`, and C99 source `242→167 B`. These are fixture-specific
artifact results, not a claim that pblang wins every program or workload. See
`docs/Execution_pipeline.md` for the runtime and hardware security boundary.

The next shared pass propagates constants into a loop only when the body does
not write that register, then removes zero/one arithmetic identities and
identical branches. Its pinned fixture changes `12→5` IR instructions,
x86-64 `188→157 B`, i386 `140→114 B`, ARM64 `188→156 B`, and C99 `283→179`
characters. `optimization_matrix(ir)` checks bounded raw/optimized equivalence,
rejects syscalls, reports all four artifact deltas, and never runs native code.
The ordinary ARM64 ELF segment is now `R|X`, with a header regression test.
The 17-item pblang-native requirement ledger is executable through
`tools/pbvision.sapp`; broad goals remain active until their exit criteria pass.

## 2026-07-16 — Bounded shared caches and dirty-region rendering

The pblang-only `stdlib:cache.sapp` preserves its original API while adding
hard entry/unit quotas, safe TTL arithmetic, generation-safe FIFO eviction,
O(1) size/accounting, cached-nil support, and operational metrics. The shared
pbAI ensemble uses fixed-size SHA-256 prompt keys with bounded native (512) and
voter (256) caches. The pblang pb-mcp-server router uses a bounded handler-index
cache, rejects tool names over 256 bytes, and publishes dispatch-cache metrics.

`stdlib:render_cache.sapp` now bounds dirty regions (128 default, 512 maximum),
deduplicates identical marks, defers coalescing until area pressure or frame
consumption, exposes metrics, and normalizes dimensions/FPS. Coverage is in
`cache_bounded_test.sapp`, `pbai_ensemble_cache_test.sapp`,
`render_cache_test.sapp`, the MCP runtime-guard test, and the pblang-only
`cache_render_stages.sapp` benchmark.

The platform emulator now validates memory writes transactionally, uses
subtraction-first finite address checks, tracks sparse cells/files/storage/env
usage in O(1), and enforces aggregate storage/environment byte quotas. New
quota-safe list/delete/unset operations and `machine_stats()` extend the portable
userspace contract; `platform_resource_stages.sapp` measures the accounting hot
paths and locks malformed-batch atomicity.

## 2026-07-18 — Unified bounded data stores and verified pbAI cache

`stdlib:data_store.sapp` now provides one namespaced API over bounded memory,
native SQLite, and Redis. `stdlib:redis.sapp` is a PBLang RESP2 client rather
than a `redis-cli` shell wrapper: requests/responses are bounded, TLS and AUTH
are supported, and server errors remain typed. Memory defaults to lower-cost
FIFO; exact LRU is opt-in through `cache.bounded_policy`.

The persistent pbAI cache now publishes v2 entries with domain-separated
SHA-256 metadata verification, cached-nil support, TTL, per-kind quotas, staged
rename, and interrupted-replacement recovery. Legacy 31-bit entries remain
readable but are surfaced as unverified. Tests, example, benchmark, and limits
are documented in `docs/Data_Storage.md`.

## 2026-07-18 — Pure-PBLang CDN cache and origin shielding

`stdlib:cdn_cache.sapp` now provides provider-neutral browser/edge/regional/origin
cache semantics with bounded stores, SHA-256 request keys, hot-key memoization,
Cache-Control policy, ETag/304, SWR, explicit or blocking revalidation,
stale-if-error, regional promotion, immutable hashed assets, purge, and metrics.
Private/authenticated/cookie/range and unsafe response paths fail closed. This
is deployable cache behavior, not a claim that a local process creates global
POPs; provider deployment and authenticated purge adapters remain external.

`cdn_cache_test.sapp` passes, `examples/cdn_cache` exercises static origin ->
edge -> 304 -> purge, and `cdn_cache_stages.sapp` records nine-sample medians.
The local edge-hit median improved from about 3.68 ms/op to 0.108 ms/op after
bounded canonical-key memoization. HTTP serialization now uses UTF-8 byte
Content-Length and emits the proper 304 reason phrase.

The canonical eight-worker semantic suite passes 689/689 with zero failures,
ignored, or filtered tests in 205.21 seconds; whole-runner peak RSS was
1,801,672 KiB. This is suite-level evidence, not per-CDN-request memory usage.

## 2026-07-15 — Robust benchmark gates and shared IR constant-copy lowering

The pblang-only benchmark path now reports interpolated p90/p95/p99, standard
deviation, MAD, IQR, coefficient of variation, outliers, and evidence stability
instead of relying only on average/min/max. New
`stdlib:performance_budget.sapp` applies fail-closed, multi-metric regression
budgets to latency, memory, artifact size, throughput, or custom numeric fields.

The common register-IR optimizer now materializes a known `movr` value as a
constant destination write, allowing shared DSE to remove dead source and
intermediate writes. The instruction contract was completed for i386, ARM64,
and portable C99 lowering; x86-64 already supported it. On the permanent
correctness fixture, optimized IR changed from 7 to 3 instructions, with
x86-64 ELF `167→150 B`, i386 ELF `121→110 B`, ARM64 ELF `168→148 B`, and C99
source `210→154 B`. These are fixture-specific artifact results, not universal
application-speed claims.

## 2026-07-15 — Category benchmarks and bounded platform emulation

Two new pblang-only modules turn broad portability/performance goals into
correctness-gated evidence:

- `stdlib:language_benchmark.sapp` defines identical startup, recursion,
  integer-loop, array, string, and hash-map workloads for 15 runtimes. It
  auto-detects toolchains, uses bounded subprocess execution, verifies exact
  checksums, preserves every sample, and rejects caller-supplied process specs.
- `stdlib:platform_emulator.sapp` resolves validated Linux, Windows, macOS,
  Android, iOS, FreeBSD, and WASI version/architecture profiles and executes a
  bounded userspace contract: byte memory, virtual files/environment,
  normalized sandboxed paths, captured output, deterministic clock/random, and
  fail-closed portable syscalls. It maps profiles to existing checked ISA or
  native codegen models without claiming complete kernel/framework emulation.

The local baseline produced 90 language/category rows: **54 passed, 36
unavailable, 0 failed**. All 11 execution gates passed (ten ISA/quantum models
plus the platform/ABI model). Raw evidence and methodology are under
`bench/results/2026-07-15-language-platform-matrix/`.

## 2026-05-30 historical milestone - runnable self-hosted pipeline

This milestone established that the pblang-written compiler under
`stdlib/compiler/` can tokenize, parse, compile, and run representative pblang
programs end-to-end. It did not establish complete parser, checker, backend, or
stdlib parity with the legacy host. Later parity work is recorded in the dated
sections above and in `docs/VISION_LEDGER.md`; the broad self-host item remains
active until its differential exit criteria pass.

## 🤖 v0.5.6 AI Integration

| Feature | Description |
|---------|-------------|
| `pblang-ai` crate | Full AI/ML integration library with provider registry, chat completion, embeddings, token counting, RAG pipelines |
| `ai_chat()` builtin | Chat completion via OpenAI, Anthropic, Ollama, or custom providers — returns JSON response |
| `ai_embed()` builtin | Text embedding generation via any registered provider — returns JSON with vector + dimensions |
| `ai_token_count()` builtin | Token estimation for prompt budgeting (chars/1.3 and chars/4 heuristics) |
| `onnx_infer()` builtin | Capability-gated Python bridge for ONNX Runtime. Requires `allow_exec`; returns structured `{success, outputs? | error?}` maps and degrades cleanly if Python/onnxruntime/model files are unavailable. `stdlib:ai.sapp` exposes `ai.onnx_run(...)`. |
| Provider auto-registration | Auto-detects `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `OLLAMA_URL` env vars |
| Streaming support | `chat_stream()` in the AI crate for SSE-based streaming responses |
| Vector math | `cosine_similarity()`, `dot_product()`, `euclidean_distance()` in the AI crate |
| RAG helpers | `chunk_text()`, `semantic_search()` for retrieval-augmented generation pipelines |
| Sandbox integration | All AI builtins respect `allow_http`/`allow_net` capability flags |

---

## 🔒 v0.5.2 Concurrency Module Improvements

| Primitive | Description | Pattern |
|-----------|-------------|---------|
| `SafeMutex` | Production-quality mutual exclusion with timeout and RAII `with_mutex()` | Rust `Arc<Mutex<T>>`, Go `sync.Mutex` |
| `SafeRwLock` | Read-write lock allowing concurrent readers or exclusive writer | Rust `RwLock<T>`, Go `sync.RWMutex` |
| `SafeChannel` | Bounded/unbounded channels with `send`/`recv`/`try_send`/`try_recv` and timeout | Go `chan T`, Rust `mpsc` |
| `ThreadPool` | Fan-out/fan-in worker pool: `thread_pool(n_workers, tasks, fn)` | Java `ExecutorService`, Go worker pools |
| `Barrier` | Cyclic barrier for N-thread rendezvous (`barrier_new`/`barrier_wait`) | Rust `Barrier`, Java `CyclicBarrier` |
| `Once` | One-time initialization guarantee (`once_new`/`once_do_lazy`) | Go `sync.Once`, Rust `Once` |
| `Timer` | One-shot timers with `wait`/`cancel`/`remaining_ms` | Go `time.Timer`, Rust timeouts |
| `WaitGroup` | Coordinated goroutine completion (`wait_group`/`wg_done`/`wg_wait`) | Go `sync.WaitGroup` |
| `Semaphore` | Counting semaphore for rate limiting (`semaphore`/`acquire`/`release`) | Go `semaphore.Weighted`, Java `Semaphore` |
| `CircuitBreaker` | Resilience pattern: CLOSED → OPEN → HALF-OPEN transitions | Netflix Hystrix, resilience4j |
| `RateLimiter` | Token-bucket rate limiter with auto-refill ticker | Go `rate.Limiter` |
| `FanOut`/`FanIn` | Channel distribution (round-robin fan-out) and merging (fan-in) | Go pipeline patterns |

## 🛡️ v0.5.7 Runtime Memory Safety (VM builtins)

| Feature | Description |
|---------|-------------|
| `MemoryTracker` in VM | Per-VM ownership/borrow/lifetime tracker in `crates/pblang-vm/src/memory_safety.rs` |
| **VM builtins wired** | `allocate`, `borrow_immutable`, `borrow_mutable`, `borrow_unique`, `release_borrow`, `load`, `store`, `deallocate`, `push_lifetime`, `pop_lifetime`, `memory_stats` |
| Borrow rules | Mutable borrows exclusive; multiple immutable borrows allowed; `store` rejects active borrows; `borrow_unique` = use-after-move |
| Lifetime scoping | `push_lifetime` / `pop_lifetime` invalidate values allocated in popped scopes |
| Safe pointer stdlib | `memory_safety.sapp` wraps the builtins with Box, NonNull, borrow guards, Pin, Rc/Weak, checked slices, failure-reporting scopes, and quota/checkpoint/rewind arenas |
| Tests | `tests/semantics/memory_safety_builtins_test.sapp`, `tests/semantics/memory_safety_stdlib_test.sapp`, and Rust unit tests on `MemoryTracker` |
| **Not yet** | Compile-time borrow checking for ordinary values; the current safety layer is runtime-enforced |

## Bounded product runtime

`stdlib/product_runtime.sapp` now gives pblang, PBIDE, pbAI, pbOS, and
pb-mcp-server one pblang-native admission/lifecycle contract. Product profiles
reject unknown workloads, permit only limit-tightening overrides, account known
resources transactionally, contain callback failures, release scoped memory on
all outcomes, and return stable defensive snapshots. This is policy-level
protection; hard process and device quotas still require launcher/kernel support.

Coverage: `tests/semantics/product_runtime_test.sapp`, PBIDE profile assertions,
and product suites in pbAI, pbOS, and pb-mcp-server.

## 🔒 v0.5.1 Security Fixes (Comprehensive Audit)

| Issue | Severity | Fix |
|-------|----------|-----|
| `DangerousAllowAllVerifier` — TLS accepted ANY cert (MITM) | 🔴 Critical | Replaced with webpki-roots + rustls-native-certs |
| Path traversal in `static_file_response()` | 🟠 High | `_path_safe()` strips `../`, URL-decodes, returns 403 |
| Command injection in `os.which()` | 🟡 Medium | New `os_which` builtin searches PATH safely without shell |
| `fs.sapp` zero validation | 🟠 High | Absolute paths + `../` blocked by default; opt-in `*_abs()` |
| URL query params not decoded | 🟡 Medium | `url_decode()` added to `_parse_query()` |
| `NonNull` (T!) type erased | 🟡 Medium | `Ty::NonNull` preserved; nil→T! emits diagnostic |
| Only 4 HTTP status codes | 🟡 Medium | Full RFC 7231 table (60+ codes) via `http_status_reason()` |
| `env_get` required exec cap | 🟡 Low | Reading env vars is always safe; cap check removed |
| Missing builtins: `str_replace_all`, `hmac_sha256` | 🟡 Medium | Both implemented |
| MIME types missing charset | 🟡 Low | 30+ types with proper charset annotations |


## 🚀 v0.5.6 Bit-Level Optimizations & New Modules

| Feature | Description |
|---------|-------------|
| String Constant Folding | Compile-time string concatenation folding in optimizer (`"a" + "b"` → `"ab"`) |
| Peephole Optimizer | Redundant op removal: `Dup+Pop`, `Not+Not` → skip |
| Jump Threading | Multi-pass jump-to-jump resolution for branch optimization |
| `bitops.sapp` | 26 bit-level operations: AND/OR/XOR/NOT/SHL/SHR, popcount, CLZ/CTZ, bit test/set/clear/toggle, rotations, byte swap, binary/hex conversion |
| `compression.sapp` | Gzip, Zstd, Brotli compress/decompress + auto-detect |
| `serialization.sapp` | Versioned bounded parse/stringify/convert results, strict documented TOML/YAML subsets, safe JSON validation, quote-aware CSV, XML text escaping, deterministic output, legacy trusted-input compatibility |


## 🚀 v0.5.6 — Generic Types + 6 New Stdlib Modules + Full Algorithm Suite

| Feature | Description |
|---------|-------------|
| Generic Type Resolution | `TypeEnv` with generic binding/substitution, `is_subtype_of()` variance rules, `TraitDef` + `TraitRegistry` |
| `machine_learning.sapp` | 10 functions: normalize, train_test_split, accuracy, confusion_matrix, kfold, one_hot, euclidean/manhattan distance |
| `blockchain.sapp` | 10 functions: sha256d, merkle_root, mine_block, verify_block, create_wallet, sign/verify, transactions |
| `networking_advanced.sapp` | 12 functions: ping, dns_lookup, whois, port_scan, ssl_expiry, http_download, url parse/encode/decode |
| `data_structures.sapp` | 28 functions: Stack, Queue, PriorityQueue, LinkedList, Trie, LRU Cache, DisjointSet (Union-Find) |
| `image_processing.sapp` | 10 functions: grayscale, ASCII art, bilinear resize, box blur, Sobel edges, threshold, invert, brightness/contrast |
| `algorithms_advanced.sapp` | 10 tested functions: balanced 3-way quicksort, merge sort, memory-bounded Levenshtein/knapsack, O(V+E) BFS/iterative DFS, heap-based validated Dijkstra, binary search, LCS, Fibonacci DP |
| `min_priority_queue.sapp` | Reusable pure-pblang binary min-heap: O(log n) push/pop, O(1) peek, encapsulated storage, finite numeric priorities, and deterministic empty behavior |


## 🚀 v0.5.6 — LSP Goto-Def + Self-Hosting Type Checker + OS Groundwork

| Feature | Description |
|---------|-------------|
| **LSP Goto-Definition** | `textDocument/definition` — jump to fn/let/const definitions |
| **LSP Hover** | 40 builtin signatures + local fn/let type info |
| **LSP Completions** | Builtins + keywords + local declarations |
| Self-Hosting Type Checker | `compiler/typeck.sapp` — completes the 4-stage pipeline: **tokenizer → parser → type_checker → codegen** |
| Native Bitwise Opcodes | `OpBitAnd/Or/Xor/Not/Shl/Shr` in VM for 10-50x speedup |
| `systems.sapp` Enhanced | OS groundwork: inline asm stubs, port I/O, MMIO, raw pointers, interrupts, page tables, GDT, multiboot, hardware detection |

## ✅ What Works (Production-Ready)

| Component | Status | Notes |
|-----------|--------|-------|
| Core VM | ✅ Stable | Bytecode, GC, sandboxing, error handling |
| Package Manager | ✅ Functional | Lockfile, semver, file/hosted registry, HMAC verify |
| Stdlib | ✅ 313 top-level `.sapp` files | HTTP, WebSocket, SQLite, crypto, threads, quantum, AI, tensor, graphics, blocks, machine optimization, linker, LLM offload, category benchmarks, platform emulation, cross-language helpers |
| Threading | ✅ Working | VM-per-thread, channels, select, thread pool |
| OOP/Types | ✅ Working | class, struct, enum modules with full test coverage |
| New Stdlib | ✅ Added | testing, orm, async, env, log, jwt, template, graph, benchmark, crypto_advanced, class, struct, enum, http_client |
| CLI Tooling | ✅ Complete | run, test, fmt, lint, build, install, publish, search |
| Type Checker | ✅ Improved | Builtin prelude, generic call substitution, fn-literal body checks; the self-hosted `.sapp` compiler now parses/compiles/runs the full language (parity reached) |
| LSP Server | ✅ Unified | `pblang lsp` → `pblang-lsp` (tower-lsp): diagnostics, completions, hover, goto-def, workspace symbols |
| Native Executables | ✅ Production | `pblang compile` (C99), `bundle`, `pack`, `bundle --self-extract` (embedded runtime at `bin/pblang`); C runtime JSON parse/stringify now handles real structured values |
| TLS/HTTPS | ✅ Stable | rustls + webpki-roots, proper certificate verification |
| Concurrency | ✅ Enhanced | SafeMutex, SafeRwLock, SafeChannel, ThreadPool, Barrier, Once, Timer |
| Memory Safety | ✅ Wired | Runtime builtins + `MemoryTracker`; not compile-time for all values |
| Async/Await | ✅ Added | `async_spawn`, `await`, `await_all`, `await_any`, `map`, `then` via `stdlib/async_await.sapp` |
| Quantum Computing | ✅ Bridge | `pblang-quantum` crate: Qiskit, Cirq, Braket via Python subprocess; `quantum_run`, `quantum_statevector`, `quantum_backends` builtins |
| Polyglot Execution | 🛡️ Bounded bridge | `stdlib:polyglot.sapp` requires explicit trusted-source approval and routes supported interpreters through the bounded registry. Raw `polyglot_*` VM builtins are retained bootstrap internals and are blocked by the PBLang security analyzer. Compile-and-run and in-process cached-library loading remain fail-closed pending isolated brokers. |
| WASM Backend | ✅ Prototype | `pblang-wasm` crate: compile pblang AST → WAT (.wat); functions, arithmetic, strings, exports |
| LLVM Backend | ✅ Prototype | `pblang-llvm` crate: LLVM IR text emitter → compile with clang; `pblang compile --llvm`; functions, if/else, while, for, comparisons |
| Self-Hosting | ✅ **Complete** | `stdlib/compiler/` (tokenizer→parser→semantic→typeck→ir→optimizer→codegen + `selfhost_frontend.sapp` entry + `vm.sapp` interpreter, ~5,300 lines) tokenizes, parses, compiles **and runs** pblang end-to-end. All language features self-host (incl. `match`, `struct`/`enum`, `class`+colon-call, closures, nil-safe ops). Frontier is now breadth/perf |
| Self-host VM direct builtins | ✅ Optimized | Shared pure-pblang builtin registry plus optional call-site descriptors remove per-call descriptor allocation and registry lookup without changing opcodes, global shadowing, first-class builtin mutability, or Rust VM/PBCK compatibility |
| Polyglot Build | ✅ Bridge | `pblang build-polyglot`: extracts @python/@rust/@js blocks from .sapp files, compiles to .so/.pyc, generates manifest |
| Math Library | ✅ Enhanced | `math.sapp`: trig (sin/cos/tan), hyperbolic, exported `math.stats` (sum/mean/median/stddev/variance/min/max/range), native tensor-backed large vector/matrix routes, constants (PI/E/TAU/PHI), number theory (gcd/lcm/is_prime) |
| New Stdlib | ✅ Added | `units.sapp` (150+ conversions), `functional.sapp` (map/filter/reduce/compose/pipe/curry/memoize), `encoding.sapp` (hex/URL/CSV/base64/JSON), `colors.sapp` (HEX/RGB/HSL/blending/palettes) |
| Array Helpers | ✅ Enhanced | `array.sapp`: clamped slice, native-backed flatten/unique/zip/chunk/take/drop, numeric min/max, find/find_index/count/partition helpers |
| Map Helpers | ✅ Enhanced | `map.sapp`: copy/defaults/merge_all, optimized omit, has_all/has_any, sorted-key helpers, update/rename, filter/map entries |
| String Helpers | ✅ Enhanced | `strings.sapp`: native-backed repeat/pad, safe empty-needle replace/count, final-length truncate, normalize/collapse/indent/quote helpers |
| Type Checker | ✅ Enhanced | "did you mean?" suggestions for undeclared variables (Levenshtein distance), better error messages |

## ✅ Test Results

```
pblang test semantics --jobs smart --quiet → 714 passed; 0 failed; 0 ignored; 0 measured; 0 filtered (complete current semantic corpus; 2026-07-19)
pblang test tests/semantics --allow-all --quiet → same suite with script print output captured/suppressed for faster, cleaner CI logs
pblang test tests/semantics --allow-all --jobs auto --quiet → run independent test files in isolated VM workers
pblang test semantics → shorthand for tests/semantics + allow-all + quiet + resource-aware jobs
pblang test llm → shorthand for the LLM subset + timings; 3 passed; 0 failed; 186 filtered out
pblang test tests/semantics --allow-all --timings --slow 10 --quiet → show per-test durations and the slowest tests
pblang test tests/semantics --filter llm --allow-all --quiet --timings --slow 3 → LLM subset passes; pbAI-Max diagnostics use bounded samples plus native LLM-core tensor/activation routing (~4.0s Max fixture)
pblang test tests/semantics --list → discover tests without executing them
pblang test tests/semantics --exact <id> --quiet → rerun one exact discovered test id
pblang test tests/semantics --skip <substr> --quiet → run everything except matching discovered test ids
pblang test tests/semantics --fail-fast → stop after the first failure for faster debug loops
cargo test --test semantics      → 192 passed; 0 failed; 1 ignored (os_env_set_pass)
cargo test --test selfhost_drift selfhost_drift_full_parser_matches_statement_count -- --ignored --exact → 1 passed in 0.96s (legacy Rust oracle; explicit run, 2026-07-14)
cargo test --test modules        → 28 passed; 0 failed
cargo test --test typeck_compiler→ 5 passed; 0 failed
cargo test -p pblang-typeck      → 46 passed; 0 failed
cargo test --test fmt            → 8 passed; 0 failed
cargo test --test fmt_drift      → 2 passed; 0 failed
cargo test -p pblang-vm --lib    → 2 passed; 0 failed
```

Ignored tests are currently:
- **semantics:** `os_env_set_pass` — kept ignored because `env_set()` stays disabled in v0.5 security hardening (`std::env::set_var` is unsound in multi-threaded programs). This is the *only* ignored semantics test.
- **selfhost_drift:** the legacy Rust oracle still carries its historical `#[ignore]`, but the old >60s claim is obsolete: its explicit five-snippet run completes in 0.96s. The equivalent strict pblang-native `selfhost_full_parser_drift_test.sapp` is default-on and passed in the canonical suite.

> The former match-guard limitation (arm guards + `:` separator vs. the `obj:method()` colon-call postfix) is **resolved**: `match_guard_pass`, `match_guard_accepts_colon_separator`, and `match_guard_still_accepts_tilde_separator` all pass. Guards work with `:`, `then`, and `#` separators.

## 🚀 Key Improvements Since v0.4

1. **Concurrency module** — Production-quality `SafeMutex`, `SafeRwLock`, `SafeChannel`, `ThreadPool`, `Barrier`, `Once`, `Timer`, `WaitGroup`, `Semaphore`, `CircuitBreaker`, `RateLimiter`, `FanOut`/`FanIn` in `stdlib/thread.sapp` and `stdlib/concurrency.sapp`
2. **Memory safety module** — `MemoryTracker` + VM builtins (`allocate`, `borrow_*`, `load`, `store`, lifetimes); semantics test `memory_safety_builtins_pass`
3. **Async/await primitives** — `stdlib/async_await.sapp` providing `async_spawn`, `await`, `await_all`, `await_any`, `map`, `then` built on channel/thread infrastructure
4. **Spawned thread builtins** — Child VMs now inherit the parent VM's builtins module
   via parent chain, so `recv`, `send`, `channel`, etc. work inside spawned closures
5. **`select()` now returns `{index, value}`** — Previously returned just index number
6. **channel_close/is_closed** — Now correctly accept sender handle numbers
7. **String char-code builtins**: `strfromcharcode`, `strcharcodeat` — both **codepoint-based** (as of 2026-05-30): `strcharcodeat(s, i)` returns the full Unicode scalar at codepoint index `i`, consistent with `len`/`s[i]`/`slice`/`str_index_of`, so `strfromcharcode(strcharcodeat(s,i))` round-trips. For raw bytes use `str_to_bytes(s)` → `Array<u8>`
8. **stdlib/websocket.sapp** — Full `write_frame(opcode, payload)` implementation
9. **stdlib/http.sapp** — Added `parse_sse_line`, `post_auth`
10. **stdlib/quantum.sapp** — Added `to_qasm`, `measurements_to_string`, `measurements_to_int`,
   `apply_h`, `apply_cnot`, `measure_all`, `n_qubits` key on circuit objects
11. **stdlib/provider_registry.sapp** — Added `call_best`, `call_with_fallback`
12. **stdlib/collections.sapp** — Added `set_union`, `set_intersect`; queue is now array-based
13. **stdlib/math.sapp** — Added `util.clamp`, `util.lerp`, `util.smoothstep`
14. **stdlib/http_server.sapp** — Added `sse_event`, `res_sse_headers`
15. **stdlib/thread.sapp** — Fixed `channel_close_wrapper`/`channel_is_closed_wrapper`
16. **spawn capture reliability** — `spawn(...)` now snapshots open upvalues from the parent VM stack at spawn time, so common channel-capture closures work without unsafe transfer semantics
17. **upvalue crash hardening** — stale/open upvalue access now uses bounds-safe fallback behavior (`nil`) instead of out-of-bounds panic in close/get/set paths
18. **spawn/join closure wire format** — thread handoff now supports transferring function/closure values safely (when captures are transferable), unblocking parallel worker closures in `thread_pool`

## 🔴 Critical Gaps (Production Blockers)

| Issue | Priority | Effort |
|-------|----------|--------|
| ~~No TLS/HTTPS~~ | **✅ DONE** | Implemented with rustls |
| ~~Native executables incomplete~~ | **✅ DONE** | `bundle --self-extract` ships on Linux (Windows CI follow-up) |
| ~~Match guard body separator ambiguity~~ | **✅ RESOLVED** | Guards work with `:`/`then`/`#`; all match-guard tests pass |
| ~~Type system stub~~ | **✅ N/A** | Real prelude + generic substitution + fn-literal checks; struct/enum/class supported |
| ~~Self-hosted parser default-CI perf~~ | **✅ RESOLVED in pblang suite** | Direct AST construction is allocation-efficient; strict full-parser drift is default-on. Legacy Rust test metadata remains historical. |

## 📋 Recommended Next Steps

The self-host frontier is now **breadth/perf**, not grammar (all language features self-host):

1. **Differential run** — run a real `stdlib/*.sapp` module through `frontend_run` and diff output against the Rust VM.
2. **Interpreter throughput** — profile tree-walker calls, scopes, and builtin dispatch; current phase data shows execution, not tokenize/parse, is the dominant self-host cost.
3. **Bootstrap probe** — feed `frontend_compile`'s own source through `frontend_run` and measure how far the pblang compiler gets toward compiling itself.
4. **Cranelift JIT expansion** (N2-beta+) — the backend is wired and handles a numeric subset; calls/recursion/globals/closures/collections still fall back to the VM and are tracked as backend work.
5. **LSP depth** — semantic tokens, richer type-aware hover, cross-file intelligence.
6. **Hosted C99 breadth** — `ctranspile.sapp` now owns complete numeric hosted
   programs, optimized/size builds, explicit integer specialization, and bounded
   execution. Unknown AST, unresolved imports, and profile-invalid operations
   now fail closed instead of producing `0`/comments. Simple if/ternary and
   assignment expressions lower natively; mode state recovers after rejection;
   numeric fallthrough returns deterministic zero; compiler option-like C paths
   are rejected; and the fixed kernel heap checks every allocation before
   advancing. Hosted numeric closures and named function values now use pblang-
   generated closure conversion with transitive capture-by-value, persistent
   private captured mutation, higher-order dispatch, exact validated handles,
   arity checks, overflow guards, and deterministic exit cleanup. Forged numeric
   handles terminate with exit code 72 instead of becoming C pointers.
   The bounded source linker now resolves dependency-ordered project modules,
   gives top-level values persistent translation-unit storage, initializes them
   deterministically in source order, and routes global closure calls through the
   checked runtime. Freestanding/kernel closure lifetimes, GC-managed general
   values, thread-safe module initialization, filesystem/package loading, and
   full stdlib parity remain outside this backend subset.

   The common product execution facade publishes this pblang-written source path
   as build-only target `c99-hosted`. The 2026-07-16 focused run passed 16/16
   C99/kernel suites, all then-current 42 pblang-native self-host suites, 7/7 linker/pipeline/
   PBIDE/ledger suites, and 4/4 shared product-boundary suites. The linked project
   fixture covered dependency constants, mutable module state, and a global
   closure; it matched VM and native output (`41`, `42`) and passed the available
   ASan+UBSan path. The same run measured 2.397 ms average bounded linking,
   1.228 ms average linked C99 lowering, 70.431 ms C compilation, a 16,752-byte
   executable, and 0.367 ms average verified process execution over five runs.
   Timings are host/run specific; see the dated benchmark report for methodology
   and the separate language-category baseline.

**Longer-term vision:** **pbAI** — a native AI model family (a flagship pbAI-Max plus
distilled, efficient sub-models) **written in pblang**, used as a coding/debugging agent
to make pblang better, driving the **pb-mcp-server** tool catalog. The first reusable
LLM substrate is now in `stdlib/llm_core.sapp`, `stdlib/llm_tokenizer.sapp`,
`stdlib/llm_data.sapp`, `stdlib/llm_transformer.sapp`, plus the scale-up
`llm_attention`, `llm_train`, `llm_stream`, `llm_weights`, `llm_tokenizer_train`,
`llm_eval`, `llm_quant`, and `llm_binary` modules. `llm_binary` is the first explicit
bit-level token substrate for token-id bits, bitstreams, byte-bit conversion, and
binary LM example features. `llm_core` can route larger dot/vector/matrix operations
plus selected softmax/argmax/ReLU/layernorm helpers through native `tensor_*` kernels
while keeping normal array APIs. `llm_attention` now exposes live architecture updates
for adding decoder layers, extending context, tuning binary adapter strength, and
safe anti-collapse generation through `generate_safe(...)` plus
`repetition_penalized_logits(...)`; `llm_eval` includes prompt-generation quality
scoring through `generation_quality(...)`, `choose_generation(...)`, and
`max_repetition_run(...)`, plus sequence-level loss/perplexity/accuracy through
`sequence_evaluate(...)` and deterministic output-shape scoring through
`text_shape(...)`, cheap screening proxies through `regression_proxy(...)`, and
validation/regression candidate ranking through `selection_score(...)`;
`llm_data` includes bounded representative sampling plus curriculum repetition/interleaving
helpers for faster, higher-signal pbAI-Max diagnostics and training
(the Max semantics fixture now uses a small diagnostic eval set instead of
benchmark-scale full-example eval);
`llm_train` includes analytic binary-adapter backprop into `binary_projection`,
analytic deep-input updates for token embeddings, positional embeddings, binary
projection, and the LM head, adaptive error reduction, feedback loops, and guessing
loops that keep previous/current feedback and guess states, then evaluate each
correction or guess as successful or failed with a reward/punish/neutral reinforcement
signal. Elite selection ranks these candidates, keeps the best, throws the worst, and
uses the kept elite as the next generation seed.
pblang owns these generic/Nano foundations; the pbAI-Max product lives in
`AAP/ai/pbai`; its continuation path now uses pblang threads to run independent
candidate evaluations in parallel while keeping checkpoint writes parent-only; design:
`docs/pbAI.md`.

## Archived snapshot (legacy notes below)

The sections below are preserved for historical traceability and may include
older milestone wording or counts. Use the top summary and `Test Results`
section as canonical current status.

Known stale examples in the archive: old test counts, the old "Type System:
Stub" row, and older LSP/Cranelift maturity notes. Do not treat the archived
tables as live project status.

---

## 📊 Executive Summary

pblang has evolved from a promising prototype into a robust, production-quality core runtime with an expanding ecosystem. The language successfully balances two distinct use cases:

1. **Embedded Scripting**: Secure, sandboxed scripting for applications/games/tools
2. **Standalone General-Purpose**: Full-featured programming language with package ecosystem

### Maturity Assessment

| Component | Status | Grade | Notes |
|-----------|--------|-------|-------|
| Core VM | ✅ Stable | ⭐⭐⭐⭐⭐ | Bytecode execution, GC, sandboxing production-ready |
| Package Manager | ✅ Functional | ⭐⭐⭐⭐ | Local & remote registry working, lockfile v2 ready |
| Stdlib | ✅ Comprehensive | ⭐⭐⭐⭐ | 20+ modules covering web, AI, quantum, crypto |
| Semantics Tests | ✅ Passing | ⭐⭐⭐⭐⭐ | 147/149 tests passing (2 ignored) |
| Thread Safety | ✅ Working | ⭐⭐⭐⭐ | VM-per-thread model with channels |
| Native Executables | ✅ Self-extract | ⭐⭐⭐ | `bundle --self-extract` embeds runtime; E2E on Linux |
| LSP Server | ✅ Unified | ⭐⭐⭐ | tower-lsp via `pblang lsp`; diagnostics, completion, hover, goto-def, workspace symbols |
| Type System | ⚠️ Stub | ⭐ | Crate exists but unimplemented |
| TLS/HTTPS | ✅ Stable | ⭐⭐⭐⭐ | rustls wired (`tcp_listen_tls`, `tcp_connect_tls`, `tcp_wrap_tls`) |

---

## ✅ What's Already Working (Production-Ready)

### 1. Core Language Runtime ⭐⭐⭐⭐⭐

**Status**: PRODUCTION-READY

The VM core is stable and battle-tested:

- **Bytecode Execution**: Efficient stack-based VM with compiled bytecode
- **Memory Management**: RC + cycle-breaking GC (adequate for most workloads)
- **Capability-Based Security**: 
  - `VmLimits` (max_instructions, max_stack_depth, io_timeout_ms)
  - `FfiPolicy` with allowlist
  - `StdlibConfig` for feature gating
- **Error Handling**: Comprehensive `RuntimeError` system with spans and stack traces
- **Module System**: 
  - Local imports: `import "./lib.sapp"`
  - Package imports: `import "stdlib:thread.sapp"`
  - Export system with `export` keyword
- **Testing**: 147/149 semantics tests passing (2 ignored)

**Files**:
- `crates/pblang-vm/src/vm.rs` - Core VM (stable)
- `crates/pblang-core/src/parser.rs` - Parser (stable)
- `crates/pblang-compiler/src/lib.rs` - Compiler (stable)

### 2. Package Manager ⭐⭐⭐⭐

**Status**: FUNCTIONAL (registry polish + lockfile v2)

- **Lockfile v2**: `BTreeMap` deps, `archive_sha256`, `fetch_url`, git `rev`; content-hash verify on install/run
- **Registry resolve**: relative tarball URLs, post-unpack validation (always, including cache hits), symlink rejection, HMAC sidecar
- **Rehydrate**: registry/git deps from lock when cache path missing (`fetch_url` or `PBLANG_REGISTRY_INDEX`)
- **Imports**: `import("@scope/pkg")`, `import("pkg:subpath")`, bare git package names via lockfile
- **Publish**: module/entry validation, immutable version republish; `publish --registry-dir` for local registries
- **Search**: `file://` registry dirs read `index.json` locally (hosted HTTP search unchanged)
- **Tests**: `tests/install_cli.rs` (31 cases), `cargo test -p pblang-pm`

**Files**:
- `crates/pblang-pm/src/lib.rs` - Package manager core
- `tests/install_cli.rs` - Registry CLI integration tests
- `docs/Language_Semantics.md` - Package import section
- `docs/Registry_MVP.md`, `docs/Registry_hosted.md`

### 3. Standard Library ⭐⭐⭐⭐

**Status**: COMPREHENSIVE (20+ Modules)

| Module | Status | Features |
|--------|--------|----------|
| `thread.sapp` | ✅ Enhanced | spawn, join, channels, select, WaitGroup, Barrier, Once, Semaphore, CircuitBreaker, RateLimiter, FanOut/FanIn |
| `concurrency.sapp` | ✅ Added | SafeMutex, SafeRwLock, SafeChannel, ThreadPool, Barrier, Once, Timer, Atomic, concurrency stats |
| `memory_safety.sapp` | ✅ Runtime-safe wrapper | Box/NonNull/borrow guards/Pin/Rc/Weak/checked slices/arenas over the VM memory-safety builtins |
| `async_await.sapp` | ✅ Added | async_spawn, await, await_all, await_any, map, then |
| `http.sapp` | ✅ Stable | GET, POST, streaming, SSE |
| `http_server.sapp` | ✅ Stable | TCP server, routing, middleware, timeouts, case-insensitive headers, multipart/form-data uploads, chunked decode, html/redirect helpers |
| `json.sapp` | ✅ Stable | Parse/stringify, deep helpers, pretty/JSON Lines, bounded line APIs, chunked output |
| `crypto.sapp` | ✅ Stable | SHA256, base64, random bytes |
| `db.sapp` | ✅ Stable | SQLite wrapper, migrations |
| `tensor.sapp` | ✅ Working | Basic tensor operations |
| `quantum.sapp` | ✅ Working | Circuit builder, QASM emitter |
| `quantum_sim.sapp` | ✅ Working | Measurement, histogram, ECC helpers |
| `collections.sapp` | ✅ Stable | Stack, queue, set helpers; `array.sapp` and `map.sapp` expose richer data-structure helpers |
| `math.sapp` | ✅ Stable | Complex, lerp, clamp, smoothstep |
| `csv.sapp` | ✅ Stable | Quote-aware CSV/CRLF, native unquoted fast paths, linear header inference, native quote escaping, typed/safe/bounded option APIs |
| `dataframe.sapp` | ✅ Stable | Normalized tables, stable merge sort, indexed joins/pivot, linear rolling windows/column inference, quote-aware CSV, grouping/reshape/statistics/schema |
| `template.sapp` | ✅ Stable | Isolated loops, chained filters, condition helpers, strict missing-var checks |
| `validate.sapp` | ✅ Stable | Nested schemas, defaults, strict fields, batch validation |
| `config.sapp` | ✅ Stable | Layered config, typed schema maps, validation bridge |
| `datetime.sapp` | ✅ Stable | Unix-ts arithmetic, calendar add_months/add_years (day-clamped), period boundaries, quarter/day_of_year/ISO week, date_range, business-day helpers |
| `websocket.sapp` | ✅ Stable | Full RFC 6455: client+server handshake, masked/unmasked framing, streaming decode |
| `graphics.sapp` | 🟡 MVP | Basic graphics primitives |
| `ffi.sapp` | ✅ Stable | C ABI bindings |
| `fs.sapp` | ✅ Stable | File I/O |
| `net.sapp` | ✅ Stable | TCP sockets |
| `os.sapp` | ✅ Stable | Process, environment |
| `path.sapp` | ✅ Stable | Path manipulation |
| `time.sapp` | ✅ Stable | Time utilities |

**Files**:
- `stdlib/*.sapp` - All stdlib modules
- `tests/semantics/*.sapp` - Comprehensive tests for each module

### 4. Threading & Concurrency ⭐⭐⭐⭐

**Status**: MVP COMPLETE (per-VM handle registry)

- **VM-per-Thread Model**: Each thread gets isolated VM instance; spawn/join/channel/sync handles live in `VmHandleRegistry` (`Arc` shared with child VMs from `spawn`, not process-global)
- **Channels**: Message passing with `channel()`, `send()`, `recv()`
- **Select**: Multi-channel select with `select(channels_array)`
- **Stdlib Wrapper**: `stdlib:thread.sapp` provides ergonomic API
- **Thread Pool Helper**: ✅ Implemented (T5-alpha)

**Example**:
```pblang
import "stdlib:thread.sapp";

let jobs = thread.channel_new();
let results = thread.channel_new();

let worker = thread.spawn(fn() {
    let job = thread.recv_value(jobs.receiver);
    while job != nil {
        thread.send_value(results.sender, job * 2);
        job = thread.recv_value(jobs.receiver);
    }
});

thread.send_value(jobs.sender, 5);
thread.send_value(jobs.sender, nil); // termination signal
thread.join(worker);
```

**Files**:
- `stdlib/thread.sapp` - Thread stdlib wrapper
- `tests/semantics/thread_stdlib_test.sapp` - Thread test suite

### 5. CLI Tooling ⭐⭐⭐⭐

**Status**: COMPREHENSIVE

All major commands implemented:

| Command | Status | Description |
|---------|--------|-------------|
| `pblang run` | ✅ | Execute .sap/.sapp files |
| `pblang check` | ✅ | Parse + typecheck without execution |
| `pblang test` | ✅ | Run/list/filter/exact-select test suites |
| `pblang install` | ✅ | Install dependencies or packages |
| `pblang publish` | ✅ | Publish to registry |
| `pblang search` | ✅ | Search packages |
| `pblang bundle` | 🟡 | Package app (self-extract pending) |
| `pblang doc` | ✅ | Generate documentation |
| `pblang fmt` | ✅ | Format source code |
| `pblang lint` | ✅ | Lint source code |
| `pblang --annotate` | ✅ | Annotated disassembly |
| `pblang --symbols` | ✅ | Symbol table dump |
| `pblang --dump-bytecode` | ✅ | Bytecode disassembly |

**Files**:
- `src/main.rs` - CLI implementation
- `docs/Tooling.md` - CLI reference

### 6. Semantics Test Suite ⭐⭐⭐⭐⭐

**Status**: 100% PASSING (143/143 tests)

**Test Categories**:
- ✅ Block expressions and shadowing
- ✅ Closures and captures
- ✅ Arrays and maps
- ✅ JSON parsing/stringification
- ✅ Thread stdlib wrapper
- ✅ HTTP server with timeouts
- ✅ Quantum circuits and measurement
- ✅ Tensor operations
- ✅ AI streaming inference
- ✅ Collections (stack/queue/set)
- ✅ Math extensions
- ✅ WebSocket frames
- ✅ CSV parsing

**Known Issue**:
- `thread_stdlib_wrapper_pass` may timeout on slower machines (not a failure, just needs timeout protection)

**Files**:
- `tests/semantics.rs` - Test harness
- `tests/semantics/*.sapp` - Individual test scripts

---

## 🟡 In Progress / Needs Completion

### 1. Native Executables (E5) - Self-extract complete on Linux

**What's Done**:
- ✅ `crates/pblang-stub` — extract, run, `--extract-only`, `--bundle-version`, temp cleanup
- ✅ `pblang bundle --self-extract` — stub + sentinel + tarball with embedded `bin/pblang`
- ✅ E2E tests: sentinel, run without `PATH`, `--extract-only` includes runtime
- ✅ Portable `pblang bundle` (folder with copied `pblang` binary)

**Remaining** (follow-up):
- Windows CI for self-extract E2E
- Optional: true single-file without temp extract (native compile backend)

### 2. TLS/HTTPS Support (W4) - Implemented

**Priority**: MEDIUM  
**Blocker**: none (shipping; follow-up hardening only)

**What's Done**:
- ✅ Design in `docs/TLS_notes.md`
- ✅ Per-request timeout in HTTP server (W4-alpha)
- ✅ SSE support in HTTP server
- ✅ VM-level TCP read/write timeout through `VmLimits.io_timeout_ms` (no script-level `tcp_set_timeout` builtin in this build)

**What's Missing**:
- ⏳ Expanded cross-platform cert-path test matrix
- ⏳ More end-to-end TLS docs/examples for deployment presets

**Required Work** (follow-up):
1. Keep certificate validation regression tests in CI
2. Expand deployment playbooks (local certs, hosted certs, proxies)
3. Document operational hardening defaults

### 3. Type System - 10% Complete

**Priority**: LOW (for v1.0)  
**Blocker**: Major implementation effort

**What's Done**:
- ✅ `crates/pblang-typeck` crate created
- ✅ Type annotation syntax in parser

**What's Missing**:
- ❌ Type inference engine
- ❌ Type checking pass
- ❌ Struct/enum syntax
- ❌ Generic types
- ❌ Type error reporting

**Status**: Stub only, requires significant design work

---

## ⚠️ Critical Gaps (Production Blockers)

### 1. ~~No TLS/HTTPS~~ - ✅ COMPLETE

**Status**: Implemented in v0.5 with rustls
- `tcp_listen_tls(addr, cert_path, key_path)` — TLS server listener
- `tcp_connect_tls(host, port)` — TLS client connection
- `tcp_wrap_tls(handle, cert_path, key_path)` — upgrade plain TCP to TLS
- Client verification uses trust roots (`webpki-roots` + native roots where available)

### 2. Native Executables Incomplete - 🔴 HIGH (now #1 priority)

**Impact**: Cannot distribute single-binary applications  
**Workaround**: Distribute source + `pblang` runtime  
**Effort**: 1-2 weeks

### 3. No Type System - 🟢 MEDIUM

**Impact**: Limited IDE support, no compile-time type safety  
**Workaround**: Use `pblang --check` for syntax/type diagnostics  
**Effort**: 2-3 months

---

## 🎯 Recommended Next Steps (Next 2-4 Weeks)

### Week 1-2: Native Executables (E5) - HIGH PRIORITY

**Goal**: Self-extracting single-binary executables

**Tasks**:
1. Implement tarball extraction in `pblang-stub`
2. Add binary concatenation logic to `pblang bundle --self-extract`
3. Test Linux executable generation
4. Test Windows executable generation
5. Add cleanup handlers for temp directories

**Deliverable**: `pblang bundle --self-extract --output myapp` produces working `myapp` executable

### Week 3-4: TLS/HTTPS Support (W4) - HIGH PRIORITY

**Goal**: Production-ready HTTPS servers

**Tasks**:
1. Add `rustls` dependency
2. Implement `pblangStream` trait
3. Add `tcp_listen_tls()` builtin
4. Modify `tcp_accept` for TLS transparency
5. Create `stdlib/tls.sapp` wrapper
6. Test HTTPS server with self-signed cert

**Deliverable**: HTTPS server example works with `openssl s_client`

### Week 5+: Ecosystem Expansion - MEDIUM PRIORITY

**Goals**:
1. Expand LSP server (go-to-definition, hover docs)
2. Add more quantum computing examples
3. Improve error messages with suggestions
4. Add more tensor operations (slice, reshape, dot)
5. Document embedding API for Rust applications

---

## 📈 Success Metrics

### v0.6 Milestone (End of Week 4):
- [ ] Self-extracting binary on Linux
- [ ] Self-extracting binary on Windows
- [ ] TLS listener working
- [ ] HTTPS server example
- [ ] All semantics tests passing (76/76)

### v0.7 Milestone (End of Week 8):
- [ ] Native executables stable
- [ ] TLS support production-ready
- [ ] LSP with go-to-definition
- [ ] Expanded stdlib examples
- [ ] Performance benchmarks documented

### v1.0 Milestone (End of Month 6):
- [ ] Production-ready for embedded scripting
- [ ] Production-ready for web servers
- [ ] Package ecosystem functional
- [ ] Native executables stable
- [ ] Security audit passed
- [ ] Comprehensive documentation

---

## 🔧 Development Workflow

### Testing

```bash
# Run all tests
cargo test

# Run semantics tests only
cargo test --test semantics

# Run specific test
cargo test thread_stdlib_wrapper_pass

# Run with output
cargo test -- --nocapture
```

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run CLI
cargo run -- run examples/hello.sapp
```

### Documentation

```bash
# Generate docs
cargo doc --open

# Run pblang doc
cargo run -- doc examples/
```

---

## 📚 Key Documentation Files

| File | Purpose |
|------|---------|
| `docs/Future_plans.md` | Vision and long-term roadmap |
| `docs/Tasks.md` | Active backlog |
| `docs/Report.md` | Progress tracking |
| `docs/IMPROVEMENT_PLAN.md` | Detailed implementation guide |
| `docs/IMPROVEMENT_STATUS.md` | Current status (this document) |
| `docs/Spec_freeze.md` | Language semantics contract |
| `docs/Registry_MVP.md` | Package system design |
| `docs/Registry_hosted.md` | Hosted registry API |
| `docs/Threads.md` | Concurrency model |
| `docs/Native_backend_notes.md` | Executable strategy |
| `docs/Tooling.md` | CLI reference |
| `docs/TLS_notes.md` | TLS design |

---

## 🎓 Lessons Learned

1. **Test-First Development Works**: 75+ semantics tests caught regressions early
2. **Documentation as Contract**: `Spec_freeze.md` provides clear semantics
3. **Incremental Milestones**: Breaking work into weekly chunks shows progress
4. **Architecture Investment**: Clean crate separation enables parallel work
5. **Capability-Based Security**: `VmLimits` + `FfiPolicy` enables safe embedding

---

## 🚀 Deployment Readiness

### Embedded Scripting (Games/Tools)

**Status**: ✅ PRODUCTION-READY

- Core VM stable
- Sandboxing works
- Error handling robust
- Memory management adequate
- Module system functional

**Use Case**: Safe scripting in Rust applications, game engines, CLI tools

### Standalone General-Purpose

**Status**: 🟡 EARLY-STAGE (Need TLS + Native Executables)

- Package ecosystem functional
- Stdlib comprehensive
- Threading working
- Missing: HTTPS, single-binary distribution

**Use Case**: Internal tools, prototyping, non-public-facing services

### Production Web Services

**Status**: ⚠️ NOT RECOMMENDED (Missing HTTPS)

- HTTP server works
- Routing/middleware functional
- Missing: TLS termination
- Workaround: Reverse proxy with nginx/Caddy

**Use Case**: Development/testing only until TLS added

---

## 📞 Contact & Support

- **GitHub Issues**: For bug reports and feature requests
- **Documentation**: See `docs/` directory
- **Examples**: See `examples/` directory
- **Tests**: See `tests/semantics/` for usage examples

---

**Last Updated**: 2026  
**Next Review**: rolling  
**Status**: Historical snapshot section ends here; use the top summary as canonical.

## 2026-08-13 OpenSSH process-boundary slice

- `stdlib:openssh_client.sapp` provides strict SSH execution and binary SFTP
  upload/download through fixed system OpenSSH while all admission, argv policy,
  DNS pinning, resource bounds and cleanup orchestration remain in PBLang.
- Private key and known-host bytes are handed to the child through verified
  mode-0600 sealed anonymous descriptors. Passwords, agents, proxy commands,
  forwarding, tunnels, local commands and interactive prompts are disabled.
- `linux_process.sapp` now supports a caller-bounded child file-size ceiling and
  preserves explicit empty argv elements. `linux_dirfs.sapp` supports immutable
  and mutable inheritable anonymous file handoff with stable parent-PID paths.
- Temporary OpenSSH descriptors and their exact private root must be removed for
  a successful operation; cleanup is no longer best-effort.
- The focused runtime and isolated real-`sshd` contracts pass. OpenSSH supplies
  the SSH cryptography; this status does not claim a PBLang-native SSH protocol
  or cryptographic implementation.

## 2026-08-13 bounded classic-PCAP slice

- `stdlib:pcap_capture.sapp` validates closed protocol/port filters, captures
  through fixed system `tcpdump`, and parses bounded classic PCAP into metadata.
- Binary capture output stays in the PBLang supervisor's private descriptor-held
  spool. Raw PCAP, packet payloads and network addresses are not published.
- Count, snaplen, record length, original length, total file, process output,
  memory and wall time all have explicit ceilings. Failed capture exit status
  cannot be hidden by a valid empty PCAP header.
- A real two-packet loopback contract passes inside an isolated user/network
  namespace. Normal live capture still requires least-privilege OS capture
  provisioning; there is no unsafe unprivileged fallback.
