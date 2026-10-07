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
`num_pow` and `num_mod` have different value-dependent contracts, covered below.
Exact-number constructors have the separate failure handling described below. Shadowed guest functions remain
ordinary guest calls.

See `examples/selfhost_throw_cleanup/numeric.sapp` for an executable ownership
example. `tests/semantics/selfhost_numeric_cleanup_test.sapp` exercises scalar and
generic dispatch, owner/borrow release, handlers, accepted and rejected types,
argument effects, optimized/PBCS routes and late allocations.
The tooling harness's `--numeric --reference <previous-release>` group compares
stdout, exit status and the original first error message across all six routes,
including secondary cleanup failure. This is a correctness extension; no numeric
throughput improvement is claimed.

## Value-dependent modulo and power failures

Explicit `num_mod` and `num_pow` now use the same escaping-error ownership rule
as arithmetic opcodes, in both scalar and generic builtin dispatch. Previously
`num_mod(7,nil)` and `num_pow(2,nil)` could leave private allocations/borrows live,
while the equivalent failing `%` instruction already closed its opted-in context.
This also covers checked-source `**`, which dispatches through `num_pow`.

These primitives accept both ordinary binary64 and exact numeric objects, so a
simple number-only admission check would incorrectly reject valid bigint,
rational or decimal values. The VM instead intercepts failures of the pure host
primitive only when a private owned context has live memory and no guest handler.
It attempts the existing cleanup and repeats only that failed primitive on the
already-evaluated operands to reproduce its original host diagnostic. No guest
method, call target, argument expression or external operation is replayed. The
host math implementations inspect numeric values directly, including when maps
are rejected; they do not invoke guest overloads. Shadowed guest functions and
operator-method dispatch retain their existing rules.

Covered failures include invalid argument types, bigint modulo by zero, unsupported
rational/decimal modulo, negative or out-of-range bigint exponents and nonintegral
rational/decimal exponents. Arity still fails through the existing earlier guard.
Successful exact arithmetic, mixed fractional demotion, and float NaN/infinity
results retain host behavior. A guest catch keeps memory usable; borrowed contexts
remain caller-owned. A secondary cleanup failure cannot replace the original
numeric error, but completion of cleanup is not guaranteed with corrupt private
metadata. This does not add allocation/exponent quotas or recover process aborts,
panics or exhaustion, and it does not fix arbitrary builtin/FFI failures.

`tests/semantics/selfhost_exact_math_cleanup_test.sapp` checks ownership/borrows,
handlers, argument effects, overloads, accepted exact values, optimizer/PBCS replay
and late allocations. `tests/tooling/selfhost_arithmetic_cleanup.py --exact-math
--reference PREVIOUS_RELEASE` compares original errors across six routes and an
injected secondary cleanup failure. The runnable `examples/selfhost_throw_cleanup/
exact_math.sapp` and small `tests/fixtures/selfhost_exact_math_acceptance.sapp` are
installation acceptance fixtures. This is a correctness change, not a speed claim.

## Exact-number constructor failures

`bigint(x)`, `decimal(x)`, `rational(x)` and `rational(numerator, denominator)`
now release private opted-in execution memory when a host conversion failure
escapes without a guest handler. Both scalar and generic dispatch use this rule.
Previously even a short invalid value such as `bigint(0.5)`, `decimal("bad")`
or `rational(1,0)` could leave allocations and borrows live.

The constructors inspect immutable numeric/string values and do not invoke guest
callbacks. As with modulo/power, the VM catches the failed primitive, attempts
private cleanup and repeats only that failed conversion on evaluated arguments
to retain the original host diagnostic. It does not repeat argument expressions
or successful allocations. This is intentionally separate from generic callable
retry: guest functions, effects and overload callbacks must never be replayed.

Valid exact conversions, identities, rational reduction, nonfinite/type/value
rejection and argument-error priority keep their host contracts. Arity rejection
still occurs first. Guest catches keep their memory usable; borrowed contexts
remain caller-owned; shadowed constructor names remain ordinary guest functions.
Secondary cleanup faults cannot replace the original conversion diagnostic.
Diagnostic strings can contain an invalid input string, as before; applications
must not assume that host errors redact input. Tests use synthetic values only.

This does not add digit/allocation limits, change numeric conversion semantics,
or recover native aborts, panics or exhaustion. The existing private/unaliased
metadata assumption and corrupt-state cleanup limitations still apply.

`tests/semantics/selfhost_constructor_cleanup_test.sapp` covers ownership,
lifetimes/borrows, caught and borrowed execution, late allocations, argument
effects, overloads, valid conversions, optimizer and PBCS replay. The diagnostic
driver's `--constructors --reference PREVIOUS_RELEASE` group compares host
messages across six routes and secondary cleanup faults. Small installation
probes are `examples/selfhost_throw_cleanup/constructors.sapp` and
`tests/fixtures/selfhost_constructor_acceptance.sapp`.

## Bitwise builtin and operator failures

`bit_and`, `bit_or`, `bit_xor`, `bit_not`, `bit_shl` and `bit_shr` now release
private opted-in execution memory when their host error escapes without a guest
handler. Scalar and generic dispatch follow the same rule. The PBLang parser
lowers `&`, `|`, `^`, `~`, `<<` and `>>` to these builtins, so checked-source
operators participate as well. Previously invalid types, fractional operands
and invalid shift counts could leave allocations and borrows live.

These failures depend on both type and value. The VM leaves integer admission,
signed 64-bit conversion, shift-count validation and error priority to the host.
As with exact-number constructors, it catches a failed pure primitive, attempts
private cleanup, then repeats only that primitive on already-evaluated operands
to preserve the original error. No guest operand expression or method is retried.
Successful bitwise operations are invoked once; catches and borrowed contexts
keep their memory. Arity checks and shadowed guest functions remain unchanged.

The correction deliberately preserves the host's existing binary64 boundary
rounding and conversion behavior, including its accepted rounded positive
64-bit endpoint. It does not redefine the integer range or add exact-bigint
bit operations. Tests distinguish rejected neighboring values from accepted
endpoints and retain arithmetic right shifts and shift counts 0 and 63.

`tests/semantics/selfhost_bitwise_cleanup_test.sapp` covers six builtins and
operator lowering, type/value failures, handlers, owned/borrowed lifetimes,
argument effects, generic dispatch, optimizer/PBCS replay and primary errors
despite secondary cleanup faults. The diagnostic harness's `--bitwise
--reference PREVIOUS_RELEASE` group compares exit/stdout/first host error across
six routes. Wide boundary operands in that matrix are computed at runtime:
PBCS currently rejects some serialized numeric literals before the bitwise
operation. This cleanup change does not fix that separate representation limit.
`examples/selfhost_throw_cleanup/bitwise.sapp` is a short public-CLI
installation probe for all six builtins plus caught/borrowed state. There is no
claim of general host/FFI, native abort/panic/exhaustion recovery or speedup.

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

Fallback arithmetic opcodes on map operands without a resolved guest overload
remain outside this correction, as do arbitrary host/builtin/FFI failures,
allocation exhaustion, malformed bytecode in general, composite ownership,
and other backends. It is not a speedup claim.
The retained native compiler and its nested `p:run()` source bug are unchanged.
Native PBCK execution uses a different runtime path; PBCS replay here means the
self-host IR serializer and the PBLang VM, not a fix to that retained compiler.
