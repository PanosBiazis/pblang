# Checked executable source admission

`stdlib:compiler/checked_source.sapp` provides `parse_source(source,
max_source_bytes=1048576, max_tokens=65536)`. It returns an AST only after
bounded tokenization and parsing finish without lexical or syntax errors.
Both optional limits must be finite positive integers and may only lower the
hard defaults. Non-text sources and invalid limits fail before tokenization.

Source size is measured in UTF-8 bytes. A preliminary scalar-count check rejects
obviously oversized text; byte encoding is needed only when four bytes per scalar
could exceed the selected limit. Token accounting includes comments and the
reserved EOF token. Invalid numeric tokens and unterminated strings/comments
are rejected before the parser can discard or recover them. Parser exceptions,
invalid result shapes and nonempty diagnostic arrays also reject admission.
Errors use fixed text and do not copy tokens, source or parser diagnostics.
This guarantee concerns the admission error message. The retained runtime's
uncaught-error reporter separately includes caller code and local path context;
it is not a private logging boundary and can misattribute imported-code locations.

`codegen_wasm.compile_wasm`, `codegen_arm64_ast.compile_ast` and its `compile_elf`
wrapper use the hard defaults before allocating output buffers. Previously,
recovered syntax such as `fn main(){return 7;`, `fn f(x=){return 1;} return f(2);`
or a valid return followed by an unfinished comment could still produce bytes.
These sources now throw before generation. Valid optional semicolons, completed
comments/strings, declarations and backend-supported programs retain their grammar.
Raw parser recovery remains available to editors and other existing consumers.
The ARM64 backend's internal function-node parameter also no longer uses the
reserved word `fn`, allowing its own source to pass the self-hosted checker.

The `execution_pipeline.build_source(source, "wasm32", options)` facade catches
backend rejection and returns `ok=false`, the usual format/target fields,
`error="WASM compilation rejected source"`, and an empty diagnostics array.
It includes neither `artifact` nor `artifact_bytes`. This covers admission
errors and backend-specific unsupported constructs. A larger facade
`max_source_bytes` option cannot widen the WASM backend's hard cap. The facade's
own argument/source/artifact-limit exceptions retain their existing contract.
The `pbc` CLI writes its destination only after successful compilation. A failed
build leaves any older destination file intact; callers must honor failure and
must not treat a pre-existing file as the result of the rejected build.

## Scope and remaining limits

The direct x86-64 module `stdlib/native/codegen_x64.sapp` now uses the same
admission helper for `compile_expr_exit`, `compile_program`, `compile_fns` and,
through `compile_fns`, `compile_safe`. All user-source checks precede output
allocation. The fixed internal prelude helpers retain their existing parser;
their text is a private hard-coded map, not supplied source. `compile_safe` does
not return a certificate on failed source admission. Its legacy `safe` field is deprecated; explicit
`memory_safety_certified=false`, `runtime_isolated=false`, a limited scope and
limitations now accompany the result. No field authorizes hostile-code execution.

`native/compile_session.sapp` checks source before cache keys, reads, hit/miss
counters, compilation or publication. Batch admission checks every job before
processing any job, preserving old output files when a later job is invalid.
The cached benchmark driver follows the same rule. Both include the helper in
their default dependency fingerprints. A seeded invalid-source cache entry
therefore cannot bypass admission. Cold builds currently parse source again in
the backend; warm builds now pay the required admission cost. No speedup is
claimed for this change.
The local benchmark driver still reads its source file before applying the
in-memory cap. Its caller must supply a bounded trusted local file; this change
does not bound the initial file-read allocation.

This deliberately lowers the session/driver's former 16-MiB source ceiling to
1 MiB UTF-8 and adds the 65,536-token ceiling to all direct x64 source routes.
Sessions retain their existing fixed source-size rejection; other admission
failures return `error="x86-64 source admission failed"`. The artifact cache's
standalone storage/key API is unchanged and does not itself validate source.
The cache remains a private local performance cache trusted at the same-user
boundary: matching metadata, digest and ELF shape do not authenticate machine
code against a writer who can replace both metadata and artifact.

## C99, interpreter and checked module linking

All seven C99 source profiles (function-only, hosted float/integer,
freestanding float/integer and kernel float/integer) now use the same direct
1 MiB UTF-8 / 65,536-token admission. `compiler/interp.compile_program` checks
before preparing its reusable AST; `eval_program` checks before evaluation.
A rejected suffix cannot allow an earlier `print` to execute. C wrappers retain
fixed profile-specific rejection messages and restore numeric/hosted mode.
Invalid NaN limit comparisons in the retained runtime are normalized to the
fixed admission-limit error rather than exposing an internal comparison error.

