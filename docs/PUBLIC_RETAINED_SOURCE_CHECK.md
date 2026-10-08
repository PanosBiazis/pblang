# Public retained-source compatibility preflight

The retained source compiler can lose a loop-local receiver while lowering
nested synthetic closures. The checked regression prints **9**, although its
method returns **7**:

```pblang
for _i 1 {
    let p = map{run: fn(self) { return 7; }};
    print (try { p:run() } catch 9);
}
```

The qualified Linux public entry now exposes an explicit preflight:

```sh
pblang --check --via-pblang --retained-source examples/retained_nested_method.sapp
```

It exits 1 with `retained-nested-try`, explains that the source was not executed,
and points to the explicit PBCK route. The profile first reuses
`compiler/retained_source_check.sapp`, then performs the ordinary structured
source check if admission succeeds. Semantic errors still fail; admission alone
is not sufficient for a successful result. No regex search, source rewriting,
implicit execution, permission grant or native compiler change is introduced.

## Exact contract

- Select `--check` and `--via-pblang` (or `--self-host`) first; capability flags
  may precede them. Supply one `--retained-source` before `--` and one filename.
- The profile conservatively rejects any nested try inside a block/function,
  including some programs that happen to execute correctly. It also rejects
  imports and aliases of `import`: dependencies are not recursively qualified.
- Strings/comments resembling method calls, `try` or imports do not trigger
  source scanning. The existing checked AST walker applies its byte/token,
  depth and node limits; parse/admission failures remain nonzero.
- Admission errors report fixed codes/messages, not private source literals.
  Ordinary semantic diagnostics retain their existing output contract.
- Unknown/repeated/misplaced options exit 2 before source access. Literal
  `--retained-source` filenames work after `--`, including Unicode/newline paths.
- Caller filesystem/import permissions and the locked sandbox remain enforced.
  Exec/FFI/network are not needed for the ordinary preflight.
- `PBLANG_CHECK_PROFILE` still selects the older `check` command profile. This
  public argv route uses only its explicit flag; environment text cannot select
  this profile or inject arguments.
- Combine with `--bounded-source=BYTES` for pre-allocation input admission, using
  that profile's [documented explicit FFI grants](PUBLIC_SELFHOST_CHECK.md#opt-in-bounded-source-admission).
  Without it, reading remains the existing whole-file operation.

## Explicit PBLang-to-native-PBCK alternative

For the supported compiler subset, keep source and output separate. `RELEASE`
means the installed package directory, not the caller's project directory:

```sh
pblang run "$RELEASE/tools/pbc.sapp" -- \
  examples/retained_nested_method.sapp --target pbck --out /tmp/nested-method.pbck
pblang run /tmp/nested-method.pbck --sandbox
# Expected: 7
```

Compilation uses the PBLang frontend and PBCK translator; execution uses the
unchanged retained VM with separately selected runtime permissions. Compilation
rejects unsupported sources; plain compilation is not recursive import migration.
Use an explicitly qualified static bundle when dependencies need compilation.
Choose a fresh output path: successful compilation writes the requested output.

The direct explicit spelling `p.run(p)` produces 7 in this narrow plain-map
fixture, but it is not a general automatic rewrite for `receiver:method()`.
Colon dispatch can select `__class`, and complex receivers/arguments can have
effects. Rewriting can change dispatch or evaluation order.

## Why default execution is unchanged

### Compiler boundary behind the nested-method case

In the native reference source, `crates/pblang-core/src/parser.rs` lowers a colon
method into a value block that evaluates the receiver once, selects `__class`
when present and calls the selected method with the original receiver. In
`crates/pblang-compiler/src/lib.rs`, `ExprKind::BlockExpr` becomes a synthetic
zero-argument function. The `ExprKind::FnLit` compiler constructs an enclosing
snapshot with `enclosing: None`; `resolve_upvalue` follows only the enclosing
chain it receives. Nested synthetic functions can therefore lose a binding from
a more distant loop/function frame. This source-level finding is consistent
with the retained regression; it is not proof of the provenance of every
distributed native binary.

A fresh retained PBCK decode of the fixture above makes this boundary visible:
the colon form's nested synthetic function loads global `p` with empty captures;
the explicit form captures local slot3 and reads upvalue0. Removing the outer
catch exposes `undefined global 'p'` before the method runs. A receiver-mutation
control leaves6 after the colon failure but reaches7 through the explicit call;
an explicit throwing method exposes42 whereas the nested colon form fails before
that method. The no-exception control still returns7, and an extra loop retains
the caught9. Native-emitted PBCK preserves the faulty binding; PBLang-emitted
PBCK is a different compilation route and returns7. These observations identify
the broken capture, not a repaired direct-source route.

Default source execution in `src/main.rs` calls that native parser/compiler.
Editing the PBLang frontend or VM cannot replace the compiler already embedded
in the retained executable. The supported explicit PBLang-to-PBCK path above
avoids that lowering for its supported subset and already passes the fixture.
It remains an alternative route, not a retained-native repair. A complete native
repair requires work on the native compiler and verification of its rebuilt
binary; that lies outside a PBLang-only production change.

An automatic switch to the self-hosted VM or PBCK would change supported syntax,
imports, builtins, argument/host context and error behavior. A mandatory pre-read
would add filesystem/import requirements to native source loading, and the
conservative gate would reject previously supported programs. Checking then
reopening a path for execution also permits the file to change between stages.
This preflight does not create an atomic check-and-execute boundary.

Default `run`, bare scripts, direct retained-native execution and ordinary checks
therefore keep their previous semantics, including this known defect. A passing
preflight is not universal compiler-correctness certification. Linux tests do
not qualify other platforms, all programs, or retained import/FFI/GC behavior.

`tests/tooling/public_retained_source_check.py` reproduces public/native 9,
explicit-call 7, PBCK 7, fixed preflight rejection, ordinary semantics, permissions,
argument boundaries, bounded composition and source-hidden installed origins.
