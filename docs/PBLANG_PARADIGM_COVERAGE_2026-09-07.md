# Programming-model support and composition — 2026-09-07

## Decision and scope

PBLang has substantive multi-paradigm implementations, not demonstrated full
support for every feature of every paradigm or every combination/backend.
This is the first bounded coverage slice requested alongside 1.0.0 remediation,
not a completed language-wide audit. Baseline: `b9a1b68`; evidence belongs to
this report's commit. Runtime/library support, syntax sugar, static checking
and independently executed backend support are separate claims.

The full `functional.sapp`, `class.sapp` and `generics.sapp` modules and
their selected tests were read. Compiler scope/capture paths and selected
class/async parser paths were checked. Other rows below are source-location
inventory with explicitly partial inspection, not whole-module certification.

## Current evidence matrix

| Model | Concrete implementation | Evidence and limits in this slice |
| --- | --- | --- |
| Imperative / procedural / structured | Functions, mutable bindings, branches and loops in `compiler/parser.sapp`, `codegen.sapp`, `vm.sapp` | Focused count/range/for-in, scope, try and closure tests pass on plain/optimized PBLang VM paths; not every control-transfer/backend path. |
| Functional | `functional.sapp`: map/filter/folds, compose/pipe, partial/curry, memoization, declarative combinators | Selected combinator and composition tests pass. These are eager array operations, not proof of universal lazy evaluation or enforced purity. Positional curry now rejects arities outside 0–4; `curry_args` adds explicit vector-callback collection for 0–256 values, not general positional spread. Memoization identity is repaired below; stringified unique/grouping semantics still need separate tests. |
| Object-oriented | `class.sapp`: single inheritance, overrides, super helpers, mixins, statics, bound methods and operator bridges | `oop_complete` and mixed-library tests pass. `call`/`call1..4` and `bind0..2` are finite adapters, not arbitrary arity. Single-parent extension is not multiple inheritance; mixins merge methods with later values winning. Inherited super-calls through deeper subclasses still need dedicated resolution tests. |
| Class syntax | `compiler/parser.sapp::parse_class` builds a class table and constructor | Separate from the class library API. Selected source review shows method type/default syntax is parsed but discarded by this lowering; comment documents no `__call`/new0..new3 parity. Full constructor/method/default/type equivalence remains open. |
| Generic / parametric / structural | `generics.sapp`: composable type predicates, typed lists/maps/boxes/options, interface-predicate bridge | `generics` and composition tests pass for runtime checks, negative mapper results and preserved input. This library is runtime-parametric, not evidence of static higher-kinded/dependent-type support or all native generic syntax. |
| Ad-hoc polymorphism / typeclasses | `typeclass.sapp` runtime registration and predicate-selected instances | Entry points inventoried; whole-module laws, overlap/coherence and combinations not revalidated here. |
| Logic / relational | `logic.sapp` facts, rules and substitutions | Partial source inspection; explicitly a small Datalog-style engine, with ground-value inequality rather than a complete dif/2 constraint store. No Prolog/constraint-language completeness claim. |
| Concurrent / asynchronous | `async.sapp`, async parser sugar and spawn/channel/join boundaries | Partial source inspection; selected parser `await` lowers to join and library await waits for a result. This is not proof of nonblocking scheduling, cancellation, structured concurrency or every actor combination. No workers started for this audit. |
| Event-driven / reactive | `signal.sapp`, `event_bus.sapp` callbacks/pub-sub | Entry-point inspection only; not certification of reactive streams, backpressure, reentrancy or concurrent subscription changes. |
| Metaprogramming | `macro.sapp` source templates and generated-source checking | Partial inspection; library explicitly distinguishes itself from native syntax macros. General hygiene/staging/reflection combinations remain unreviewed here. |
| Other declarative / dataflow / domain-specific / polyglot styles | Existing library/tooling domains require separate contract inventories | Not assessed as complete by this slice. Foreign language integration is a separate execution boundary, not automatic adoption of that language's semantic guarantees. |

