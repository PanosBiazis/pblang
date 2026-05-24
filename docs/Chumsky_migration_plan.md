# Chumsky 0.8 → 0.9+ → 1.0 migration plan

> **Status (final): chumsky 1.0.0-alpha.8 is shipped workspace-wide.**
> The migration is complete. `cargo check -p pblang-core` is clean,
> `cargo test --test semantics --test modules --test sandbox
> --test stdlib_parse` is 161+16+10+1 green (with `match_guard_pass`
> un-ignored and passing). The single remaining `#[ignore]` in the
> semantics suite is the pre-existing `env_set` security-audit ignore,
> unrelated to chumsky.
>
> Migration arc (this document is preserved end-to-end as a record):
> 0.8 → 0.9.3 was API-compatible (zero `parser.rs`/`lib.rs` changes).
> 0.9.3 → 1.0-alpha.8 was the real cliff — one function-signature
> change (`Parser<char, T, …>` → `Parser<'src, &'src str, T, extra::Err<…>>`)
> collapsed the error surface from 1,085 → 113. The remaining 113 were
> mechanical: `text::ident/int/digits` now return `&str` (mapped to
> `String`), `.map_with_span(...)` became `.map_with(...)` with
> `e.span().into_range()`, `.foldl` flipped argument order, `filter`
> became `any().filter`, `Simple::custom` became `LabelError::
> expected_found`, and `parse_recovery` was replaced by
> `parser().parse(...).into_output_errors()`.
>
> A small `ParseError { message, span, found }` owned struct now sits at
> the `pblang-core` boundary because chumsky 1.0's `Simple<'src, char>`
> borrows from the input and can't cross function boundaries. CLI and
> LSP diagnostics keep span/line/column underlining via `e.span` +
> `underline_span`/`range_from_span`.
>
> ---
>
> **Historical Status update (Checkpoint 0 result):** chumsky 0.9.3 had
> been shipped workspace-wide. The 0.8 → 0.9.3 migration was
> **API-compatible for our usage** — zero changes to `parser.rs` or
> `lib.rs` were required. The Cargo.toml bump alone (workspace dep,
> pblang-core, pblang-lsp all unified on `workspace = true`) compiled
> clean, all 6 LSP tests + 160 semantics + 10 sandbox + 16 modules +
> 1 stdlib_parse passing.
>
> **SP1-gamma (match guards) is FIXED on chumsky 0.9.3.** The fix was
> grammar-shaped, not framework-shaped: a restricted `guard_expr`
> sub-parser declared with `Recursive::declare()` (alongside `expr` /
> `stmt`) that bottoms out at `<atom> [cmp_op <atom>]` — only `.or_not()`,
> no `.padded().repeated()` — so the chumsky-0.8/0.9 cascade past the
> guard's trailing `~`/`#` never runs. A second fix corrected a
> longstanding `ws1`-before-`if` bug in the guarded-arm prefix (since
> `ident.padded()` already consumed the trailing whitespace, requiring
> ws1 again was unreachable).
> Result: `tests/semantics.rs::match_guard_pass` is un-ignored and
> passes; no change to the BindGuard AST, compiler, or VM is needed.
>
> The 6-checkpoint plan below is preserved for the eventual **chumsky
> 1.0** migration, which *will* have the wall-of-breakage this doc
> originally anticipated.
>
> **Measured 1.0-alpha.8 failure surface** (Checkpoint 0 for that
> jump): `cargo check -p pblang-core` produces **1 074 errors**.
> Categories:
> - 580× `char: chumsky::input::Input<'_>` not satisfied (every parser
>   combinator needs the new `Parser<'a, I, O, E>` trait shape)
> - 271× `ValueInput` bound failures
> - 146× `StrInput` bound failures
> - 32× `map_with_span` no-method (rename to `map_with`)
> - 12+ type-inference failures from the new generic surface
>
> That's the genuine 1.5–2.5 day migration the original plan estimated.
> The fix for SP1-gamma is **not** in that bump — it's grammar
> restructure work that's independent of chumsky version.

This document is the **starting point** for migrating pblang's parser
from chumsky 0.8 to the current stable chumsky (0.9+). Migration is the
single biggest roadmap unblocker today — it lifts:

* ~~The **match-guard parser cascade** that's been documented as `SP1-gamma`
  for months (`tests/semantics.rs::match_guard_pass` is `#[ignore]`d
  because of this).~~ **Fixed on 0.9.3** via a restricted `guard_expr`
  sub-parser; see the status block above. Listed here for historical
  context only.
* The **single-error-per-file ceiling** — chumsky 0.8's `parse_recovery`
  collapses any partial failure into a `None` AST. 0.9+ has per-parser
  `recover_with` so a parse can report multiple errors and still produce
  a usable partial AST.
* Future grammar work (custom operators, structs/enums when those land)
  benefits from the cleaner 0.9 trait surface.

This is **multi-session work** — likely 1–2 dedicated days for someone
familiar with chumsky 0.9, longer otherwise. This doc breaks it into
checkpoints so progress is visible and bisection-friendly.