`source_inliner.link_modules_checked` adds an explicit compiler linking path.
It validates the raw main and every reachable module before import discovery
or transformation. Unreachable files still receive the generic snapshot shape,
path and byte checks, but do not have to parse. Checking only flattened text is
insufficient: concatenation can close an earlier file's unfinished comment,
string or function block. Generic `link_modules` and C99 `link_program` retain
their source-linking compatibility contract; they are not source-admission APIs.

`ctranspile.transpile_program_linked` and builds with `options.modules` force
checked linking, then check the flattened text before lowering. The separate
`checked_source.parse_linked_source(source, max_source_bytes=16777216,
max_tokens=1048576)` profile accepts lower-only finite positive integer caps.
Raw files also obey the linker's per-file byte limit. The C linked-output cap
cannot exceed 16 MiB; a larger requested cap fails before transformation. The
generic linker's optional 128 MiB ceiling is unchanged. Each reachable file and
the final output have their own token cap; this is not an aggregate project
work budget. Generated C retains existing output/memory limitations.

`build_program` translates successfully before compiler discovery or output
writes. Rejection preserves older output/C files and creates no new artifact.
`run_program` validates its timeout, including NaN, before any build. Compiler
execution authority, watchdog availability and runtime isolation are separate
boundaries and have not been redesigned here.

The new direct C cap deliberately also constrains callers supplying already
flattened text, including `dist` publication and the integer benchmark driver,
even when their outer byte checks allow more. Their effective C source cap is
now 1 MiB / 65,536 tokens. Use the explicit in-memory module build API for the
larger checked project profile; merely flattening externally loses raw-file
syntax provenance. No raw-AST API (`eval_ast`, C expression/statement emitters)
is made safe for arbitrary caller-crafted or mutated ASTs by this change.
`tools/pbi.sapp` still reads its input file before in-memory admission; the
initial file-read allocation is not bounded by the new parser gate.
The native-selfhost benchmark now bundles the helper, but current direct-x64
compilation still rejects the existing missing `type` builtin lowering. Native
self-hosted interpreter parity is not established by the retained-host tests.

This is lexical/syntax admission, not complete semantic validation or universal
compiler parity. Backend-specific type, numeric, ABI and language-subset behavior
remain separate. Editor recovery is unchanged. Source/token ceilings do not
establish peak heap, nesting, wall-clock, native isolation or an aggregate
runtime budget. Generated ARM64 tested in an emulator is not guest-OS validation.

## Regressions

- `tests/semantics/checked_source_test.sapp`: inclusive byte/token limits, UTF-8,
  malformed limits and input, lexical/syntax rejection and recovery compatibility.
- `tests/semantics/backend_source_admission_test.sapp`: direct WASM/ARM64/ELF
  rejection, hard caps, artifact-free pipeline failures and valid controls.
- `tests/semantics/backend_source_diagnostics_live_test.sapp`: Linux child-process
  positive controls, actual fixed diagnostics and synthetic source non-disclosure.
  Set `PBLANG_BIN` when testing with a retained binary outside the checkout.
- `tests/semantics/x64_source_admission_test.sapp`: all four x64 source routes,
  inclusive byte bounds, UTF-8/token rejection, valid programs and prelude helpers.
- `tests/semantics/x64_cache_admission_test.sapp`: readable seeded invalid-source
  cache entries rejected before counters/effects, old outputs and entries intact,
  valid cache reuse, and the actual cached benchmark driver.
- `c_interp_source_admission_test.sapp`: every C/interpreter source route,
  malformed and recovered input, inclusive bytes, UTF-8/tokens and mode recovery.
- `c99_checked_link_test.sapp`: cross-file healing, reachable-only admission,
  lower/hard caps, and linked projects above the direct byte ceiling.
- `c99_admission_effects_test.sapp`: positive-control local compiler probe,
  rejection before external effects, old-output preservation and invalid timeouts.
- `c_interp_diagnostics_live_test.sapp`: actual fixed child errors and no early
  interpreter output from malformed source.
- Existing WASM Node/V8, ARM64 execution, pipeline, frontend, PBIDE and emulator
  suites check compatibility. Node/V8 needs virtual address space for its WASM
  reservation; this is distinct from physical memory use.

Validation for the earlier WASM/ARM64 change passed 48 suites, including 17 emulator suites and
real Node/V8 execution. After the ARM64 parameter rename, all four affected
admission/ARM64 suites passed again. The retained-bootstrap project build passed;
the self-hosted checker reported zero errors/warnings across eight touched source
files, and their mandatory security scans passed. Native lint completed with
style/conservative unused-value warnings. No guest operating system was tested.
Graphify extraction ran but its update did not finish within the 45-second bound;
this change makes no graph-freshness claim.
