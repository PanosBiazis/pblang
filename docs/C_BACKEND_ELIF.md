# Conditional branches in the PBLang C backend

The PBLang-written `stdlib/compiler/ctranspile.sapp` reads the parser's
`elif` branch body in both closure analysis and C statement emission. Earlier
versions emitted an empty C branch for the `elif` spelling: for example,
`if x == 0 { return 10; } elif x == 1 { return 21; }` could fall through
and return zero when `x` was one. The two-token `else if` spelling used a
nested statement and was not affected by that representation mismatch.

The correction applies to the numeric functions-only `transpile(source)`
route and hosted float/integer profiles. Hosted closures now also discover
captures, function literals, and named function values occurring only inside
an `elif` body. The shared body reader prefers the parser's `body` field and
retains `block` and `then` aliases for callers of the direct AST API. An
explicit empty canonical block remains empty.

Unsupported statements inside `elif` go through normal rejection instead of
being silently dropped. This does not add support for exceptions in emitted
C, change numeric semantics, or establish support for an entire embedded
board. Integer mode remains explicit; general backend equivalence and
hardware/toolchain verification remain separate requirements.

Run the executable regression from the repository root with a C compiler on
PATH:

```sh
pblang run tests/semantics/ctranspile_elif_test.sapp --allow-exec
```

This test requires a compiler rather than silently skipping native execution.
It checks actual program output in both hosted numeric modes, nested and
multiple branches, loop control, condition order, empty bodies, closure
captures and creation, legacy AST aliases, and unsupported branch content.
Native child execution uses the existing bounded C runner. Successful C
compilation alone would not have detected the original wrong-result bug.

The separate functions-only route is compared against actual PBLang execution
at C `-O0` and `-O2`, with strict C99 for this numeric fixture:

```sh
python3 tests/tooling/ctranspile_elif_runtime.py --pblang /path/to/reviewed/pblang
```

The Python file only supervises local test processes. Production lowering and
branch handling remain PBLang. This fixture does not establish that every
other feature of the hosted backend is ISO C99-only.