## Current surface (the scout)

| | Count |
|---|---|
| `crates/pblang-core/src/parser.rs` | **2 265 lines** |
| `crates/pblang-core/src/lib.rs` (chumsky-aware boundary) | 772 lines (1 entry point `parser()`, 1 `parse_recovery` site) |
| `just(…)` calls | 206 |
| `.padded()` calls | 129 |
| `.then_ignore(…)` | 105 |
| `.map_with_span(…)` | 69 |
| `.choice(…)` blocks | 35 |
| `.or_not()` | 25 |
| `.repeated()` | 25 |
| `.delimited_by(…)` | 15 |
| `.boxed()` (manual trait-object hint) | 2 |
| `recursive(…)` blocks | 2 |
| `text::ident/int/digits/keyword` | 0 (we hand-roll lex) |
| `one_of/none_of/filter` | 0 |
| `parse_recovery` call sites | 1 (`lib.rs::parse_with_polyglot`) |

The parser is **medium-sized** by chumsky-grammar standards — moderate
risk, fully contained in one file plus a thin boundary in `lib.rs`.

## API differences that will hit us

| Area | 0.8 (today) | 0.9+ |
|------|-------------|------|
| `Parser` trait | `Parser<C, O, Error = …>` | `Parser<'a, I, O, E>` — input type and "extra info" became explicit generics |
| Error type | `Simple<char>` | `extra::Err<Simple<char>>` (wrapped in an extras tuple) |
| Span / context in maps | `.map_with_span(\|x, sp\| …)` | `.map_with(\|x, extra\| …)` — `extra.span()` replaces the closure arg |
| Recovery | `parser.parse_recovery(src)` returns `(Option<O>, Vec<Error>)` | `parser.recover_with(skip_then_retry_until(…))` on individual sub-parsers; the top-level `.parse(input)` returns a `ParseResult` with both partial output **and** errors |
| `just("xy")` for multi-char strings | works | works — the `just_str` helper isn't needed |
| `text::ident()` | returns `String` | returns `&str` (zero-copy); we don't use this, but if we add it, plan accordingly |
| `boxed()` | works | still works; sometimes needed less because of better inference, sometimes needed more (the new trait surface needs hints in tight recursion) |
| Const traits / `'static` bounds | implicit | the new trait makes the input lifetime visible — some closures need `'static` or `move` |
| `recursive` | closure takes `Recursive<Output>` | closure takes `Recursive<dyn Parser<…>>`; trait-object signature is more explicit |

## Migration in 6 checkpoints

Each checkpoint is a separate PR / commit. The whole suite must stay
green at every checkpoint, with `match_guard_pass` un-ignored only at the
last one.

### Checkpoint 0 — Pin the toolchain & branch

* Bump `Cargo.toml`: `chumsky = "0.9"` in `crates/pblang-core/Cargo.toml`.
* `cargo update -p chumsky`.
* Expect a wall of compile errors from `parser.rs` + `lib.rs`. **Don't
  fix them yet** — commit the failing baseline so the diff in
  Checkpoint 1 isolates only the type-surface noise.
* Document the failure count in the commit message (typically dozens of
  `unresolved import` / `the trait bound … is not satisfied`).

### Checkpoint 1 — Quiet the type-surface

The new `Parser<'a, I, O, E>` trait surfaces in every function signature
and most local bindings. This checkpoint is **mechanical, no logic
changes**:

* `use chumsky::prelude::*` stays.
* Change every `impl Parser<char, T, Error = Simple<char>>` to
  `impl Parser<'a, &'a str, T, extra::Err<Simple<char>>>` (or factor
  out a `type Out<T> = extra::Err<Simple<char>>` alias).
* Where the type can't be inferred (recursive `boxed()` parsers),
  use a `Boxed` alias and hint explicitly.
* In `lib.rs`, change `parser()` to take a lifetime parameter or to
  capture `'static` (the `&str` input means the parser holds the source's
  lifetime).
* `.parse(src)` now returns `ParseResult<O, E>` not `Result<O, Vec<E>>`.
  Adjust the four call sites in `lib.rs` accordingly:
  - `parse` / `parse_strict` / `parse_with_polyglot_strict` / `parse_with_polyglot`.

