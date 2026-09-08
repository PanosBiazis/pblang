# Functional iterate: exact successor evaluation

`functional.iterate(f, seed, n)` builds the eager sequence
`[seed, f(seed), f(f(seed)), ...]` of length `n` for nonnegative integer `n`.
The seed is already the first element: the callback is invoked exactly
`max(n - 1, 0)` times, left to right. Empty and singleton sequences do not invoke
the callback. No callback is evaluated on the final returned element.

```sapp
let F = import("stdlib:functional.sapp");
let seen = [];
let values = F.iterate(fn(x) { push(seen,x); return x+1; },1,4);
// values == [1,2,3,4]; seen == [1,2,3]
let singleton = F.iterate(fn(x) { throw "unused"; },7,1);
// singleton == [7], without an exception
```

Previously the helper called `f` once after adding the final element and then
discarded that result. This could cause extra state changes or an unnecessary
exception. The singleton regression failed on baseline `123991a` and passes
after this correction. Code depending on that extra side effect must perform
the intended action explicitly; preserving it would preserve the defect.

Failures from a required callback still propagate immediately, with no later
callback execution. Nil is a valid sequence value. Mutable seed/results remain
borrowed references, not snapshots: if a callback repeatedly returns the same
map, every entry can refer to that same map. This patch removes the final unused
mutation but does not add cloning or purity enforcement.

This change does not validate or redesign the legacy count argument. Negative,
fractional, nonfinite or nonnumeric counts are not newly qualified. Callers must
supply a bounded nonnegative integer count and a terminating callback. This is
an eager allocation, not a lazy/infinite iterator or hard memory/time guarantee.
Positional callback arity, callback type inference and cross-backend semantics
remain separate concerns. No compiler, Rust or VM implementation changed.

Tests cover empty/singleton/longer sequences, exact callback order/counts,
terminal values, required exceptions, nil, borrowed mutation and composition.
The actual library is also imported/run through the PBLang-written frontend/VM
with 20,000 steps, 24 frames, eight modules and a 32-KiB source limit. This is
not universal-platform or full-suite qualification.
