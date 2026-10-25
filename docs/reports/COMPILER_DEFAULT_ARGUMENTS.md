# Omitted arguments and explicit nil

The PBLang-written bytecode compiler previously filled default parameters inside
function bodies by replacing nil. Thus `fn f(x=7){return x;}` returned 7 for both
`f()` and `f(nil)`. Explicit nil was incorrectly replaced. The retained source
runtime and tree interpreter distinguish the two calls.

Newly compiled named functions carry literal `defaults` alongside `arity` and
`min_arity`. Call normalization fills only positions absent from the argument
list, before captured parameter cells are constructed. Explicit nil, false, zero
and empty strings remain unchanged. The VM's direct/general closure, raw
function and operator-overload call paths share this rule. Existing minimum and
maximum arity errors remain in force. Named functions captured through the cell
factory retain their default metadata.

The native PBCK translator writes the same literals into the existing native
function ABI. It no longer pads new functions with nil and relies on body code
to replace it. This is PBLang-only compiler/runtime logic; the retained binary
and native PBCK version 1 format are unchanged.

## Bytecode compatibility

PBCS version **4** adds an optional defaults table to each function constant,
after the version-3 optional minimum-arity field and before the upvalue table.
It consists of a presence byte (0 or 1), then, when present, a u32 item count and
ordinary scalar value encodings. The count must equal `arity - min_arity`, with
minimum arity present. Only number/string/bool/nil literals are supported.
Defaults contribute to the existing aggregate constant/string budgets; malformed
flags/counts, truncated tables, unsupported values and excessive sizes fail.

Readers still accept PBCS v1, v2 and v3. Missing legacy defaults stay absent:
legacy functions retain their old body-fill behavior. Re-serializing old bytecode
does not repair its semantics. Recompile source to obtain the corrected behavior.
Older PBCS readers reject v4, so distribute the updated reader with new artifacts.
No defaults are inferred from executable instruction sequences. Raw in-memory
chunks remain trusted compiler inputs rather than a new sandbox boundary.

## Example and validation

`examples/default_arguments/main.sapp` prints `automatic`, `nil`, `false`.
Compile it with `tools/pbc.sapp --target pbck` and execute the resulting artifact
with the retained runtime, or use the PBLang VM through the frontend.

The focused suites are `selfhost_default_values_test.sapp` and
`native_default_values_test.sapp`. They cover omitted and explicit values,
trailing defaults, captured parameters, returned functions, try/catch,
argument side effects, raw/operator call paths, optimized chunks and PBCS
round-trips. The native suite executes each scenario through plain, optimized
and serialized/restored compilation. Existing PBCS arity/hardening,
try/capture and native method suites remain regression coverage.

## Exact retained nested p:run boundary

The separate source program below still prints 9 on the retained executable:

```pblang
for _i 1 {
    let p=map{run:fn(self){return 7;}};
    print (try {p:run()} catch 9);
}
```

The retained parser lowers colon calls into a block expression
(`crates/pblang-core/src/parser.rs`, `Postfix::Method`). Its Rust compiler then
lowers block expressions into immediate function literals
(`crates/pblang-compiler/src/lib.rs`, `ExprKind::BlockExpr`). The function-literal
compiler copies only one enclosing compiler level and sets that snapshot's
`enclosing` to `None`; deeper synthetic closures lose the outer local binding.
The named-function path copies the current compiler and at most one additional
enclosing level, whose `enclosing` is also set to `None`; it does not preserve an
arbitrary-depth chain. That path is not used by those generated literals.
Changing the retained compilation paths needs a Rust compiler change/rebuild,
which is outside this PBLang-only task.

The existing explicit PBLang-to-PBCK route producing 7 is regression-tested,
but is not a new fix for direct retained execution. No automatic reroute,
source rewrite, global installation change or permission change is introduced.
This default-argument fix does not close the nested p:run, general lazy-import
origin, FFI or garbage-collection backlog. Native execution tests qualify Linux;
no universal operating-system or performance claim is made.

The installed public checker can now expose the existing conservative admission
profile directly: `pblang --check --via-pblang --retained-source FILE`. Its early
rejection does not repair direct execution. See
[public retained-source checking](PUBLIC_RETAINED_SOURCE_CHECK.md).
