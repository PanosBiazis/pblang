# Ordered pblang implementation ledger

The executable source of truth is `stdlib:vision_ledger.sapp`; run
`pblang tools/pbvision.sapp` to view it. It records the requested work in a
stable sequence: pblang-only policy, self-host parity, x86-64, i386, C99,
interpreter, platforms/devices, measured performance, one-line apps,
libraries, products, AAP idea review and migration, language and polyglot
benchmarks, safety/security, then docs/UI quality.

Each item has three fields that must stay distinct:

- `state`: broad progress, never a marketing claim.
- `evidence`: code or tests that exist now.
- `exit_criterion`: what must pass before the item can be complete.

The first item now has an executable, pure-PBLang enforcement layer rather than
evidence based only on file extensions. `stdlib:migration_policy.sapp` and
`tools/pbmigrate.sapp` reject new or added Rust, require bounded exact-path
exceptions for necessary non-Rust host boundaries, and validate the ordered
`reference -> parity -> verified -> retired` lifecycle. This keeps the policy
`active`: adoption by every product/release workflow and removal of hidden host
implementation dependencies are still exit criteria. The gate does not claim
that the current bootstrap is already Rust-free.

The one-line-apps item now has a completed facade substage while remaining
`active`. `stdlib:app.sapp` exposes compact CLI, GUI, web, REST, desktop,
full-stack, game, multimedia, database, networking, automation, AI, and ML
entry points. `stdlib:app_domains.sapp` delegates to real lazily loaded PBLang
libraries, bounds actions/steps/options/input/collection, keeps preview
plan-only by default, and requires explicit I/O permission for database,
networking, and AI execution. REST and desktop retain the complete hardened
web and GUI configuration surfaces. Audio/video multimedia providers and
cross-target UI acceptance remain open, so the broad item is not marked
complete.

The polyglot/parallel item now has a completed controlled-execution substage
while remaining `active`. `thread_pool.sapp` bounds workers and queued tasks,
uses transferable closed-upvalue worker factories, and records parallel task
outcomes in the parent VM with deterministic priority order. `compute.sapp`
separates hardware presence from executable runtime availability and exposes a
versioned policy/route/result contract; accelerator runtime failure either
fails or reports an explicit CPU fallback. Polyglot execution retains language
and source-size limits in every mode, and the PBLang FFI layer adds exact
library/symbol allowlists, arity/type checks, argument bounds, and raw-pointer
opt-in. Compiler-proven purity/race freedom, generic device code generation,
remote trust, cancellation, and real dynamic work stealing remain active exit
criteria.

The current item is self-host parity. Its latest completed substages give the
pblang-written hosted C99 backend numeric closures, named function values, and
bounded in-memory linking of project modules with persistent top-level values.
Function literals are closure-converted to generated C functions; captures are
copied transitively, captured writes persist in the private environment, and
higher-order calls use validated numeric handles rather than raw pointers. The
runtime checks handle validity, exact numeric representation, arity, allocation
overflow, and registers deterministic exit cleanup. Float and integer profiles,
native execution, forged-handle and arity-failure paths, sanitizer execution,
the C99/kernel suites, product boundaries, and all 44 pblang-native self-host
suites verify the closure work. The linker additionally verifies canonical
relative paths, dependency order, namespace collisions, input/output limits,
PBIDE integration, translation-unit storage, deterministic module initialization,
mutable globals, checked global closure calls, sanitizer execution, and native
cross-module output. Freestanding closure lifetime policy, GC-managed general
values, thread-safe module initialization, filesystem/package loading, and full
backend/stdlib parity remain active.

The self-hosted checker substage now has bounded lexical type environments,
function-signature and ADT hoisting, recursive array/map/result annotations,
options, non-null and numeric precision types, nested generic substitution,
typed struct fields and enum payloads, forward ADT references, and enum-match
exhaustiveness. Malformed AST and resource exhaustion fail closed, and the real
tokenizer/parser replaced `frontend_analyze` substring heuristics. All 44
self-host suites and the complete 682-test semantic run pass. Pinned evidence
records both the remaining 7.94% legacy-path feature cost and the 13.32%
reduction from the first recursive-type draft. Bounded deterministic typo
suggestions now cover unknown identifiers, struct fields, enum variants, and
frontend fix-its; semantic/context-aware ranking beyond edit distance remains
open. Result propagation, generic constraints, and whole-stdlib differential
checking remain active exit criteria.

The interpreter ledger item also gained a completed substage without changing
its broad `active` state. The pblang-only tree interpreter now proves when a
builtin identifier cannot be shadowed and skips repeated dynamic scope lookup
only in that case. Sixteen-builtin and adversarial shadowing tests pass, the
full semantic run is 680/680, and seven paired pinned-process benchmarks record
6.95-11.52% lower median execution on the four targeted call-heavy fixtures.
The measured preparation cost and rejected numeric-ID dispatcher are retained
in the benchmark report. General interpreter wall-time, RSS, and adversarial
resource budgets remain open exit criteria.

No finite implementation can truthfully guarantee that one language wins
every workload, platform, benchmark, or future device. This ledger turns that
ambition into reproducible, target-specific gates and keeps unsupported
targets labeled as ongoing work.
