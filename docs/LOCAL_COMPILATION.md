# Local developer source admission

PBLang is a general-purpose language. The size of a valid local program should
not be confused with a service's budget for inspecting untrusted input. Local
developer admission therefore has no preset source-byte or token-count ceiling.
It still rejects malformed numbers, unterminated strings/comments and every
parser diagnostic before emitting or evaluating a recovered AST.

This is an explicit source policy, not a sandbox or a guarantee that a machine
has enough memory. Parsing still materializes the source character array, tokens
and AST. Existing parser nesting guards, backend subsets, allocation limits and
runtime behavior remain. Separate compilation by module is still future work;
this slice supports larger individual or already assembled source inputs.

## Interpreter CLI

```sh
pblang run tools/pbi.sapp --allow-all -- app.sapp
pblang run tools/pbi.sapp --allow-all -- app.sapp --max-source-bytes=8388608 --max-tokens=500000
```

The first command uses local developer admission. The second applies the exact
budgets requested by the developer. Both budgets are optional positive decimal
integers. Unknown, duplicate, zero, fractional or malformed options reject;
flags use `--name=value`. Byte counts use UTF-8; tokens include comments and EOF.
The CLI reads and decodes the file before applying the source budget, so that
budget does not bound the initial file-read allocation. There is no new implicit
execution timeout. Use external supervision where the application requires it.

## PBLang library entry points

```pblang
let I = import("stdlib:compiler/interp.sapp");
let ast = I.compile_local_program(source);
I.eval_ast(ast);
I.eval_local_program(source, map{max_source_bytes: 8388608, max_tokens: 500000});

let C = import("stdlib:compiler/ctranspile.sapp");
let built = C.build_program(source, output, map{
    admission: "local", profile: "portable",
    source_budget: map{max_source_bytes: 8388608, max_tokens: 500000}
});
```

C99 local admission is explicit through `admission: "local"`; ordinary
`build_program` remains strict for compatibility. Pure translation is available
as `transpile_program_local(source, budget=nil, numeric="float")`. Integer mode
is explicit because arithmetic semantics differ. Invalid source or exhausted
budgets reject before compiler discovery or output writes, preserving old outputs.
A build report identifies the selected policy in `source_admission`.
`run_program(source, options)` forwards the same admission and source-budget
options. Its existing execution watchdog remains a separate control; selecting
local source admission does not disable the runner's execution timeout. That
watchdog covers the generated program, not transpilation or the C compiler process.

The budget map accepts only `max_source_bytes` and `max_tokens`. Missing or nil
values mean no requested ceiling. Values must be positive, finite, exact integers
within the runtime's exact-count range; they are not clamped to the strict source
profile. The caller's map is not changed. Token limits are enforced during token
production, before allocating the next token, rather than after full tokenization.

Use the separate checked module APIs for project snapshots. Local admission plus
`modules` rejects explicitly; it does not bypass per-file syntax checks by joining
unchecked files. `source_budget` without local admission also rejects instead of
silently ignoring a caller's requested budget.

## WebAssembly local compilation

```pblang
let W = import("stdlib:codegen_wasm.sapp");
let bytes = W.compile_wasm_local(source);
let limited = W.compile_wasm_local(source, map{max_tokens: 500000});
```

```sh
pblang run tools/pbc.sapp --allow-all -- app.sapp --out app.wasm --target wasm --local-source
pblang run tools/pbc.sapp --allow-all -- app.sapp --out app.wasm --target wasm --local-source --max-source-bytes=8388608 --max-tokens=500000
```

Without `--local-source`, `pbc` and `compile_wasm` keep strict admission.
Local admission is currently supported only for the WASM target in `pbc`.
Budgets without local mode, invalid or duplicate budget options, duplicate
source/output/target arguments, missing option values and unknown flags fail
before the source is read. A path starting with `-` can be written with `./`.
The CLI still reads the entire source before enforcing an optional byte budget.
Compilation must finish successfully before the output file is replaced.

Both APIs share the same private emitter. Static string data determines initial
linear-memory size after all literals have been emitted. The existing minimum
is 16 pages; larger data receives enough additional pages to fit its aligned
heap base. Small programs retain their previous memory declaration. The memory32
address and string-header representation is checked, including signed encoding
of high-bit heap-base constants. The architectural memory32 ceiling is distinct
from source size; see the [WebAssembly memory limits](https://webassembly.github.io/spec/js-api/index.html#limits).
The allocator still does not grow memory automatically at runtime: fitting static
data does not establish sufficient capacity for later concatenations/allocations.

String-type inference now runs to convergence. Its fixed set of return/parameter
slots can each move only from numeric to string, so every changing round consumes
one of a finite number of transitions. The former silent 200-round cutoff could
miss string-returning functions in long call chains. A 210-function chain is
checked through both admission APIs and executed in Node/V8.

Tests execute source above both former direct caps, instantiate static data
exactly at and just over the 16-page boundary, and exercise CLI budget rejection
with old-output preservation. Node/V8 is required for these execution regressions;
absence or failure is not reported as success. No guest operating system is involved.

## Other routes and limits

The ARM64 backend now encodes large frames and distant frame slots correctly,
and rejects unrepresentable compact immediates/branches. Its source-admission
policy is unchanged. See [ARM64 encoding and measured limits](ARM64_ENCODING.md).

- `checked_source.parse_source`, `interp.compile_program`/`eval_program`, PBIDE
  and existing strict backend entry points keep their contracts.
- `checked_source.parse_local_source` and `tokenizer.tokenize_local` expose the
  developer policy explicitly. The strict bounded tokenizer is unchanged.
- `pbbuild`/smart build, native-x64 cache sessions, other backend routes and dependency/security scans
  have separate limits that this change does not widen. Full large-project
  support across those paths remains an explicit next priority.
- Never forward untrusted request options into local admission automatically.
  The profile is chosen by the developer, not selected because a strict check
  rejected the input. Local compilation can consume substantial machine resources.
- No claim is made that all backends support every language construct, or that
  the synthetic large-file checks establish production readiness or performance.

Regressions: `local_source_admission_test.sapp` checks strict/local separation,
UTF-8 and EOF boundaries, invalid budgets and source rejection;
`local_source_live_test.sapp` runs a source above 1 MiB and 65,536 tokens through
pbi and a real C99 artifact, verifies optional budgets and preserved old outputs,
and rejects a malformed suffix without executing its preceding print.
