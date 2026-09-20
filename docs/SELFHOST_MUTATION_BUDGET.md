# Collection growth in explicitly bounded VM execution

`run_bounded` and `frontend_run_bounded` now check collection growth before
`SET_INDEX` and `SET_ATTR` write to a caller-visible array or map. Previously,
these instructions consumed their target from the stack; a later runtime tick
could never inspect it again. A two-element array could acquire a third element
and execution could return successfully with `max_value_items: 2`.

The guard is PBLang code in `stdlib/compiler/vm.sapp`. It adds no Rust or foreign
block. The actual array/map write still uses the retained host's implementation.

## Contract and example

```pblang
let FE=import("stdlib:compiler/selfhost_frontend.sapp");
// Rejected before a third element is written.
let denied=try FE.frontend_run_bounded(
    "let a=[1,2];a[2]=3;",map{max_value_items:2}
) catch nil;
// Replacing an existing element at the limit remains valid.
let allowed=FE.frontend_run_bounded(
    "let a=[1,2];a[1]=9;print a[1];",map{max_value_items:2}
);
// Ordinary execution gains no collection-size or elapsed-time limit.
let ordinary=FE.frontend_run("let a=[1,2];a[2]=3;print a[2];");
```

Array admission follows current host semantics: a numeric index truncates toward
zero; only the index equal to the length appends. Thus `2`, `2.5` and `2.999` all
append to a two-element array and are rejected at a two-element budget. Replacing
an existing index remains valid. Invalid index types and out-of-range indices
continue to reach the host's validation without mutation.

Maps admit replacement of an existing string key, but reject a new key when at
the budget. Removing a key permits another addition. Both indexed writes and
attribute writes follow this rule. A refusal sets the existing sticky runtime
failure state; guest try/catch cannot clear it and continue execution. Over-budget
growth reports the budget refusal before any later host write validation.

`max_value_items` is a per-value item count, not a hard bound on aggregate RAM.
It does not account for all nested values, shared references, compiler state,
host allocations, GPU memory or device resources. This change does not repair
every mutation path or make the VM a complete hostile-code sandbox. Existing
host freeze/borrow checks remain in the write operation.

## Validation

`tests/semantics/selfhost_vm_mutation_budget_test.sapp` covers final-instruction
writes with externally retained aliases, fractional indices, exact limits,
below-limit growth, replacements, removal/reinsertion, map attributes/indexing,
invalid indices/keys, nested aliases, ordinary execution and sticky failures.
The standalone reproduction changed from successful execution with length three
to rejection with length two.

24 PBLang suites pass, including frontend/codegen/optimizer/VM, ownership,
Unicode and five emulator suites. The changed VM also passes the self-hosted
frontend check. Two PB-MCP integration suites pass: document chunking and 192
service projection/core comparisons plus seven rejection cases. An initial
service test failure reproduced on baseline too: its dynamically imported
fixture module was missing from staging. Supplying that module fixed both runs.

The isolated project builds and publishes repository-relative lock paths with
the PBLang portable-lock library. Lint exits successfully, with style/unused-variable
warnings (including the nested map admission condition); this is not a zero-warning
claim. Fixtures are synthetic; no credentials, desktop control, provider network
requests or Specisoft servers are involved. These are local Linux/VM/emulator
checks, not evidence for real guest operating systems or universal compatibility.

## Resource measurement

`bench/selfhost_mutation_control.sapp` exports `run(bounded)`. Import it and call
`run(false)` or `run(true)` from a small driver. Each mode compiles the same source
once, then runs four iterations of 10,000 array and map replacements, checking
the exact output `29997` each time. The bounded mode uses `max_value_items: 8`
and `max_steps: 2000000`. Inputs, outputs and iteration counts are unchanged.

The baseline is `95bd09130ba7f30f0479855411bc51f89cd5c17d`, with only the VM restored
to that version in an otherwise matching isolated tree. Measurements use fresh
Linux x86_64 processes, two warmups and seven retained samples per version per
mode, alternating order. Startup and compilation are included. OS effects are
denied except imports. Raw samples, source/runtime hashes, exact workload,
medians and nearest-rank p95 are under
`bench/results/2026-09-20-selfhost-mutation/`.

| Mode / metric | Baseline median | Candidate median | Baseline p95 | Candidate p95 |
| --- | ---: | ---: | ---: | ---: |
| Ordinary CPU ms | 1246.300 | 1181.756 | 1307.526 | 1280.662 |
| Ordinary wall ms | 1247.042 | 1183.017 | 1327.740 | 1281.687 |
| Ordinary peak RSS KiB | 24304 | 24272 | 24480 | 24412 |
| Bounded CPU ms | 3532.214 | 3693.080 | 3991.003 | 6811.186 |
| Bounded wall ms | 3534.903 | 3696.180 | 4013.920 | 6814.463 |
| Bounded peak RSS KiB | 24308 | 24336 | 24468 | 24468 |

Ordinary median CPU was 5.2% lower and bounded median CPU 4.6% higher; peak RSS
was nearly unchanged (-0.13% / +0.12%). Timing variation, particularly the bounded
tail sample, prevents a general speedup or zero-overhead claim. The additional
admission work is a correctness cost. No disk, GPU/NPU or peripheral improvement
is inferred from this CPU workload.

## Remaining work

General error-preserving cleanup still needs a host-supported way to retain and
rethrow the original error across cleanup. Composite/shared ownership, aggregate
memory accounting, full native/FFI and backend parity remain open. PB-MCP still
has polyglot/HTTPS boundaries, unported Rust/AI functionality, the common desktop
broker and wider platform verification outstanding. This change neither disables
those boundaries nor claims completion of PBLang 1.0.0 or PB-MCP.
