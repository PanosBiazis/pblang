# Shared closure bindings in the self-hosted compiler

Validated locally on 2026-09-22 against baseline
`27f2ef9c5d22bc780c67b4924642c6c647f392fb`.

The costs below describe the initial shared-binding implementation. The later
[grouped VM dispatch measurements](SELFHOST_DISPATCH_GROUPS.md) reduce closure
execution/creation overhead while retaining these semantics and checks.
Further [small-array allocation changes](SELFHOST_SMALL_ARRAYS.md) address
temporary argument arrays and the storage of retained captures.

## Behavior

Source compiled through `frontend_compile` / `compile_stmts` now shares a captured
binding between its owner, sibling closures and transitive closures. Each call
and each re-entered declaration has an independent binding:

```sapp
fn make() {
    let n = 0;
    let inc = fn() { n = n + 1; return n; };
    let read = fn() { return n; };
    n = 10;
    print inc();   // 11
    print read();  // 11
    print n;       // 11
    return [inc, read];
}
let pair = make();
print pair[0]();   // 12
print pair[1]();   // 12
```

Before this change the self-hosted VM copied scalar captures independently and
produced `1, 0, 10, 2, 0` for this program. The new regression fails against the
baseline at this behavioral difference.

The implementation is PBLang in `compiler/capture_cells.sapp`, invoked by
`compiler/codegen.sapp`. It adds no Rust, polyglot block, opcode or file-format
version. It does not replace the native runtime or introduce a cycle collector.

## Representation and lifetime

After lexical capture discovery, the compiler lowers captured bindings to
one-element arrays. Declaration stores construct new cells; assignments update
cell element zero. Parameters are boxed once at function entry, before defaults.
Uncaptured bindings retain ordinary local instructions. Try body and handler
chunks share the enclosing binding set and scratch slot, but receive no parameter
boxing prologue. Branch targets are remapped after expansion, including targets
at the end of a chunk. Backward branches skip the entry prologue.

A short compiler-generated factory passes cell references as parameters before
creating the actual closure. This is necessary because the retained native VM
captures local stack slots by reference: directly capturing a loop's reused slot
would make older closures observe a later iteration's cell. Factory parameters
hold stable cell references and close when that factory returns. The factory
replaces the original function constant; the function body is not serialized
twice. Optimized and ordinary native bytecode use the same mechanism.

Cleanup now clears the owner's slot, including supported block exits, explicit
throws, return, break, continue and try-handler cleanup. It does not clear the
cell contents held by escaped closures. Source assignments hold the stable cell address and index beneath RHS
evaluation, then consume them at the write; nested assignments are ordered
from outer to inner. Value computation and side effects retain their order.
A scratch-slot fallback for manually emitted unmarked stores is cleared
immediately after the write.

This is reference retirement, not a guarantee that physical RAM immediately
returns to the OS. Recursive closures can form cycles; general cycle reclamation,
external resources/FFI cleanup, and all implicit uncaught-error cleanup remain
outside this change. Shared cells also do not make mutable closures thread-safe.

## Compatibility and explicit budgets

- Old PBCS artifacts and manually authored VM chunks retain their previous raw
  upvalue semantics. Recompile source to obtain shared bindings. Existing reader
  admission checks and artifact limits remain in place.
- `VM.exec` still accepts ordinary caller-provided locals/upvalues for raw chunks.
  Compiler-generated closure internals now contain cells. Debuggers inspecting
  `__upvalues`, captured local slots or synthetic call frames must account for
  this representation; these are not source-language scalar variables.
- Low-level callers constructing `CodeGen` with `codegen_block` / `codegen_expr`
  must call `codegen_finish(cg, parameter_count)` before using the chunk. The normal
  frontend does this automatically. Finalization is idempotent; further emission
  after finalization is unsupported. Handwritten chunks are not finalized.
- Bound VM execution charges the additional instructions, operand-stack entries and
  temporary factory frame through existing guards (and a scratch slot for
  unmarked low-level stores). Capturing programs near a
  budget can now exhaust it sooner. Nested factories also consume existing PBCS
  nesting/constant budgets. No budget bypass or general source-size cap is added.
- Named nested-function scoping, general native/frontend differences and other
  code-generation backends are not declared universally conformant.

## Validation and focused review

**125 suites passed** in an isolated clean checkout: the tracked self-host suites
plus the prior runtime/operator/lifetime/resource/emulator selection. Temporary
paths in the staged test copies were redirected to a disposable test directory.
The checkout's unrelated modifications were not used as test dependencies.
The clean-tree build passed and its lock was normalized with `tools/pblock.sapp`.
Lint passed for all 11 touched PBLang files, with 10 warnings retained.
Whitespace, repository-relative documentation links and privacy checks passed.

The new matrix has **30 source cases × 8 execution routes** (ordinary/optimized,
unbounded/bounded VM, PBCS roundtrip, loader), plus finalization and legacy-VM
regressions. It covers sibling and transitive mutation, parameters/defaults,
separate calls, count/for-in/while loops, match bindings, shadowing, continue,
break, recursive closures, try/catch, assignment failure, escaping closures and
captured callable/container values. It also covers indexed/attribute setter
stack balance, nested assignments sharing the same start offset, RHS calls,
throw/catch within an assignment, and nil/boolean short-circuit branches. **60 native bytecode executions** check the
same matrix, ordinary and optimized, on the retained Linux runtime. These are
not tests on all physical devices or inside guest operating systems.