Goal: workspace compiles and `cargo test --workspace` passes (or fails
only on tests that depend on the `parse_recovery` partial-AST behaviour,
which we'll fix in Checkpoint 4).

### Checkpoint 2 — Rename `.map_with_span` → `.map_with`

69 sites. Mechanical:

```diff
-.map_with_span(|x, span| Foo { x, span: span.into() })
+.map_with(|x, e| Foo { x, span: e.span().into() })
```

If your closure ignored the span (`|x, _sp| …`), just rename to
`.map(|x| …)` — no extra generic noise.

### Checkpoint 3 — Re-wire the few combinator name changes

Run the workspace test suite. Anywhere chumsky 0.9 renamed a method
(rare but happens — e.g. some `then_ignore`/`ignore_then` overloads
have new shorter forms), fix and recompile. This is small; budget
30 minutes.

### Checkpoint 4 — Replace `parse_recovery` with `.recover_with` placements

This is the **substantive part**. `parse_recovery` was a single
top-level call that asked "give me whatever you can"; 0.9+ wants each
recovery placement made explicit on the sub-parser that needs it.

Concrete placements for pblang's grammar (in order of importance):

1. `stmt` — the top-level recovery boundary. Wrap with
   `.recover_with(skip_until([';'], || Stmt::error()))` so one bad
   statement doesn't kill the file.
2. `match_arm` — recover at `,` or `;` so one bad arm doesn't kill the
   whole `match`. This is what fixes `SP1-gamma`.
3. `expr` (top-level inside `( )` and `[ ]`) — bound recovery to the
   closing delimiter.
4. Replace the `parse_with_polyglot` fallback ladder in `lib.rs`:
   the `match parser().parse(masked.as_str())` `Ok` arm stays;
   the `Err` arm becomes "parse_recovery_ish" — collect partial AST
   and the errors from `ParseResult::into_output_errors()`.

The `Stmt::error()` ctor doesn't exist yet — add a `StmtKind::Error`
variant to `pblang-core::Stmt` so the parser can produce well-typed
placeholders the type-checker can skip.

### Checkpoint 5 — Un-ignore `match_guard_pass`; add multi-error tests

* ~~Remove `#[ignore]` from `tests/semantics.rs::match_guard_pass`.~~
  **Done on 0.9.3 ahead of the 1.0 migration** — the restricted
  `guard_expr` sub-parser made it possible without 1.0's new recovery
  primitives. The test is currently un-ignored and green.
* Add a `tests/parse_recovery.rs` test target with three cases:
  - "one bad stmt doesn't hide the rest" — file with a broken `let`
    followed by valid code; assert the parser surfaces 1 error AND
    produces a partial AST containing the valid bits.
  - "one bad match arm doesn't kill the match" — feeds the test pattern
    from the SP1-gamma fixture.
  - "unclosed paren reports balanced error count" — checks the
    `delimited_by` recovery placement.
* Update `docs/CURRENT_STATUS.md` and `docs/Tasks.md` to reflect
  SP1-gamma resolved.

### Checkpoint 6 — Cleanups

* Remove obsolete `#[allow(deprecated)]` annotations.
* Run `cargo clippy --workspace -- -D warnings` and fix any new lints
  the 0.9 surface introduced.
* Delete the SP1-gamma rationale comments in `parser.rs` (now obsolete).
* Update `docs/Future_plans.md` v0.7 section — the match-guard separator
  fix is shipped.
* Bump `pblang-core` minor version; add a CHANGELOG entry.

## Risk register

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Combinator-chain inference breaks in 3–5 tight spots | Medium | Use `boxed()` plus an explicit type alias; chumsky 0.9 docs have a "complex types" section |
| `recursive(…)` closures need re-shaping | Medium | Both recursive sites are well-localised (expr + stmt body); rewrite each as a single commit |
| Some `padded()` semantics differ (0.9 trims more aggressively) | Low | The semantics test suite catches whitespace regressions |
| `.parse()` return shape change cascades into `lib.rs` callers | Low | 4 call sites; rewrite once |
| A subtle precedence regression from re-ordered `choice(…)` | Low | The 160-case `semantics.rs` suite + `parser_ast.rs` golden ASTs catch this |

## Estimated effort

| Checkpoint | Effort |
|------------|--------|
| 0. Pin & commit failing baseline | 30 min |
| 1. Quiet type surface | 3–5 h |
| 2. `map_with_span` rename | 1–2 h |
| 3. Combinator name changes | 30 min |
| 4. `.recover_with` placements + Stmt::Error | 4–6 h |
| 5. Un-ignore SP1-gamma + new tests | 1 h |
| 6. Cleanups | 1 h |
| **Total** | **~1.5–2.5 days dedicated work** |

## What I would do if you said "do it"

I'd start with Checkpoint 0 + 1 in one session (the failing-baseline
commit + the type-surface fix), verify the workspace still passes,
ship it. Then 2 + 3 in a second session. Then 4 alone (the substantive
part), then 5 + 6 to close.

Doing all of it in one session is technically possible but the build
cycles are ~5 min each — splitting across commits is much faster to
verify and to roll back from if any single checkpoint introduces a
regression.

## References

- chumsky 0.9 release notes: <https://github.com/zesterer/chumsky/releases>
- chumsky 0.9 docs: <https://docs.rs/chumsky/0.9>
- chumsky 0.9 "moving from 0.8" guide:
  <https://github.com/zesterer/chumsky/blob/main/MIGRATION.md>
- pblang's existing `SP1-gamma` rationale: search `parser.rs` for
  "SP1-gamma" comments and `tests/semantics.rs::match_guard_pass`
  for the failing fixture.
