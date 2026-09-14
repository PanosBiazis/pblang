# Bounded execution pipeline

`stdlib:execution_pipeline.sapp` is the shared pblang-only contract used by
PBIDE, pbAI, pbOS, MCP tools, benchmarks, and CLI code-generation workflows.

## Inputs and targets

Register IR can be evaluated through the bounded correctness oracle or built as
x86-64 ELF, i386 ELF, ARM64 ELF, or portable C99. Full pblang source can be
compiled to checked self-hosted bytecode, executed in the bounded self-hosted
VM, lowered by the pblang-written hosted C99 backend with `c99-hosted`, or
compiled to WebAssembly. Backend modules are statically imported so the module
graph is visible to freezing; backend compilation is selected by target.

`c99-hosted` also accepts an immutable in-memory project snapshot:

```sapp
let built = pipeline.build_source(root_source, "c99-hosted", map{
    entry_path: "app/main.sapp",
    modules: map{
        "lib/math.sapp": "fn answer() { return 42; } export answer;"
    }
});
```

The snapshot linker canonicalizes virtual relative and `stdlib:` paths, resolves
dependencies before importers, gives every module a collision-safe namespace,
and performs no filesystem or network I/O. Absolute paths, backslashes, foreign
schemes, root traversal, cycles, duplicate canonical paths, nested imports, bare
module values, and the reserved `__pbi*` namespace fail closed. Module count,
depth, per-module bytes, total supplied bytes, linked-output bytes, and path
length all have configurable defaults and immutable hard ceilings. The bounded
snapshot linker now measures UTF-8 bytes for sources, paths and linked output,
including both raw and canonical path spelling. See
`docs/Source_linker.md` for the complete contract.

```sapp
let pipeline = import("stdlib:execution_pipeline.sapp");
let result = pipeline.build_ir([
    ["seti", 0, 6], ["seti", 1, 7], ["mul", 0, 1]
], "eval");
print result.evaluation.value; // 42
```

```sapp
let preview = pipeline.run_source(
    "fn sum(n) { let s = 0; for i n { s = s + i; } return s; } print sum(100);",
    map{ max_eval_steps: 10000, max_frames: 32 }
);
print preview.runtime.steps;
```

`plan_platform(os, version, arch)` resolves a validated platform profile and
selects a direct backend only when its executable format matches. Current
native emitters produce Linux ELF. Windows, macOS, Android, iOS, and other
targets use the C99 fallback until a matching PE, Mach-O, mobile package, and
ABI integration exists. WASI uses the WebAssembly backend.

## Fail-closed limits

The WASM source backend additionally requires bounded, lexically and syntactically
valid source before generation: at most 1 MiB UTF-8 and 65,536 tokens including
comments/EOF. Backend rejection returns a fixed `ok=false` result without an
artifact. Raising the facade's source limit cannot widen this backend ceiling.
See [Checked backend source admission](BACKEND_SOURCE_ADMISSION.md) for the
direct WASM/ARM64 contract and remaining limits.

The facade validates finite source, IR instruction, artifact, evaluator-step,
frame, stack, locals, output, module-count/depth/source/path, and value-size
options with immutable maxima. These are distinct admission/runtime controls,
not a complete process-memory or wall-clock guarantee. Register-IR syscalls are denied unless
`allow_syscalls: true` is explicit. Native artifacts are built but never
executed by this library. `eval` uses `codegen_ir.eval_bounded`; source preview
uses `selfhost_frontend.frontend_run_bounded`. A single sticky runtime budget is
threaded through calls, closures, operator overloads, and try/catch sub-chunks,
so language exception handling cannot clear that VM budget's resource-limit
failure. This does not establish a single aggregate budget across imported
modules and the main chunk; that frontend limitation remains open.

### Byte units and output representation

`build_source` and `run_source` reject source above `max_source_bytes` using
UTF-8 bytes before dispatching to the frontend or selected backend. A preliminary
character-count rejection avoids encoding text already known to exceed the
ceiling. For example, `λ` is one Unicode scalar but two UTF-8 bytes; `😀` is one
scalar but four bytes. Exact N-byte input is admitted at N, and rejected at N-1.
This correction intentionally tightens admission of non-ASCII source; ASCII
limits, target selection and successful program semantics are unchanged.

`artifact_bytes`, `max_artifact_bytes` and optimization-matrix byte metrics use:

- UTF-8 bytes for text artifacts such as C source.
- Element count for the byte vectors produced by current ELF/WASM backends.
- UTF-8 bytes of the existing diagnostic `str()` representation for checked
  bytecode maps and scalar evaluation results. This is **not serialized PBCK
  size**, a canonical interchange encoding, or an in-memory heap measurement.

