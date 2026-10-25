# Self-hosted expression blocks and catch assignments

Update 2026-09-22: [shared capture lowering](SELFHOST_SHARED_CAPTURES.md) now
addresses the scalar-copy limitation described in this historical report.

The self-hosted compiler now emits eligible expression blocks directly in their
enclosing frame. For example, this program prints `true` with the retained
runner and the self-hosted frontend:

```pblang
fn sample() {
    let caught=false;
    let value=try {throw "synthetic";} catch {caught=true;};
    return caught;
}
print sample();
```

Before this change, the self-hosted frontend printed `false`. The parser
represented each expression block as an immediately called function. The
self-hosted VM copies scalar captures, so assigning the hidden function's copy
did not update the enclosing variable. The parser now marks its synthetic calls;
code generation gives eligible blocks a lexical scope in the current frame.
The block still produces its tail value, or nil when no tail is present.
Existing scope and exception cleanup release block-local references without
discarding the returned value.

## Compatibility boundaries

Explicit `return`, `break` and `continue` retain the old function boundary.
Blocks containing user functions also retain it: native reference captures in
repeated blocks must have distinct environments when closures escape. A test
creates two closures in a loop and requires their results to remain `[1,2]`.
Nested synthetic blocks are inspected recursively; their generated tail return
does not count as an explicit return from user source.

This is not a rewrite of general closure capture. Scalar mutation through
user-written closures, or through blocks that require the compatibility path,
can still differ between the retained and self-hosted runtimes. The shared
fixture also avoids a separate retained-compiler transitive-capture failure;
nested throwing handlers that exercise that failure are tested on the
self-hosted frontend only.

No bytecode opcode, container version, runtime permission or source-size limit
was added. Existing resource budgets remain enforced. Fewer call frames and
instructions, and potentially more locals in the caller, can change precisely
where an explicitly bounded execution exhausts its budget. The AST scan runs
during compilation, not on each VM iteration. Existing parser depth protection
is unchanged.

## Validation

The shared fixture first reproduced the scalar-assignment failure against
baseline `53722e7`. The changed compiler passes retained source execution,
plain/bounded self-hosted VM execution, serialized self-hosted bytecode,
optimized frontend execution and both ordinary and optimized native PBCK.

[The contract](../tests/semantics/selfhost_inline_blocks_test.sapp) checks
assignments, lexical shadowing, tails including nil/false/zero, nested throwing
handlers, repeated failures, escaping closures and aliases, failed-block local
cleanup, malformed source rejection and absence of hidden call/closure opcodes
for a simple block. The [shared fixture](../tests/fixtures/selfhost_inline_blocks.sapp)
checks explicit control-flow fallback as well.

[Validation inventory](evidence/selfhost-inline-blocks-validation-2026-09-22.json):
57 suites passed, including 17 emulator suites, parser-depth/drift, invalid AST,
bounded execution, lifetime/exception cleanup, PBCK and optimizer regressions.
Build and portable lock publication passed in the isolated candidate. Retained
lint and self-hosted source checks passed for all five touched source files.
Lint retains existing compiler style advisories and intentional fixture
shadowing/constant-condition advisories; it reports no errors.

The focused review covered lexical isolation, escaping capture lifetime, stack
result preservation, failure cleanup and fallback control flow. It adds no
thread synchronization or new external effects and does not establish general
TOCTOU, FFI or concurrent-memory safety.

## Measurements

Compare isolated baseline `53722e7` and candidate trees with identical retained
executable and benchmark source. Build and publish portable locks before timing.
Run [the benchmark](../bench/selfhost_inline_blocks.sapp) with each argument pair:

```
pblang run bench/selfhost_inline_blocks.sapp -- plain success
pblang run bench/selfhost_inline_blocks.sapp -- plain failed
pblang run bench/selfhost_inline_blocks.sapp -- bounded failed
pblang run bench/selfhost_resource_cost.sapp -- plain numeric
```

Each block workload executes 1,000 calls with asserted identical output. The
numeric control uses the existing unchanged workload. Two warmups and seven
measured processes per variant alternate order. GNU time measures process CPU
(user plus system), wall time and peak RSS. Nearest-rank p95 is the largest of
seven observations, not an estimate of a guaranteed worst-case pause. No tests,
builds or Graphify ran concurrently with the measurements.

[Raw samples](evidence/selfhost-inline-blocks-cost-2026-09-22.json):

| Workload | CPU median / p95 before, s | CPU median / p95 after, s | Peak RSS median before / after, KiB |
| --- | ---: | ---: | ---: |
| blocks_success | 0.93 / 1.26 | 0.90 / 1.02 | 35676 / 35816 |
| blocks_failed | 0.97 / 1.24 | 0.92 / 1.06 | 35688 / 35560 |
| blocks_bounded | 1.08 / 1.43 | 1.03 / 1.14 | 35708 / 35576 |
| numeric_control | 1.47 / 1.49 | 1.48 / 1.60 | 35704 / 35704 |

Block workloads show about 3–5% lower median process CPU here. The numeric
control shows about 0.7% higher median CPU and a higher observed p95. Peak RSS
medians vary by less than 0.4%; no general RAM saving is established. These
short process measurements include compiler startup and are not VM-only
timings, collection-pause measurements or proof of a universal speedup.

## Remaining integration

Long kernel collection pauses, wider SMP/platform validation, general runtime/
FFI ownership and polyglot integration remain open. This compiler change does
not add those modules to the installed PB-MCP desktop, which uses the retained
runtime and does not include this compiler. Its installation was not changed
or restarted in this slice. No live-streaming, GPU/NPU, disk-write, energy or
whole-system memory benefit is established by these process benchmarks.

Graphify AST refresh exceeded its 35-second bound; its owned process group
was terminated. It does not provide semantic `.sapp` coverage.
