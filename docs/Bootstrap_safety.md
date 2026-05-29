# Bootstrap safety — surviving the move from Rust to pblang

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
| **3. Pre-compiled `.pbck` artifacts** of every self-hosted module live in `git`. | A working binary today even if the source-form pblang parser broke yesterday. | One CI step + a few MB in the repo. |
| **4. `pblang transpile → C → cc`** as the universal floor. | If both Rust *and* pblang fall over, C still bootstraps. | The transpiler already exists; just keep it working. |
| **5. A monthly "fresh-checkout build" CI job.** | Catches drift in the Rust reference path itself. | One scheduled workflow. |

Each layer survives the failure of the layers above it. You'd have to
lose **all five** to be in a recovery scenario you can't dig out of.

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
fn parser_drift_smoke() {
    for snippet in CORPUS {
        let rust_ast = pblang_core::parse_strict(snippet).unwrap();
        let pblang_ast = run_pblang_script(&format!(
            "let f = import(\"stdlib:compiler/selfhost_frontend.sapp\");
             let r = f.frontend_parse_full({:?});
             print json_stringify(map{{stmts: len(r.ast.stmts)}});",
            snippet,
        ));
        assert_eq!(rust_ast.stmts.len(), pblang_ast["stmts"].as_u64().unwrap());
    }
}
```

The trick is keeping the comparison **structural, not byte-equal**:
the two implementations build different AST representations (Rust
uses typed enums, pblang uses maps), so the test must extract the
salient properties (statement count, expression kinds, span ranges)
and compare *those*.

---

## Layer 3 — Checked-in `.pbck` artifacts

Self-hosted compiler modules live in two places in `git`:

1. **Source form**: `stdlib/compiler/*.sapp`
2. **Compiled form**: `stdlib/compiler/_pbck/*.pbck` (binary chunks)

The compiled `.pbck` files are produced by `pblang compile` and checked
in. They are **never edited by hand** — a pre-commit hook regenerates
them from source. They are intentionally a small additional commit:
the `.pbck` format is compact (a few KB per module).

**Why this matters:** suppose the pblang parser breaks badly enough
that `stdlib/compiler/parser.sapp` will no longer parse itself. The
Rust frontend rebuilt from `git` can still load the pre-compiled
`.pbck` for `parser.sapp` from disk — no parser needed at all to load
a chunk file — and continue running. You diagnose the breakage with a
working `pblang` in hand, not from a cold boot.

**CI gate:** a workflow that (a) deletes every `.pbck` under
`stdlib/compiler/`, (b) rebuilds them from source, (c) confirms the
new `.pbck` files match what was committed. If they diverge, the PR
fails: either the source changed without re-checking the chunk, or
the chunk was hand-edited.

**Storage cost:** the entire `stdlib/compiler/` tree compiled today
would be on the order of 30-80 KB total. Negligible.

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

## Known gap: pblang-parser perf

The differential test `tests/selfhost_drift.rs` includes a
`selfhost_drift_full_parser_matches_statement_count` case that drives
`stdlib/compiler/parser.sapp` (not just the lightweight whitespace
analyzer in `frontend_analyze`) and compares its statement count
against the Rust reference parser. It is currently `#[ignore]`-marked
because the pblang-written parser, running on the VM interpreter,
takes >60 seconds per snippet — too slow to default-on in CI.

This is a **perf** problem, not a correctness one. When one of these
lands the `#[ignore]` should be removed:

1. The pblang JIT becomes the default execution path for stdlib
   modules, so the self-host parser runs as native code.
2. The self-host modules are loaded from pre-compiled `.pbck` chunks
   (Layer 3 above) instead of being re-parsed every test invocation.
3. The pblang parser implementation in `stdlib/compiler/parser.sapp`
   gets a non-trivial algorithmic improvement (the current
   implementation has at least one quadratic loop).

Until then, run the heavy drift check manually with
`cargo test --test selfhost_drift -- --ignored` whenever you touch
either parser, and treat any output divergence as a release blocker.

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