Artifact rejection happens after backend generation. Text measurement also
materializes a UTF-8 byte array, after its preliminary character check. Neither
step enforces peak heap usage or prevents allocation inside a backend. The
snapshot linker's module/aggregate/linked-output/path byte accounting is now
corrected as well; its file-backed loader's pre-admission reads and allocations
remain a separate boundary. See `Source_linker.md` for evidence compatibility.

The `c99-hosted` source target is build-only and returns C source, not an
executed process. Unsupported AST nodes, unresolved imports, profile-invalid
operations, and collisions with generated runtime symbols fail closed. Linked
modules support transitive imports, top-level `let`/`const`, closures, mutable
module state, and named function values. Every top-level value receives static C
translation-unit storage and is initialized once in flattened source/dependency
order by generated `main`; functions therefore observe persistent module state.
Global closure calls still pass through the validated handle/arity runtime.
Freestanding `print` is rejected instead of silently
discarded. Numeric functions emit deterministic zero for pblang nil/fallthrough,
compiler option-like C paths are rejected, and the kernel bump allocator checks
negative/oversized requests plus remaining capacity before advancing its fixed
heap. The hosted native convenience runner remains separately watchdog-bounded.

Hosted float and integer C99 also lower numeric function literals and named
function values. Closure conversion preserves capture-by-value, transitive
captures, persistent writes to a closure's private environment, returned
closures, and higher-order calls. Values are validated registry handles rather
than raw function/environment pointers. Construction checks allocation and
handle-space overflow, calls check exact handle representation and arity, and an
`atexit` cleanup releases closure environments and the registry. Invalid handles
or arity terminate deterministically with exit code 72 instead of dereferencing
attacker-controlled numeric addresses. Freestanding and kernel profiles still
reject closures until they have a bounded no-libc ownership/lifetime policy.

The self-hosted optimizer recursively optimizes nested function and try/catch
chunks without mutating its input. `OpAddLocalConst` and `OpAddLocalLocal`
superinstructions collapse common loop updates only when no jump targets the
middle of the sequence. Overloaded addition still takes the normal dunder slow
path. On the permanent 20,000-iteration hot-loop fixture this reduced total
bytecode from 25 to 19 instructions and dispatch steps from 260,016 to 140,016;
two local runs measured 643-653 ms raw and 405-418 ms optimized. The latest
whole benchmark process used 13,728 KiB peak RSS. Timing and RSS are
machine/run specific, while
the instruction and dispatch counts are deterministic.

The shared IR optimizer reports instruction counts after peephole, constant
propagation, dead-store elimination, and final cleanup. It folds known
`add/sub/mul`, propagates read-only loop invariants, removes zero/one arithmetic
identities, collapses identical dynamic branches, removes statically zero
loops, and only materializes values that remain valid `seti` immediates.

`optimization_matrix(ir)` validates the common backend subset, evaluates raw
and optimized IR with the bounded oracle, rejects syscalls, and reports raw vs
optimized artifact size for x86-64 ELF, i386 ELF, ARM64 ELF, and C99. It never
executes a native artifact. On the current identity/branch fixture, shared IR
shrinks from 12 to 5 instructions; x86-64 from 188 to 157 bytes, i386 from 140
to 114, ARM64 from 188 to 156, and C99 source from 283 to 179 characters.

## Security boundary

These guarantees cover state controlled by pblang: validated IR, step and
memory-shaped budgets, artifact size, capability decisions, and deterministic
reference execution. They do not by themselves isolate a defective or hostile
kernel, driver, firmware, DMA device, hypervisor, physical bus, or hardware
implementation. Those require OS process isolation, IOMMU/device policy,
firmware trust, hardware protections, and deployment-specific controls.

## Verification

- `tests/semantics/codegen_ir_test.sapp`
- `tests/semantics/execution_pipeline_test.sapp`
- `tests/semantics/execution_pipeline_byte_limits_test.sapp`
- `tests/semantics/source_linker_bounded_test.sapp`
- `tests/semantics/selfhost_vm_bounded_test.sapp`
- `tests/semantics/ctranspile_hosted_test.sapp`
- `tests/semantics/ctranspile_module_globals_test.sapp`
- `tests/semantics/ctranspile_intmode_test.sapp`
- `tests/semantics/ide_features_test.sapp`
- `bench/codegen_ir_optimizer_stages.sapp`
- `bench/execution_pipeline_stages.sapp`
- `bench/selfhost_vm_superinstructions.sapp`
- `bench/selfhost_c99_stages.sapp`

The full ordered vision is queryable without external tooling:

```bash
pblang tools/pbvision.sapp
```

`stdlib:vision_ledger.sapp` keeps every broad requirement visible with current
evidence and a measurable exit criterion. Broad ambitions remain `active`
instead of being mislabeled complete.