Review checked binding initialization versus assignment, owner cleanup versus
cell cleanup, fresh declarations in loops, try-frame aliasing, scratch-slot
collisions, jump relocation, nested function double-lowering, RHS failure before
mutation, admission of the generated operations and preservation of existing
budgets. No permission, desktop approval, network or FFI boundary was changed.
This is a focused correctness review, not a universal security guarantee.

[Suite evidence](evidence/selfhost-captures-validation-2026-09-22.json) and
[native matrix](evidence/selfhost-captures-native-validation-2026-09-22.json).

## Reproducible cost

Run the same workload in clean before/after checkouts with the same retained
executable:

```sh
target/release/pblang run bench/selfhost_shared_captures.sapp --allow-all -- counter
target/release/pblang run bench/selfhost_shared_captures.sapp --allow-all -- creation
target/release/pblang run bench/selfhost_shared_captures.sapp --allow-all -- numeric
```

Each process compiles once and executes three times. Counter performs 5,000
single-closure mutations, creation constructs/calls 1,000 read-only closures,
and numeric is a noncapturing control. All three have identical expected output
on baseline and current code. The invalid shared-sibling baseline result is not
used as an equivalent-work performance comparison.

Measurements use two warmups and seven alternating samples per side/workload.
CPU is user+system time; peak RSS includes compilation and the retained runtime.
Nearest-rank p95 at seven samples is the maximum sample, not a worst-case bound.
No tests, builds or Graphify run concurrently with timing.

| Workload | Median CPU before -> after, s | p95 CPU before -> after, s | Median RSS before -> after, KiB |
| --- | --- | --- | --- |
| counter | 1.62 -> 1.81 | 1.67 -> 1.86 | 36,544 -> 36,664 |
| creation | 1.01 -> 1.13 | 1.03 -> 1.17 | 36,412 -> 36,628 |
| numeric | 1.14 -> 1.15 | 1.18 -> 1.16 | 36,612 -> 36,748 |

Shared mutation and closure creation cost about **11.7% / 11.9% more CPU** in
these whole-process workloads; the numeric control is +0.9% with variation.
Median RSS is about +0.3% to +0.6%. These are correctness costs, not a general
speedup or RAM saving. An initial scratch-based mutation implementation measured
1.93 s CPU versus 1.81 s after holding the assignment address on the operand
stack; it was not retained. The remaining cost is not eliminated.

### Native collection-assignment stack repair

During cross-backend testing, a separate pre-existing translator bug surfaced.
Self-hosted `SET_INDEX` and `SET_ATTR` consume the assigned value, while native
setters leave that value on the stack. The PBLang translator now emits a native
pop after either setter. Previously `10 + { a[0] = 7; 2 }` could compute 9 instead
of 12, and repeated assignments retained obsolete values until frame exit.
No Rust implementation change is needed.

To reproduce, generate before/after artifacts with
`bench/selfhost_assignment_artifact.sapp --allow-all -- indexed OUTPUT.pbck`
or mode `attribute`, then time `pblang run OUTPUT.pbck --allow-all` separately
from compilation. Each artifact performs 20,000 assignments of a fresh 1 KiB
string and prints 1024. Both revisions must produce that same output. Two
warmups and seven alternating samples per side/mode:

| Workload | Median CPU before -> after, s | p95 CPU before -> after, s | Median RSS before -> after, KiB |
| --- | --- | --- | --- |
| indexed | 0.02 -> 0.01 | 0.03 -> 0.01 | 30,928 -> 8,468 |
| attribute | 0.03 -> 0.01 | 0.04 -> 0.01 | 31,056 -> 8,596 |

Peak RSS fell **72.6% / 72.3%** for these specific assignment workloads. The CPU
samples are only 0.01-0.04 s and have 0.01 s resolution; they do not support a
precise general speedup claim. Each repaired artifact is nine bytes larger
(one native pop plus its span), trading tiny static size for correct stack
behavior. This does not establish live-streaming or whole-product savings.
[Native execution samples and artifact sizes](evidence/selfhost-captures-native-cost-2026-09-22.json).


[All measurement samples](evidence/selfhost-captures-cost-2026-09-22.json).

## Product and platform limits

The configured installed PB-MCP desktop bundle was inspected read-only: it has
`resource_scope.sapp`, but lacks `compiler/codegen.sapp`, `compiler/vm.sapp` and
`selfhost_pbck.sapp`. This compiler change has no demonstrated live-view benefit;
the bundle was neither replaced nor restarted, and approvals were preserved.

Large collector pauses, broader runtime/FFI/polyglot integration, physical-device
and guest-OS validation remain open. The earlier codec's memory overhead is not
removed by this change. The project remains alpha; the
[stable-release gates](STABLE_1_0_READINESS.md) still apply. No Specisoft server was
used. The pre-existing 212-file change baseline was preserved.

A diagnostic probe that printed the entire cyclic VM globals map recursed in
the retained host formatter and aborted. Subsequent checks inspect scalar output
only; cycle-safe diagnostic formatting is a separate unresolved runtime issue.
No new crash artifact was found in the checked diagnostic locations.

Graphify update exceeded its 35-second bound and was terminated. The graph
query did not provide semantic coverage of the PBLang implementation.
