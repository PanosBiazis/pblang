# Self-hosted arithmetic, builtin and non-callable cleanup

The PBLang VM now releases opted-in scalar ownership when a non-map arithmetic
or ordering primitive fails outside any guest `try`, and when the generic call
path rejects a non-callable value. This is a runtime correction, not a new
admission profile. It applies to `compiler/vm.sapp`, including bounded execution,
optimized IR and PBCS-decoded chunks.

Previously this checked source could raise while keeping an opted-in execution
context's allocations and borrows alive:

```pblang
memory_alloc(7);
fn subtract(a, b) { return a - b; }
subtract("x", 1);
```

Likewise, `let f=3; f();`, `let f=nil; f();`, `let f=map{}; f();` and calls
through a function parameter escaped through a host map/field access without
releasing that context. The frontend accepts these dynamic call forms; the VM
must preserve ownership even when runtime invocation fails.

## Contract

- `execution_context(true)` closes its private scalar memory on these escaping
  errors. Owners and borrows are released once; later close is idempotent.
- A guest `catch` keeps memory available to its handler and following code.
- The default borrowed `execution_context()` remains caller-owned, even on error.
- The original host failure message remains the failure; cleanup errors must not
  replace it. Stack locations can change with the additional implementation frame.
- Operands, call targets and arguments are evaluated once, in their existing
  order. Their prior side effects are not rolled back.
- The non-map paths for `+`, `-`, `*`, `/`, `%`, `>`, `>=`, `<`, `<=` and the two
  optimized local-add instructions participate. Valid values, boxed exact
  arithmetic, strings, and the host's floating-point division behavior remain.
- Overloaded guest methods are never retried. Their scalar failures use the same
  correction when the guest method executes the affected instruction.

For non-map arithmetic, a private helper intercepts a failed synchronous host
primitive, attempts ownership cleanup, then repeats only that primitive on its
already-evaluated operands to preserve the host's original diagnostic. This
host does not expose caught error values for a direct rethrow. No guest callback
or operand expression is repeated.

The generic non-callable path has a guaranteed rejection before entering a guest
body: a non-map callee, or a legacy raw descriptor without a map-valued `chunk`.
It attempts cleanup before the original access rejects, without replacing that
access's message. Valid legacy raw function constants still run. This does not
validate every arbitrary malformed VM descriptor.

## Strict numeric builtins

`num_floor`, `num_ceil`, `num_abs`, `num_sqrt`, `num_ln`, `num_exp`,
`num_min` and `num_max` also release opted-in ownership on an escaping type
error, in both scalar and generic dispatch. For example:

```pblang
memory_alloc(7);
fn call(f, x) { return f(x); }
call(num_sqrt, nil);
```

These eight primitives accept ordinary numbers only. A non-number is a guaranteed
host rejection, so the VM attempts cleanup before invoking the original primitive
once on the already-evaluated arguments. The original message and first-invalid-
argument priority remain. The same caught/borrowed-context rules above apply.
Wrong arity still fails before inspecting arguments, via the existing arity guard.
Successful calls, negative domain results, NaN and infinities keep the host behavior.
Boxed exact integers, rationals and decimals are still rejected by these primitives;
`num_pow`, `num_mod` and exact-number constructors have different contracts and are
not included in this numeric cleanup extension. Shadowed guest functions remain
ordinary guest calls.

See `examples/selfhost_throw_cleanup/numeric.sapp` for an executable ownership
example. `tests/semantics/selfhost_numeric_cleanup_test.sapp` exercises scalar and
generic dispatch, owner/borrow release, handlers, accepted and rejected types,
argument effects, optimized/PBCS routes and late allocations.
The tooling harness's `--numeric --reference <previous-release>` group compares
stdout, exit status and the original first error message across all six routes,
including secondary cleanup failure. This is a correctness extension; no numeric
throughput improvement is claimed.

## String transforms, search and split

`str_upper`, `str_lower`, `str_trim`, `str_split`, `str_contains`,
`str_starts_with`, `str_ends_with` and `str_index_of` now close eligible owned
scalar state on an escaping argument-type failure, in both generic and scalar
dispatch. `type(value) == "string"` identifies actual strings; the broad host
`is_string` predicate is not sufficient for boxed exact numeric objects.
The VM closes only before guaranteed type rejection, then calls the original
host primitive once. Wrong arity retains the earlier shared arity check.
Guest catches and caller-owned contexts keep their memory and original errors.

Successful Unicode case conversion, trimming, split results, character-based
search offsets, empty strings, false predicates and not-found `-1` results are
unchanged. No allocation-size quota or general host allocation-failure cleanup
is added. Slice/character-code operations, parsing, join, repeat, replace and
padding have separate contracts; they are not part of this extension.

See `examples/selfhost_throw_cleanup/string_types.sapp` and
`tests/semantics/selfhost_string_cleanup_test.sapp`. The source-hidden tooling
harness supports `--strings --reference <previous-release>` to compare exact
original diagnostics across plain/optimized/PBCS and bounded/unbounded routes,
including cleanup failure. This is a correctness extension, not a speed claim.

## Checks and limits

`tests/semantics/selfhost_arithmetic_cleanup_test.sapp` covers ownership,
borrows, handlers, optimized local adds, source/PBCS/optimizer parity, exact
values, argument order, overload effects, and secondary cleanup faults.
`tests/tooling/selfhost_arithmetic_cleanup.py` runs release-only fixtures in a
network-isolated environment with the checkout absent, and compares the previous VM failure messages with all six execution routes.
For example, the self-host `%` instruction uses `num_mod`, whose error message
already differs from the retained native `%` opcode; that existing distinction
is preserved.

The contract does not include map operands without a resolved guest overload,
arbitrary host/builtin/FFI failures, allocation exhaustion, malformed bytecode in
general, composite ownership, or other backends. It is not a speedup claim.
The retained native compiler and its nested `p:run()` source bug are unchanged.
Native PBCK execution uses a different runtime path; PBCS replay here means the
self-host IR serializer and the PBLang VM, not a fix to that retained compiler.
