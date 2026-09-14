# Tree-interpreter scope resolution

The PBLang-written evaluator in `stdlib/compiler/interp.sapp` searches the
current environment, then its captured parent chain. Declarations add a binding
when executed. Reads and assignments choose the nearest existing binding;
assigning an undefined name throws. A binding containing `nil` still exists.
This describes the tree interpreter's existing generic lookup contract, not a
claim of complete compatibility with every PBLang backend.

A closure captures its environment. A later declaration in that environment
can therefore change which binding the closure finds:

```pblang
let x = 1;
fn outer() {
    let read = fn() { return x; };
    print read(); // 1: finds the outer binding
    let x = 2;
    print read(); // 2: finds the newly declared, nearer binding
}
outer();
```

Previously, prepared identifier and assignment nodes reused the lexical depth
observed on their first execution. This could return or update the wrong
variable after a late declaration. Different closures sharing a function-body
AST could also poison one another's cached depth, depending on call order.

Preparation now counts every supported variable, constant, named function,
parameter and loop binding, saturating each count at two. Names declared more
than once use ordinary scope-chain lookup for both reads and writes. Only
names with at most one declaration retain depth caching. The analysis is
conservative across the whole program: unrelated functions using the same
parameter name can also select dynamic lookup. No performance improvement is
claimed. Statically unshadowed builtin dispatch remains available.

Private prepared-AST version 3 rebuilds earlier metadata before evaluation,
including version 2 nodes with previously resolved depths. Public parser node
kinds and the `compile_program`, `eval_ast`, and `eval_program` APIs are unchanged.
Caller-mutated ASTs carrying the current private version remain outside the
trusted reusable-AST contract; this change does not authenticate arbitrary ASTs.

## Validation and remaining differences

`interp_late_shadow_test.sapp` covers late variable/constant/function declarations,
reads, assignments, numeric fast paths, both closure warmup orders, repeated
AST execution, missing names and version migration. Existing lexical-cache tests
retain positive checks that unique local and captured names still cache correctly.
`interp_late_shadow_live_test.sapp` checks the actual `pbi` driver on Linux using
`tests/fixtures/interp_late_shadow_program.sapp` and a bounded local child.
Set `PBLANG_BIN` when the retained binary is outside the checkout.

The fixture prints `1, 3, 2, 4, 3` (one value per line) through the tree interpreter.
The retained bootstrap currently prints `1, 3, 3, 4, 4` for that same fixture.
This is an observed cross-runtime closure-binding discrepancy, not a passing
parity result. The bootstrap is unchanged. General native self-hosting remains
blocked by missing builtin lowering and value-representation differences.

## Literal defaults in named functions

Named declarations retain their parser-provided number, string, boolean and
nil defaults when the interpreter creates a closure. Defaults are decoded once
as literal values; they are never evaluated as expressions. Calls with omitted
positional arguments now use the corresponding defaults on every dispatch path
(zero, one, two, or more arguments). An explicitly supplied value, including
`nil`, always wins. For example, `fn f(x = 7) { return x; }` returns `7` for
`f()` and `nil` for `f(nil)`.

Missing required parameters retain the interpreter's existing `nil` behavior.
Extra arguments are still evaluated before being ignored. This is compatibility
with the current tree interpreter; it does not qualify every backend's arity
rules. Parameter type annotations are not enforced by this change. Anonymous
function defaults remain unsupported by the self-hosted parser, and computed
defaults still fail checked source admission. The existing negative-number
literal syntax remains accepted. Use body logic with a nil sentinel for complex
initialization.

The new defaults lane is part of the interpreter's array-based closure layout.
That layout is not opaque: interpreted array operations can inspect or mutate
closure internals. Neither literal-default support nor the trusted `eval_ast`
contract establishes capability isolation or protection against closure forgery.
Unsupported default AST kinds/shapes throw a fixed diagnostic without evaluating
the supplied expression. This is a narrow check, not general AST validation.

`interp_defaults_test.sapp` checks literal kinds, explicit nil, required/extra
arguments, repeated AST evaluation and rejected source/default ASTs.
`interp_defaults_live_test.sapp` runs one shared fixture through the retained
host and `pbi`, covering all argument-count paths, nested closures, recursion,
member/index calls and repeated default use. Its negative child verifies the
fixed diagnostic and no print effect from a call node substituted as a default.
This establishes parity for that fixture; the closure-shadowing discrepancy
above and other backend/default semantics remain open.
