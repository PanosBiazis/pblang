# Curried calls in PBLang

Import `stdlib:functional.sapp`. There are two explicit calling conventions;
neither performs generated-code evaluation or a foreign-language call.

## Positional curry

`curry(f, arity = nil)` collects one value per call, then invokes `f` with
positional arguments. The supported arity is an integer from 0 through 4.
Omitted/nil arity remains 2; arity 0 immediately invokes `f()`.

```pblang
let F = import("stdlib:functional.sapp");
let add = F.curry(fn(a, b) { return a + b; });
let add10 = add(10);
print add10(5); // 15
```

Invalid types, negative/fractional values and arities above 4 reject at
construction, before collecting arguments or executing the callback. The
underlying positional dispatcher also rejects excess arguments, rather than
silently discarding them. The declared arity is not reflection on `f`'s actual
signature; a mismatched callback still fails through normal runtime call checks.

Compatibility correction: older code could accept arity 5 and call with only
the first four values, or treat a negative arity as zero. These cases now
reject. Valid 0–4 behavior and the default remain unchanged.

## Vector curry for larger compositions

`curry_args(f, arity)` collects 0–256 values and invokes `f(args)` with ONE
array containing all of them. Arity is required, integral and range-checked.
For zero it immediately invokes `f([])`.

```pblang
let F = import("stdlib:functional.sapp");
let total5 = F.curry_args(F.sum, 5);
print total5(1)(2)(3)(4)(5); // 15
```

This also works with a bound method accepting an array, or a callback that
validates an array's element types before processing it. It is not a spread
adapter that can invoke an arbitrary five-parameter function unchanged.

Each step copies the accumulated outer array. Saved prefixes can branch or
retry after callback failure; mutation of a completed outer array cannot alter
the saved prefix. Element objects are still shared references. Callback errors
propagate; no results are cached and partial calls do not invoke the callback.

The 256-value ceiling bounds this collector, not element size, callback work
or aggregate VM heap. Prefix copying costs quadratic total copied references
over a complete chain. No performance, concurrent-use, purity or isolation
guarantee is implied.

## Evidence

`functional_curry_contract` tests eager rejection without effects, positional
0–4/default behavior, order, reusable prefixes and callback errors.
`functional_curry_args` tests malformed arities, 256/257 boundaries, complete
argument order, heterogeneous values, prefix isolation and retry, plus
object-bound-method + generic-validation + functional-reduction composition.
`selfhost_curry_args` imports the actual library through the bounded PBLang
frontend/VM and checks vector calls, branching, positional calls and rejection.
These are focused checks, not qualification of all backends or combinations.
