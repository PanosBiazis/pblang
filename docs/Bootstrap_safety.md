# Bootstrap safety — surviving the move from Rust to pblang

> **Status (2026-05-30):** self-hosting is already in place. The pblang-written
> compiler under `stdlib/compiler/` (tokenizer → parser → semantic → typeck → ir
> → optimizer → codegen, plus `selfhost_frontend.sapp` and the `vm.sapp`
> interpreter) tokenizes, parses, compiles, and runs pblang end-to-end. This
> document is the **live maintenance contract** for that frontend and for any
> further "move X from Rust to pblang" change — not a hypothetical future plan.

This document answers a real worry that comes up the moment you start
moving compiler/runtime code from Rust into pblang itself:

> *If the pblang executable is gone or broken, I have to rebuild from
> Rust — but by then the Rust source will have drifted away from the
> features the project depends on, and I'll be stranded.*

The risk is genuine. Every self-hosted language has faced it. The good
news is that the answer is **not** "don't self-host" — it's a small set
of disciplined practices, layered so that no single failure leaves you
without a path back to a working compiler. Treat this document as the
**contract** any "move X from Rust to pblang" change must satisfy.

## TL;DR — the five-layer safety net

| Layer | What it gives you | Cost |
|---|---|---|
| **1. Rust reference is never deleted, only deprecated.** | A working compiler from a clean checkout, no matter what. | ~0 — just don't delete code. |
| **2. Differential testing** every Rust↔pblang twin. | Drift is caught the moment it appears, not months later. | One test per moved feature. |
| **3. Pre-compiled `.pbck` artifacts** of every self-hosted module are reproducibly checked into `git`. | Target recovery path if the source-form parser breaks. The runtime cache exists; checked-in snapshots and freshness CI do not yet. | Generator + deterministic snapshot gate. |
| **4. `pblang transpile → C → cc`** as the universal floor. | If both Rust *and* pblang fall over, C still bootstraps. | The transpiler already exists; just keep it working. |
| **5. A monthly "fresh-checkout build" CI job.** | Catches drift in the Rust reference path itself. | One scheduled workflow. |

These rows are the required safety contract, not a claim that every layer is
already complete. Layer 3 is explicitly partial below.

---

## Layer 1 — The Rust reference never gets deleted

**Rule:** when you move code from Rust into pblang, the Rust version is
not removed. It moves into a `legacy_` / `_oracle` / `reference/` namespace
(use whichever convention reads best at the call site) and the new
pblang version is wired in alongside, gated by a feature flag.

The Rust version stops being the *default* once the pblang version
passes its differential tests for one release cycle. It stops being
*maintained for new features* once the pblang version has been stable
for two releases. It is **never deleted** while the language can still
be built from source.

**Why this works:** the worst-case recovery is `git checkout HEAD~N`
to a known-good Rust path. As long as that path exists in `git`, you
can rebuild from source in minutes, even if every `.sapp` file in the
repo somehow disappeared.

**What "deprecated" looks like in practice:** the Rust function gets a
one-line `#[deprecated(note = "use the pblang implementation in
stdlib/compiler/<file>.sapp — kept as bootstrap fallback")]` and stays
in the binary. CI fails any PR that removes a `#[deprecated]` reference
implementation without explicit `--break-bootstrap-fallback` approval.

---

## Layer 2 — Differential testing (the drift detector)

This is the load-bearing piece. **Every time you move code from Rust
to pblang, you add a paired test that drives both implementations with
the same input and asserts the outputs agree.** When the pblang version
drifts (or, equally important, when a language feature the pblang
version depends on changes its semantics in Rust), the test goes red
and tells you exactly where.

Concrete patterns:

- **Parser drift detector** — `tests/selfhost_drift.rs` in this repo
  drives `stdlib/compiler/selfhost_frontend.sapp::frontend_parse_full`
  on a corpus of source snippets and asserts the pblang-side AST has
  the same statement count and shape as `pblang_core::parse_strict`.
  This is the foundation; expand the corpus as new language features
  land. If a snippet parses in Rust but the pblang parser misses it,
  the diff is small and immediately actionable.