## Executed combinations

`multi_paradigm_composition` exercises PBLang-written libraries on the retained
host: inheritance + mixin + bound method + functional pipeline; generic map +
bound method + fold; procedural population + typed containers; functional map
+ object dispatch + exceptions. Invalid mapper output and missing methods
reject; original input remains unchanged. Existing `oop_complete`,
`functional_combinators` and `generics` suites also pass.

`selfhost_try_capture` separately executes map-based object-style dispatch +
captured function + procedural loop + try/exception behavior through plain,
optimized and in-memory serialized/reloaded PBLang bytecode. This is not a
claim that the entire class/generic libraries were independently recompiled
and qualified through all those paths.

An initial retained-host combination yielded `[nil, -1, nil]` when successful
if-branch tails were semicolon-terminated; without that semicolon it yielded
`[3, -1, 7]`. Self-hosted block-expression lowering currently keeps the last
expression-statement's value regardless of its semicolon. Record this as
**open block-tail semantic divergence**, not proof that all styles compose.
The canonical value/discard rule must be reconciled before release-wide parity.

## Work required for a full support claim

1. Freeze a feature-level contract per model: values/effects, mutation, capture,
   dispatch, arity, errors, scheduling, lifetimes, supported syntax and types.
2. Finish whole-owner/caller reviews for inventoried domains; resolve the
   concrete arity, class-default/type and block-tail limitations above.
3. Expand exact positive/negative composition tests: pairwise combinations,
   then higher-order combinations around shared state, errors, asynchronous
   work, overloads, imports and serialization. Pairwise coverage alone is not
   proof for arbitrary higher-order composition.
4. Run each supported combination through each claimed compiler/runtime/backend
   with matched output, values, effects, errors and resource behavior. Keep
   retained-host, PBLang VM, foreign adapters and device evidence distinct.
5. Define a finite advertised support matrix and explicit unsupported cases.
   No finite test campaign proves all possible programs or an undefined set
   of “all combinations”; the matrix and regressions must be reviewable.

This report supplements, and does not close, the remaining-work matrix in
`PBLANG_RUST_COMPARATIVE_AUDIT_2026-09-06.md`. No 1.0.0 readiness or universal
paradigm/platform support claim follows from the passing focused tests.

## Memoization identity remediation — 2026-09-07

Baseline `81768c6`. A new regression reproduced number/string cache aliasing.
`functional.memoize` and `pipe.memoize` now delegate to one PBLang module,
using type-tagged scalar buckets with exact input comparison. Mutable/callable,
non-finite, zero and oversized text inputs bypass caching. Retention is capped
at 1024 entries with 4096-character text keys; this is not a hard heap limit.
Successful nil values cache; exceptions retry. See `FUNCTIONAL_MEMOIZATION.md`.

Both public APIs passed the new identity/capacity tests. The helper's compiled
source also passed plain, optimized and in-memory bytecode round-trip checks
as one unit, without claiming independent import-loader qualification.
This fixes the identified memoization collision, not `unique`, generic arity,
class-default semantics, full paradigm composition or the release blockers.

## Curried argument preservation — 2026-09-07

Baseline `92354d8`. The new regression reproduced acceptance of an unsupported
positional arity before the repair. `curry` now admits only integral 0–4
arities, preserving nil/default 2 and immediate zero invocation. Its internal
dispatcher rejects oversized arrays instead of dropping values.

The new `curry_args(f, n)` collects up to 256 values for one array-taking
callback. Tests cover exact order, reusable prefixes, callback exceptions,
bound methods and runtime generic validation. A separate bounded frontend test
imports the actual functional library and exercises both conventions.
See `FUNCTIONAL_CURRY.md` for limits and migration. General positional
apply/spread, callback signature inference and full composition remain open;
this is not a claim that arbitrary-arity positional functions now work.
