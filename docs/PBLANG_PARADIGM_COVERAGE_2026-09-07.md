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
| Functional | `functional.sapp`: map/filter/folds, compose/pipe, partial/curry, memoization, declarative combinators | Selected combinator and composition tests pass. These are eager array operations, not proof of universal lazy evaluation or enforced purity. Positional curry rejects arities outside 0–4; `curry_args` collects 0–256 vector-callback values, not general positional spread. Memoization and bounded scalar unique/projection semantics are checked below; legacy string-map grouping and arbitrary structural/exact-number keys remain distinct limitations. |
| Object-oriented | `class.sapp`: single inheritance, overrides, super helpers, mixins, statics, bound methods and operator bridges | `oop_complete` and mixed-library tests pass. `call`/`call1..4` and `bind0..2` are finite adapters, not arbitrary arity. Single-parent extension is not multiple inheritance; mixins merge methods with later values winning. Explicit-owner super calls now have four-level inheritance tests; legacy omitted-owner dispatch still needs migration for inheritable overrides. |
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

## Bounded typed collection keys — 2026-09-08

Baseline `addbb8a`. Mixed-type deduplication loss reproduced before the fix.
Functional/array unique and array distinct_by now share a bounded PBLang scalar
key helper. New functional.unique_by and group_by_entries provide explicit
object projection and typed group records while preserving legacy string-map
group_by/frequencies output. Tests cover scalar distinctions, stable order,
callback effects, rejection/boundaries and actual bounded frontend/VM imports.
See `COLLECTION_KEYS.md` for the deliberate compatibility narrowing (4096 items,
scalar keys and additional key/comparison budgets). No arbitrary structural-key,
exact-number, C99 embedded-NUL or universal backend support is claimed. Direct
array_unique remains a legacy native boundary. Once and full paradigm composition
remain open; this is not full collection-wide or 1.0.0 qualification.

## Extrema callback evaluation — 2026-09-08

Baseline `d4a2c11`. The new singleton regression reproduced duplicate first-key
evaluation. `max_by` and `min_by` now start comparison at the second element,
retaining the first computed key. Exact callback-order/count tests include
stateful selection, exception effects and bound-method/generic composition.
The actual functional library also passed a bounded PBLang frontend import/run
probe. See `FUNCTIONAL_EXTREMA.md`; numeric comparison, first-tie and borrowed
result semantics are explicit. General callable/paradigm coverage remains open.

Inspection also found different existing exception behavior in `functional.once`
(marks called before callback) and `pipe.once` (marks after success). Their
failure/reentrancy contract needs a separate compatibility decision and tests;
neither implementation was changed or qualified by this extrema slice. The later
once-attempt slice below repairs functional.once and adds an explicit pipe API.

## Once attempt lifecycle — 2026-09-08

Baseline `d50f136`. Functional.once's failed-call-to-nil-success regression was
reproduced and repaired with started/completed state in a shared PBLang helper.
New pipe.once_attempt shares the contract; legacy pipe.once retains its different
retry and reentry behavior. Tests cover sequential per-closure lifecycle, nil,
first-input effects, callback errors, borrowed results, caught/uncaught reentry,
malformed callbacks, legacy retries, actual frontend/module imports and helper
serialization/optimization. `FUNCTIONAL_ONCE.md` documents compatibility and
remaining limitations: no transactional rollback, concurrency/global exactly-once,
cross-VM persistence or recovery after quota/process abort. Broader paradigm
composition and migration of legacy retry callers remain separate.

## Sequence callback evaluation — 2026-09-08

Baseline `123991a`. `functional.iterate` no longer computes the unused successor
after its final element. For nonnegative integer counts it now invokes the callback
exactly max(n-1,0) times, retaining existing seed/reference and required-exception
semantics. The new singleton regression failed before the fix. Tests verify
terminal values, callback effects and actual bounded frontend/library execution.
See `FUNCTIONAL_ITERATE.md`. This does not add lazy evaluation, validate legacy
count inputs, or resolve unique/grouping and once failure/reentrancy contracts.

## Explicit super owner — 2026-09-08