- **Formatter drift detector** — when `pblang fmt` moves to a pblang
  implementation, the test must format ~50 representative source files
  with both the Rust formatter and the pblang formatter and assert
  byte-identical output. Any non-trivial formatter change has to
  update both sides.

- **Type-checker drift detector** — when the typechecker moves, run
  the entire `tests/semantics/` corpus through both implementations
  and assert they produce the same diagnostic set (by message and
  span). Diagnostic ordering differences are allowed; *set* equality
  is required.

The pattern in pseudo-code:

```rust
#[test]
fn parser_drift_smoke() -> Result<(), Box<dyn std::error::Error>> {
    for snippet in CORPUS {
        let rust_ast = pblang_core::parse_strict(snippet)?;
        let pblang_ast = run_pblang_script(&format!(
            "let f = import(\"stdlib:compiler/selfhost_frontend.sapp\");
             let r = f.frontend_parse_full({:?});
             print json_stringify(map{{stmts: len(r.ast.stmts)}});",
            snippet,
        ));
        let pblang_stmt_count = pblang_ast["stmts"].as_u64().ok_or("missing stmts")?;
        assert_eq!(rust_ast.stmts.len(), pblang_stmt_count);
    }
    Ok(())
}
```

The trick is keeping the comparison **structural, not byte-equal**:
the two implementations build different AST representations (Rust
uses typed enums, pblang uses maps), so the test must extract the
salient properties (statement count, expression kinds, span ranges)
and compare *those*.

---

## Layer 3 — Verified `.pbck` artifacts (partial)

**Current status (2026-07-19):** PBCS v2 has a bounded reader/writer, v1 reader
compatibility, strict structural validation, and bounded loader execution. The
opt-in module cache verifies toolchain/source keys, metadata, digest, size, and
PBCS structure before reuse. However, `stdlib/compiler/_pbck/` does not exist in
this checkout, and no deterministic checked-in snapshot/freshness CI gate has
been implemented. The runtime cache is an optimization, not a bootstrap backup.

The target completed layer stores self-hosted modules in two places in `git`:

1. **Source form**: `stdlib/compiler/*.sapp`
2. **Compiled form**: `stdlib/compiler/_pbck/*.pbck` (binary chunks)

The compiled files must be generated deterministically, never edited by hand,
and verified against source and toolchain fingerprints. This remains follow-up
work; do not use cache files under `~/.pblang/cache/` as release artifacts.

**Why this matters:** suppose the pblang parser breaks badly enough
that `stdlib/compiler/parser.sapp` will no longer parse itself. The
Rust frontend rebuilt from `git` can still load the pre-compiled
`.pbck` for `parser.sapp` from disk — no parser needed at all to load
a chunk file — and continue running. You diagnose the breakage with a
working `pblang` in hand, not from a cold boot.

**Required CI gate:** a workflow that (a) deletes every `.pbck` under
`stdlib/compiler/`, (b) rebuilds them from source, (c) confirms the
new `.pbck` files match what was committed. If they diverge, the PR
fails: either the source changed without re-checking the chunk, or
the chunk was hand-edited.

**Storage cost:** measure the generated snapshot set before adoption; no current
size is claimed because the checked-in set does not yet exist.

**Local refresh gate (2026-07-29):** `tools/pbbootstrap_refresh.sapp` regenerates
one verified PBCS snapshot for every source module under `stdlib/compiler/`,
plus `dist/bootstrap/pblang-selfhost-compiler.pbck` for the PBLang-native
compiler CLI. Per-module snapshots deliberately preserve module namespaces:
flattening the compiler into a shared global namespace is unsafe because
backend modules intentionally use the same opcode constants. The tool also
emits a real x86-64 ELF probe through `stdlib/native/codegen_x64.sapp`, executes
it, and writes a digest-bound manifest. `tools/pbselfc.sapp` exposes the
source-to-PBCS workflow. This closes the reproducible local snapshot step, but
not the checked-in freshness CI criterion above. The 32 MiB host executable
remains the bootstrap VM because the native x86-64 backend does not yet cover
the complete compiler CLI/runtime; the refresh tool records that boundary
rather than replacing the host with a non-equivalent probe. The bundle resolver
normalizes canonical `stdlib:` imports to traversal-free paths below `stdlib/`,
ignores apparent imports inside comments and strings, and fails closed for
malformed or escaping module URIs.

---

## Layer 4 — The C-transpiler as the universal floor

`pblang transpile → C → cc/gcc/clang → native binary` is the realistic
self-hosting path the Future_plans roadmap recommends. It's also the
**recovery path of last resort**: even if both the Rust frontend and
the pblang frontend are broken on a given codebase, the C transpiler
takes pblang source and produces a `.c` file that a C compiler — older
than either of these projects, available on every machine — can build.

**Discipline:** keep `pblang-ctranspiler` building and passing its
existing tests on every push. It does not have to be the *primary*
build path; it has to be *a* build path that always works. One
integration test per release that does:

```
pblang transpile stdlib/compiler/parser.sapp > parser.c
cc -O2 parser.c -o parser
./parser <some input>
```

is enough to prove the floor is still under us.

---

## Layer 5 — Monthly fresh-checkout CI

A scheduled CI job (monthly is sufficient — daily is overkill) that:

1. Spins up a clean container with only `rustc`, `cargo`, and a C
   compiler.
2. `git clone` the repo at the current `main`.
3. Runs `cargo build` + the full test suite from scratch.
4. Runs `pblang transpile` on a tiny corpus and compiles the C output.

This catches the silent failure mode where a Rust-side cleanup
accidentally removed something the bootstrap path needed, but tests
pass because the local dev machine has cached state. The fresh-clone
build either works — or you have a known week-window to fix it before
it becomes a real emergency.

---

## What this means for "move X from Rust to pblang" PRs

A PR that ports a piece of compiler/runtime code from Rust to pblang
ships with **all of these** or it doesn't merge:

1. The Rust implementation is moved to `legacy_*` / `reference/`, not
   deleted. A `#[deprecated]` attribute names the pblang replacement.
2. A new differential test asserts the two implementations agree on a
   representative corpus.
3. If the moved code is a compiler module, the pre-compiled `.pbck`
   for the new pblang module is committed alongside the source.
4. The `pblang transpile`-to-C smoke test still passes.
5. Documentation in `PrjAnalysis.md` / `Future_plans.md` notes which
   layer is now in pblang and which Rust function is its oracle.

Following this discipline, the worst-case recovery from a broken
pblang frontend is:

- *Minutes:* `git revert` the offending PR.
- *Hours:* boot from the checked-in `.pbck` chunks and diagnose live.
- *Days:* rebuild from the pristine Rust reference path and re-port.
- *Weeks (this never actually happens, but it's the floor):*
  transpile to C and use the C output as a new starting point.

You are never stranded.

---

## Full-parser drift status

The differential test `tests/selfhost_drift.rs` includes a
`selfhost_drift_full_parser_matches_statement_count` case that drives
`stdlib/compiler/parser.sapp` (not just the lightweight whitespace
analyzer in `frontend_analyze`) and compares its statement count
against the Rust reference parser. Its historical `#[ignore]` text says the
pblang parser takes more than 60 seconds per snippet; that measurement is now
obsolete. On 2026-07-14 the complete five-snippet oracle passed in 0.96 seconds.

The default pblang suite now runs
`tests/semantics/selfhost_full_parser_drift_test.sapp`, which checks the same
five sources with strict statement counts. Parser AST constructors use direct
single-allocation map literals; the exported `make_node` helper remains for
backward compatibility but is no longer on the parser's internal hot path.

When either parser changes, run both the default pblang drift test and the Rust
oracle explicitly with `cargo test --test selfhost_drift
selfhost_drift_full_parser_matches_statement_count -- --ignored --exact`. Any
output divergence remains a release gate. The remaining self-host performance
priority is interpreter execution (calls/scopes/builtin dispatch), not parsing.

## What this does **not** solve

This document is about *not losing the ability to build*. It does not
address:

- **Performance regressions** when a piece of compiler code moves from
  Rust to pblang (likely; profile and decide case-by-case).
- **Capability surface changes** — moving FFI / `unsafe` code from
  Rust to pblang has security implications that need their own audit.
- **Multi-stage bootstrap reproducibility** (the
  `bootstrappable.org`-style "can a person rebuild this from a
  hand-typed seed?" question). That's a separate, deeper problem;
  we're solving the practical "can I keep shipping?" problem here.

When those concerns matter, treat them as separate workstreams.

## Strict frontend compilation boundary (2026-09-07)

Native colon-syntax match guards are emitted after pattern success for all
pattern kinds, including literals. Literal mismatch skips the guard; false
guards skip the body; both continue to the next arm. Matching guard exceptions
propagate normally. This corrects previously ignored literal guards without
changing opcodes or the bytecode format.

The existing enum `value.__variant` coverage check only credits unguarded arms
or literal boolean-true guards. Conditional bind/wildcard arms are not catchalls,
and conditional literals do not guarantee their variant. Calls, truthy values
and complementary guards are not evaluated/proved during checking. Add an
unguarded arm or `else` where guaranteed coverage is required. Guard and body
errors are still checked even when a fallback establishes coverage. Other
non-enum match expressions remain outside this enum-specific check.

Compatibility: guarded enum matches formerly misclassified as exhaustive may
now fail checked compilation. Literal-guard behavior now follows the source
condition instead of ignoring it. Tests use parenthesized call-expression
bodies to avoid the existing colon-method-call parser ambiguity. This slice
does not fix that ambiguity or establish whole-language/retained-host parity.

Native match lowering now gives each arm its own lexical scope, matching the
typechecker's binding rule. A rejected arm's name is unavailable in later arms
and the default; those resolve the enclosing binding instead. The scrutinee
occupies an unnamed internal slot, not a synthetic source identifier such as
`__match_scrut`. Reading that user-defined name therefore remains ordinary
lexical lookup. Slots still increase monotonically: scope exit changes only
name resolution, preserving the VM's existing by-value closure capture.

`selfhost_match_scope` compares plain and safely optimized bounded execution
for defaults, sibling/nested matches, repeated/distinct/Unicode binders, outer
locals and helper-like names, and selected/fallback closures. These checks
close the identified native match temporary-capture and inter-arm binding
leaks. They do not certify other compiler-generated temporaries, reflection,
arbitrary AST input, every backend, or general resource/safety guarantees.
The parser also treats `match (` as a call to the contextual identifier `match`;
the nested-subject regression uses the supported unparenthesized `match match`
form. Parenthesized scrutinee disambiguation is not repaired here.

The PBLang typechecker diagnoses proven scalar nil arithmetic with code
`nil-arithmetic`: binary `+`, `-`, `*`, `/`, `%` between a known nil and a
numeric type (or another nil), and unary `-nil`. It returns `any` for the
invalid expression after recording the error, avoiding a fabricated number
and cascading assignment errors. Both operand subtrees are still checked.
The checked frontend reports `ok=false`; its run APIs do not execute any part
of the rejected program. Inspectable bytecode remains part of the existing
type-error contract, not permission to run it.

Compatibility: these expressions were formerly accepted by the self-hosted
checker; they now fail checked compilation, including inside `try`. This
does not change runtime operators or prohibit custom reflected/forward map
operators. Existing string-addition inference, `any`, generic and optional
operand behavior is preserved, not certified correct by this fix. Optional
narrowing, all concrete operand combinations and complete retained-host parity
remain separate work. `**` is already lowered to `num_pow` and uses call
argument checking rather than the new binary rule.

One checked-token parser now feeds direct, file, guard and project compilation.
Unterminated strings/block comments become parse errors with their token line.
Recovery ASTs remain inspectable, but lexical/parser errors produce an empty
instruction chunk. File analysis skips type/semantic passes on rejected ASTs;
project name discovery ignores their recovered declarations.

Raw lexical validation precedes legacy guard rewriting, so rewrites cannot
erase unfinished tokens. Imported modules reject raw lexical errors before
dependency scanning, retain the existing empty-exports failure contract, and
validate processed syntax before cached bytecode lookup. Lint checks lexical
errors after the guarded rewrite too. This does not authenticate caches or
make the naive import scanner syntax-aware.

CLI and pipeline consumers must check `ok` before serialization/execution.
Type-only diagnostics intentionally retain inspectable bytecode with
`ok=false`. The legacy adapter now returns selected values directly, removing
the synthetic `_guard_val` nil-versus-value assignment mismatch. Native
colon-syntax match is unchanged; general legacy semantics are not certified.
The escaped-quote/semicolon corruption is repaired in the legacy string walkers.
One exclusive-end scanner preserves quoted source slices in scrutinees, guards,
arms, defaults and keyword/brace searches; identifier substitution does not
rewrite quoted literals. Unterminated spans abort the affected transformation.
The A5 raw/processed lexical gates remain mandatory and unchanged.

The legacy rewriter is still partial, but `has_guards` is now only a coarse
fast-path hint. Each individual match must contain complete supported guarded
arms before any lowering is emitted. Unrelated native matches stay intact even
when separators occur in strings, comments or another expression. Discovery
skips comments and respects complete keyword boundaries; bind replacement no
longer splits neighboring Unicode/combining-mark identifiers.

The admitted legacy subset is `case ASCII_IDENT if CONDITION ~ VALUE;` (or `#`),
with an optional final `else: VALUE;`. Names must not be tokenizer keywords.
Quoted strings and balanced parentheses/brackets (up to 64 levels) are scanned
without confusing their contents with arm separators. Colon-form cases,
literal/destructuring patterns, comments within expressions, nested braces,
misplaced separators and incomplete arms decline the entire outer transform.
This intentionally narrows the older permissive adapter, which could silently
emit partial or always-true arms. Leaving source unchanged is not a successful
compile: callers must still check frontend diagnostics. Nested candidates may
be processed independently; general nested legacy support is not established.
Each arm binds its identifier in a statement-if scope shared by its guard and
value. The condition is retained verbatim: a field such as `item.n` is not
rewritten when `n` is the binding. Successful arms return from the single outer
IIFE; guards run in order, the subject runs once, and only the selected value
runs. The optional default sees the outer scope; no match/default returns nil.
Bare braces are not used as scopes here: the parser lowers value blocks into
another IIFE, which would intercept the return.

The subject helper name is deterministic and absent from identifier tokens in
the admitted match span. Tokenization failure or unterminated tokens decline
the transformation; the candidate search is capped at token count plus one.
This covers lexical capture, not reflective/eval lookup. Tokenization still
allocates proportionally to input and does not establish a hard memory limit.
The existing function-literal return type remains `any`; this repair is not
precise result-union inference or general expression hygiene.

`guard_preprocessor_activation` checks exact native-text preservation, actual
branch selection with independent legacy siblings in either order, misleading
separators/keywords, malformed inputs and the 64/65 grouping boundary. It does
not establish a whole-source heap or wall-clock limit.

`guard_preprocessor_strings` compares fixed synthetic values in the bounded VM,
including escapes, separator-like text and literal identifier preservation.
It now requires successful checked compilation before executing bytecode.
`guard_preprocessor_results` additionally covers numeric/string/boolean/array
results, nil/defaults, bound values, outer shadowing, helper collisions,
side-effect order, exceptions, independent matches and the optimized frontend.
`guard_preprocessor_bootstrap` checks all five focused bootstrap guard checks;
the first two now validate compilation and bounded execution instead of old
generated helper names. These are targeted checks, not a complete language or
physical-platform certification.

Focused tests: `selfhost_compile_lexical`, `selfhost_project_lexical`.
Their static malformed fixtures are input text, not runnable/lint-clean
programs. The project fixture has two files and may start two compile workers.
The targeted harness bounds time, address space and captured output.
Whole-file reads, token allocation and preprocessing before byte admission on
bounded file-run routes remain separate resource issues.