Baseline `bcf223e`. `super_method`, `super_call`, `super1` and `super2` accept an
optional final owner class. Selection begins at that owner's parent and passes
the unchanged dynamic instance. Four-level inheritance, raw/bound methods,
functional composition, mixins, parent redispatch, errors and existing nil/
omitted-owner behavior pass focused tests. A separate probe imports the actual
class library through the PBLang frontend/VM.

See `CLASS_SUPER_OWNER.md`: predeclare a class binding before referencing it
inside its initializer's methods. This explicit selector does not infer lexical
ownership, prove ancestry or fix old omitted-owner calls automatically. Native
class syntax/defaults/types and broader paradigm composition remain open.

## Stable ordering with explicit compatibility — 2026-09-08

New functional.stable_sort_by composes comparator functions with opaque payloads
and stable ordering, without replacing legacy sort_by callback traces. Tested
equal-key map records retain order and object references; negative/malformed
returns, callback exceptions and actual bounded frontend/VM execution are covered.
`FUNCTIONAL_SORT.md` specifies finite numeric results and a consistent total
preorder. Purity, key immutability and callback termination are caller obligations,
not enforced type/effect-system properties. This additive library API does not
complete every functional/OOP combination or establish cross-platform parity.

## Chunk/window shape validation — 2026-09-08

Functional.chunk/windows reject non-array inputs and nonfinite/fractional or
non-number sizes before iteration. Chunk requires positive size; windows retains
the nonpositive-integer empty sentinel. Tests cover every length 0..8 with sizes
1..10, exact order/shape, partial chunks/full windows, sum pipelines and borrowed
object payloads, plus actual bounded PBLang frontend/VM execution. These checks
correct invalid-input semantics, not enforce deep immutability, lazy sequences,
total output budgets or general composition across every paradigm/backend.
See `FUNCTIONAL_WINDOWING.md`; other size/step/count APIs remain unqualified.

## Array stepped-window composition — 2026-09-08

Array.chunk/windows now validate shape and counts in PBLang before work, with
independent result slots and borrowed object payloads. Focused array/functional
and multi-paradigm regressions pass alongside actual PBLang frontend/VM imports
and plain/optimized/serialized helper execution. The matrix checks 90 chunk and
900 stepped-window combinations, not every paradigm or backend combination.

Explicit nil with a default parameter still differs between retained host and
PBLang frontend; this is a language-level unresolved distinction, recorded in
`ARRAY_WINDOWING.md`. PBCS numeric literal limits also remain. Bulk copying
uses an existing runtime primitive, not a wholly native-free implementation.
General default/arity/class/block composition, lazy/resource-safe sequences and
cross-device qualification remain open.

## Contextual match versus object access — 2026-09-08

The PBLang parser now composes match-named map/array bindings with member and
optional-member/index access. Real match expressions and a subject read from
match.value retain their existing lowering. Checked compilation, plain/optimized
VM execution and bytecode roundtrips agree in focused tests. Missing bindings and
malformed postfixes still reject. Parenthesized subjects remain an explicitly
tested unsupported ambiguity, not a completed general grammar/paradigm guarantee.
See `SELFHOST_LEXICAL_BUILD_PROGRESS_2026-09-08.md`.

## CLI security policy admission update — 2026-09-08

PBLang-owned pattern and dependency policy preflight now reject malformed
configuration before entry reads in pbsecure and security-selected pbanalyze.
The dependency library also rejects malformed pattern policy before traversal,
with an invalid report rather than a completed finding report. Scope-specific
nil resolution, command-line rule overrides and existing report publication
protections are retained. Validation outcomes expose no reusable policy handles;
normal analysis still revalidates. No Rust source or executable rebuild.

Evidence and costs: BENCHMARK_SESSION_2026-09-08_SECURITY_CLI_PREFLIGHT.md.
Remaining: concurrent policy snapshots, pbanalyze bounded pre-allocation reads
and snapshot verification, general analyzer performance, C99 embedded-NUL parity,
native self-bootstrap/executable refresh, compiler/paradigm compatibility,
VM ownership/hard heap/preemption, typed FFI/raw bypass/hostile-code sandbox,
publisher trust/artifact handoff/revocation/rollback, broad product regression
and physical-platform qualification. PBLang 1.0.0 remains NOT READY.

## Bounded selectable-analysis reads — 2026-09-08

pbanalyze now uses the existing secure descriptor reader with a 16 MiB ceiling
before materializing source bytes in the VM. Public read/UTF-8 error schemas and
policy-before-read ordering remain intact; ordinary-read fallback is forbidden.
Symlink/special-file and unsupported-platform-path rejection are intentional
compatibility tightenings. See BENCHMARK_SESSION_2026-09-08_ANALYSIS_BOUNDED_READ.md
for targeted tests, measured overhead and actual platform limits.

Still open: post-analysis source/config snapshots, concurrent namespace/policy
consistency, hard VM heap/time and cleanup enforcement, typed FFI/raw bypass and
hostile-code sandbox, publisher trust/artifact handoff/revocation/rollback,
compiler/C99 parity, native executable refresh, whole-product regression and
physical-platform verification. This does not certify 1.0.0 readiness.

## Analysis selection isolation — 2026-09-08

Fixed two reproduced source_analysis selection failures: mixed-case ALL now
selects the same canonical categories as all, and caller edits to exported or
returned category arrays no longer change subsequent security selection.
Valid defaults, unknown handling and first-requested/deduplicated ordering are
retained. This closes a selectable-analysis issue, not a bypass of every build
security gate. Evidence: BENCHMARK_SESSION_2026-09-08_ANALYSIS_SELECTION.md.

Still open: trusted caller-supplied security_report and row annotation ownership,
max_findings/input-size admission, concurrent snapshots, compiler/C99 parity,
self-bootstrap/executable refresh, runtime heap/time/ownership/cleanup, typed
FFI/raw bypass/sandbox, package trust/artifact/revocation/rollback, whole-product
compatibility and physical-platform verification. PBLang 1.0.0 remains NOT READY.

## Selectable-analysis finite finding limits — 2026-09-08

source_analysis now admits only finite integral ordinary-number max_findings
values in 1..1024; omitted keys retain 256. Fractional limits retaining two rows
and NaN throwing unstructured errors were reproduced against 2f299cc. A shared
PBLang limits API now rejects these inputs with constant structured outcomes.
pbanalyze applies the same guard before config/source I/O; explicit malformed
limits no longer silently fall back. Valid defaults/category precedence and
security blocking remain covered by focused tests. No Rust source or binary
replacement. Evidence: BENCHMARK_SESSION_2026-09-08_ANALYSIS_LIMITS.md.

Still open: direct project_audit malformed-limit admission, caller-supplied
security_report trust/row ownership, whole-input/work bounds, category-filter/
truncation semantics, source/config/namespace snapshots, compiler/paradigm/C99
parity, self-bootstrap/executable refresh, VM resources/ownership/cleanup,
typed FFI/raw bypass/sandbox, package authenticity/artifact handoff/revocation/
rollback, whole-product compatibility and physical-platform verification.
PBLang 1.0.0 remains NOT READY.

## Direct project-audit finding-limit admission — 2026-09-08

Both project_audit text and record entry points now reject malformed finding
budgets before conversion/traversal, using a fixed catchable exception rather
than a success-shaped empty report. Finite integral ordinary numbers 1..4096
are accepted; legacy nil defaults 200/500 are retained. scan_text, project audits
and readiness wrappers propagate rejection, preventing accidental clean gates.
Only PBLang production source changed. Evidence, cost and compatibility:
BENCHMARK_SESSION_2026-09-08_PROJECT_AUDIT_LIMITS.md.

Remaining: suppressed arrays and whole-input/record/work budgets; audit CLI
parser fallback and filesystem-bridge pre-I/O/module-option admission; generic
quality/progress gate input/threshold validation and truncation; snapshots;
trusted security_report ownership; compiler/paradigm/C99 parity and linter
float reasoning; self-bootstrap/executable refresh; VM resource/ownership/
cleanup guarantees; typed FFI/raw bypass/sandbox; package authenticity/artifact
handoff/revocation/rollback; broad product compatibility and physical platforms.
PBLang 1.0.0 remains NOT READY.
