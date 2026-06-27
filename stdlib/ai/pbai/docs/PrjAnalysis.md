# pbAI — Project Quality Analysis (2026-06-09)

> Full-project audit: errors · warnings · bugs · vulnerabilities · sensitive data · partial implementations · quality · comparison against ML stacks in other languages.
> Methodology: a complete read of every tracked `.sapp` file (stdlib/, server/, tools/, tests/, demo/) plus live test execution with both the installed `pblang` (0.5.7, `~/.local/bin`) and a freshly built `pblang` from `pl-dev/pblang@6b2383e`. Every finding cites file:line and was verified by execution or reading, not just pattern-matching. Project size: ~5,077 lines of pblang across 24 files (stdlib/ 3,144 · tools/ 480 · tests/ 491 · demo/ 30), 0 Rust.

---

## 1. Verification baseline — original audit snapshot, **resolved below**

This section is kept as the 2026-06-09 audit snapshot. The current Linux state after §23 is green: `pblang test tests/semantics --allow-all` reports **14 passed / 0 failed / 0 ignored**.

| Run | Result |
|---|---|
| `pblang test tests/semantics --allow-all` (installed 0.5.7) | **4 / 6 / 0 — FAILED** |
| same, with freshly built pblang (`1b2ea33`+ path-fallback feature) | **6 / 4 / 0 — FAILED** |
| failing with fresh binary | `pbai_accel_test`, `pbai_brain_test`, `pbai_tokenizer_test`, `pbai_test` — **all one root cause (§2 bug #1)** |
| extra failures with installed binary | `pbai_assets_test`, `pbai_max_test` — the installed pblang predates the manifest `paths = [...]` fallback (commit `1b2ea33`), so `stdlib:` imports resolve only against `../pblang/stdlib`, which doesn't exist in this checkout layout → **reinstall pblang** |

Two environment notes discovered while testing: (a) both binaries report "0.5.7", so the version string can't distinguish a stale install — bump the version or add a build hash; (b) `pbai_tokenizer_test` spends ~3m52s CPU before failing (it trains before hitting bug #1), so suite "hangs" are really slow-training-then-crash.

## 2. Bugs

### #1 CRITICAL — `env_get()` of an unset variable returns nil; `try/catch` cannot save it (`stdlib/pbai_accel.sapp:38-39`)

```pblang
let raw = try env_get("NUMBER_OF_PROCESSORS") catch "";  // env_get returns nil, does NOT throw → catch never fires
let c = _parse_int(raw);                                  // len(nil) inside _parse_int → runtime error
```

Verified live: `env_get("NUMBER_OF_PROCESSORS")` prints `nil` on this Linux box, then `_parse_int(nil)` throws at `len(s)` (`pbai_accel.sapp:24`). `NUMBER_OF_PROCESSORS` is a Windows variable; on Linux it is unset, so **every code path through `accel.detect()` crashes — 4 of 10 test files fail, and any pbai entry point that touches acceleration is broken on Linux**. Introduced by the multi-core training commit `e1a2cf0`. Fix is one line:

```pblang
let raw = env_get("NUMBER_OF_PROCESSORS");
if raw == nil { raw = ""; }
```

(Also worth a Linux-native fallback: read core count via `nproc`/`/proc/cpuinfo` before defaulting to 4.) Note the error's stack trace misattributes frames to the *test* file — the known pblang cross-import line-misreport quirk — which made this look like 4 unrelated failures.

### #2 HIGH — unguarded softmax division (`stdlib/pbai_lm.sapp:227`)
`p[v3] = p[v3] / sum` with no `sum > 0` guard. The max-subtraction makes `sum ≥ 1` for finite logits, but once any weight goes NaN/Inf (e.g. LR too high) the NaN cascades silently through `generate()`/`learn()`. Add a guard with a uniform-distribution fallback.

### #3 HIGH — division by `len(records)` without empty-check (`stdlib/pbai_max.sapp:118`)
`let per_record = max_examples / len(records);` — crashes if `curated_records()` ever returns empty. Currently mitigated by hardcoded records, still a latent trap on the Max training entry path.

### #4 MEDIUM — top-k renormalization can divide by a zero sum (`stdlib/pbai_lm.sapp:621`)
The second normalization pass after top-k truncation lacks the `s2 > 0` fallback the first pass has → NaN sampling in `generate_nano_sampled()` for adversarial temperature/top-k combos.

### Verified-correct (read in full, math checked)
`_tanh` (clamped, stable, correct formula, `pbai_lm.sapp:89-94`) · backprop gradients (`dlogits = p − onehot`, correct tanh derivative chain, `:232-289`) · Xavier/Glorot init scaling (`:148-155`) · exponential LR annealing (`:300-321`) · char tokenizer round-trip. The model math is sound; the defects are missing guards, not wrong formulas.

## 3. Vulnerabilities

| Finding | Severity | Detail |
|---|---|---|
| Server path handling trusts the stdlib (`tools/pbai_server.sapp:141-157`) | MEDIUM | every request path is passed to `http.static_file_response("website", rel)` with no local `..` / absolute-path rejection; containment rests entirely on the pblang builtin. Defense-in-depth: reject `..` and leading `/` in the handler. Mitigated by the `127.0.0.1:8088` default bind. |
| Website JS | CLEAN | `pbai-web.js` uses `textContent` exclusively — no HTML injection surface. |
| No auth on the server | by-design | localhost demo server; document that it must not be bound publicly. |

## 4. Sensitive data

**No secrets** (no keys/tokens/passwords anywhere in tracked files or history). **Personal-topology leak**: the user's drive layout (`/opt/app`, `./local_media phone MTP path) is hardcoded in `docs/pbAI.md:372-376`, `docs/pbAI_source_audit.md:70-78`, and `tools/pbai_feed.sapp:24-30` (drive roots in code, not config). Redact/parameterize before any public release. Also: `target/pbai-max-test/**` checkpoint JSONs are **git-tracked build outputs** (show as modified in `git status`) — `target/` should be ignored; keep only `checkpoints/pbai-max/` (220 KB, fine) as the published artifact.

## 5. Partial implementations / not implemented

| Item | Reality |
|---|---|
| `pbai_max.sapp` overall | **facade/orchestrator, not a standalone model** — all heavy math delegates to `stdlib:llm_*.sapp` living in the *pblang* repo (`pbai_max.sapp:8-17`). Intentional per `docs/pbAI_externalization.md`, but the module doc should say so. |
| `stream_status()` (`pbai_max.sapp:657-688`) | hardcoded return map (`stats.emitted = 2`); no streaming-corpus code exists |
| `distillation_plan()` (`:672-679`) | static map; no Pro/Mini/Nano distillation implementation |
| `tool_call()`/`tool_catalog()` (`pbai.sapp:1011-1017`) | thin delegation to `stdlib:pb_mcp.sapp` (pblang repo); untested in this repo |
| Roadmap P1/P2/P3 (docs/pbAI.md) | only Nano tier shipped; no Mini/Pro; no agent plan→act→verify loop. The doc's status banner says this — keep it current. |

**Shipped and real:** `pbai_lm.sapp` (char-level LM with hand-derived backprop), `pbai_tokenizer.sapp`, `pbai_bpe.sapp` (BPE with corpus-keyed cache), `pbai.sapp` (assistant + intent router + grounded skills), `pbai_brain.sapp` (perception/memory), checkpoint save/load round-trip.

## 6. Quality analysis

- **Tests:** 10 suite files, ~100 assertions; good structural coverage but (a) no assertion that `train_checkpoint()` *reduces loss* — the Max test asserts map shape only; (b) no edge-case tests (empty corpus, out-of-vocab chars, zero examples) — exactly where bugs #1–#4 live; (c) the suite was green on Windows and broken on Linux, so it isn't run cross-platform — no CI exists in this repo.
- **Comment density** healthy (12–30% per module); `pbai.sapp` 1,023 lines is the largest file and is well-sectioned.
- **Layering** clean: tokenizer ← lm ← brain/assistant ← max-facade; no circular imports; demo/ and website/ thin.
- **Caching** done right where it exists (BPE tokenizer corpus-keyed cache, `pbai.sapp:913-926`) — consistent with the project's cache-to-disk performance doctrine.

## 7. Comparison against ML stacks in other languages

pbAI's distinguishing claim is *an LM implemented in the language it serves, with zero external dependencies* — the right comparisons are educational/self-contained ML systems, not production frameworks:

| | pbAI (pblang) | PyTorch (Python) | llama.cpp (C++) | candle (Rust) | micrograd/minGPT (Python) | TF.js (JS) | Flux (Julia) |
|---|---|---|---|---|---|---|---|
| Role | self-hosted LM for the language's own IDE/tooling | production research/training | production inference | production inference/training | education | browser ML | research |
| External deps | **0** (stdlib only) | huge C++/CUDA | none (C++) | few | numpy | none (browser) | many |
| Autograd | hand-derived per-layer | full autograd | n/a (inference) | full | full (tiny) | full | full |
| GPU | no (CPU; multi-core best-of-N planned via spawn) | yes | yes | yes | no | WebGPU | yes |
| Scale today | char-level Nano (~5 KB weights) + BPE; Max = facade | any | 1B–400B+ | 1B–70B | toy | small-mid | mid |
| Where it wins | purity (whole stack readable in one language), IDE integration, determinism | ecosystem | perf/portability | safety+perf | pedagogy | reach | elegance |

**Honest placement:** pbAI today is a micrograd-class system with product packaging (checkpoints, server, website, assistant skills) — that is a legitimate and rare niche (a language shipping its own native model for completion/explanation, cf. none of Python/JS/Lua ship one in-tree). The gap to usefulness-at-scale is the same as pblang's general perf gap: no GPU path and interpreter-speed inner loops; the planned lever (multi-core spawn training) is the code that bug #1 currently breaks.

## 8. Scorecard & priorities

| Axis | Score | Note |
|---|---|---|
| Correctness | 8/10 | current suite is green; math verified sound; remaining risk is scale/edge-case depth |
| Security | 8/10 | localhost server has defense-in-depth path guards; no secrets |
| Completeness honesty | 8/10 | Max facade and placeholder returns are now labelled; full Pro/Max roadmap remains open |
| Code quality | 8/10 | clean layering, healthy comments, good caching |
| Tests/CI | 6/10 | local semantics green and Max loss-decrease assertion added; still no project CI |

**Do next:** ① one-line fix for `env_get` nil in `pbai_accel.sapp:38` (+ Linux `nproc` fallback) — un-breaks 4 suites; ② reinstall `pblang` to `~/.local/bin` from current main (stale binary lacks path-fallback → 2 more suites); ③ untrack `target/` from git; ④ guards for §2 #2–#4; ⑤ redact personal paths (3 files); ⑥ add a loss-decreases assertion to the Max test; ⑦ mark `stream_status`/`distillation_plan` as placeholders in their docstrings.

- Audit date: **2026-06-09**, branch `main` @ `e1a2cf0`.

---

## 9. Resolution log (2026-06-10)

Fixes applied after the audit; the original findings above are kept as the dated snapshot.

| Audit item | Status | What changed |
|---|---|---|
| §2 #1 — `env_get` nil crash (`pbai_accel.sapp`) | **FIXED** | `cpu_cores()` now coerces a `nil` env var to `""` before parsing, and `_parse_int()` guards `nil`. Un-breaks the Linux suite path that crashed on the unset `NUMBER_OF_PROCESSORS`. |
| §2 #2 — unguarded softmax `/sum` (`pbai_lm.sapp`) | **FIXED** | `_forward()` falls back to a uniform distribution when `sum <= 0`/NaN instead of cascading NaN. |
| §2 #4 — top-k renorm `/s` (`pbai_lm.sapp`) | **FIXED** | `_sample_logits()` first normalization now has a uniform fallback when `s <= 0`/NaN. |
| §2 #3 — `/len(records)` empty (`pbai_max.sapp`) | **FIXED** | `_examples()` returns early when `curated_records()` is empty. |
| §5 — placeholders unlabelled | **FIXED** | Accel module honesty rewritten. `stream_status()` now returns `implementation_status: "prototype-status-only"` and `distillation_plan()` now returns `implementation_status: "plan-only"` with explicit steps. |
| §4 — personal-topology paths in feed/docs | **FIXED** | `tools/pbai_feed.sapp` now reads drive/device roots from semicolon-separated `PBAI_FEED_ROOTS` and falls back to generic project-relative roots. `docs/pbAI.md`, `docs/pbAI_source_audit.md`, `docs/pbAI_externalization.md`, `stdlib/pbai_assets.sapp`, and `stdlib/pbai_tools.sapp` no longer hardcode this laptop's exact roots. |
| §4 — `target/` generated outputs | **FIXED** | Added `.gitignore` with `target/` and removed `target/pbai-max-test/**` from the git index with `git rm --cached -r target`; local files remain on disk. |
| New (not in audit) — `probe_gpu()` broken on current pblang | **FIXED** | `exec()` was hardened (rejects whitespace/metacharacters, returns only an exit code), so the GPU probe silently failed. Rewrote `probe_gpu`/`probe_npu` to use `proc_spawn_wait(cmd, [args])`, which returns `{exit_code, stdout, stderr}`. Detection now works live. |
| New (not in audit) — NPU detection | **ADDED** | `accel.probe_npu()` enumerates a present NPU (Windows `ComputeAccelerator` class). Honest: detection only; no NPU compute backend (vendor-stack territory). |
| New — GPU honesty stale | **FIXED** | pblang now has an opt-in wgpu matmul backend; `pbai_accel` and the README no longer claim "no GPU backend." |

Still open from "Do next": none from the original seven-item list.

Additional follow-up:

| Audit item | Status | What changed |
|---|---|---|
| §6 — Max test lacked loss-decrease assertion | **FIXED** | `tests/semantics/pbai_max_test.sapp` now asserts the deterministic Max training checkpoint does not worsen training loss (`metrics.loss_after <= metrics.loss_before`). |

- Resolution date: **2026-06-10**, branch `main`.

## 10. Follow-up verification (2026-06-11)

| Item | Status | What changed |
|---|---|---|
| Stale installed `pblang` on Linux | **FIXED IN LOCAL ENV** | Replaced `~/.local/bin/pblang` with the current release build from `pl-dev/pblang/target/release/pblang`, restoring stdlib path fallback for this AAP layout. |
| `pbai_hw_test` non-Windows NPU honesty shape | **FIXED UPSTREAM IN pblang** | `stdlib:hwinfo.sapp` now returns `used_by_pblang` on non-Windows fallback maps for RAM, disks, GPU, NPU, network, sound, and device-class inventory, so callers get a stable honesty-label field on every platform. |
| §3 — server path traversal defense-in-depth | **FIXED** | `tools/pbai_server.sapp` now serves only `/website/...`, rejects `..`, absolute paths, backslashes, and drive-style `:` paths before calling `http.static_file_response`, and returns 404 for unrelated routes. |
| Acceleration planning on Linux/macOS | **IMPROVED** | `stdlib/pbai_accel.sapp` now tries `nproc` and `getconf _NPROCESSORS_ONLN` after `NUMBER_OF_PROCESSORS`, so pbAI worker planning can use the machine's real logical core count instead of defaulting to 4 when the Windows env var is absent. |
| Full pbAI semantics suite | **GREEN** | `pblang test tests/semantics --allow-all` now reports **12 passed / 0 failed / 0 ignored**. |

- Follow-up date: **2026-06-11**.

## 11. Docs-driven tool substrate sync (2026-06-12)

| Item | Status | What changed |
|---|---|---|
| pb-mcp-server tool count references | **FIXED** | `docs/pbAI.md` and `stdlib/pbai_tools.sapp` now describe the companion pb-mcp-server surface as **500+ tools**, matching the current pb-mcp-server docs and the 501-handler expansion recorded in its analysis log. |
| Full pbAI semantics suite | **GREEN** | `pblang test tests/semantics --allow-all` reports **12 passed / 0 failed / 0 ignored** after the docs/tool-substrate sync. |

- Follow-up date: **2026-06-12**.

## 12. Safe live-ingestion redaction (2026-06-14)

Cross-repo follow-up with `pl-dev/pblang`: pbAI's local file/folder learning now
uses the shared pblang `stdlib:llm_data.sapp` sanitizer before appending text
chunks to `stdlib/pbai_live_corpus.txt`.

| Item | Status | What changed |
|---|---|---|
| Shared sanitizer dependency | **ADDED** | `stdlib/pbai_ingest.sapp` imports `stdlib:llm_data.sapp` and runs `DATA.sanitize_text(...)` on text-like files before chunking/remembering them. |
| Redaction accounting | **ADDED** | `ingest_path(...)` now reports `stats.redactions`, so the website/API can expose when sensitive lines were removed. |
| Secret value protection | **ADDED** | Common API-key/token/password/private-key line shapes are replaced with `[REDACTED sensitive line]` before live memory persistence. Safe neighboring content still learns normally. |
| Regression coverage | **ADDED** | `tests/semantics/pbai_ingest_test.sapp` ingests `tests/fixtures/sanitized_note.txt`, verifies safe content is learned, verifies one redaction is counted, and verifies the secret value is not written to live memory. |
| Full pbAI semantics suite | **GREEN** | `pblang test tests/semantics --allow-all` reports **13 passed / 0 failed / 0 ignored**. |

Verification:

```sh
pblang run tests/semantics/pbai_ingest_test.sapp --allow-all
pblang test tests/semantics --allow-all
```

Result: passed. The suite remains slower in Max/tokenizer/training tests, but
the new ingestion test saves and restores the live corpus so it does not leave
permanent local-learning state behind.

## 13. Shared pblang hardware summaries for pbAI acceleration probes (2026-06-14)

Cross-repo follow-up with `pl-dev/pblang`: pbAI no longer carries its own
Windows-only GPU/NPU probe strings. The acceleration policy now consumes the
shared pblang hardware inventory summary API.

| Item | Status | What changed |
|---|---|---|
| Shared detector dependency | **IMPROVED** | `stdlib/pbai_accel.sapp` imports `stdlib/pbai_hw.sapp`, which re-exports pblang's `stdlib:hwinfo.sapp`. |
| GPU probe | **IMPROVED** | `probe_gpu()` now returns `pbai_hw.gpu_summary()`, so Windows/Linux/macOS share the same best-effort inventory path and degrade to explicit unavailable/no-device notes. |
| NPU probe | **IMPROVED** | `probe_npu()` now returns `pbai_hw.npu_summary()`, keeping the no-NPU-compute-backend boundary while avoiding stale platform placeholder text. |
| pbAI hardware alias | **EXPANDED** | `stdlib/pbai_hw.sapp` re-exports `gpu_summary()` and `npu_summary()` for direct assistant/tool use. |
| Regression coverage | **ADDED** | `tests/semantics/pbai_accel_test.sapp` asserts both probe functions return non-empty status text and do not contain the old `"not implemented for platform"` wording. |

Verification:

```sh
pblang run tests/semantics/pbai_accel_test.sapp --allow-all
pblang test tests/semantics --allow-all --quiet
```

Result: passed, including the full pbAI semantics suite
(**13 passed / 0 failed / 0 ignored**).

## 14. pbAI-Max prompt-level regression gate (2026-06-14)

pbAI-Max now has a pblang-only regression layer that measures concrete generated
outputs in addition to aggregate train/validation loss.

| Item | Status | What changed |
|---|---|---|
| Regression fixtures | **ADDED** | `stdlib/pbai_max.sapp` exposes `regression_cases()` for deterministic pblang/code and assistant prompt seeds. |
| Generated-output scoring | **ADDED** | `regression_suite(ckpt, n)` runs generation for every case and scores expected-token hits, forbidden-token misses, minimum generated length, nonblank tails, and repetition/collapse signals. |
| Checkpoint metadata | **ADDED** | `train_checkpoint(...)` stores a `pbai.max.regression_suite.v1` report inside the checkpoint, and `checkpoint_summary(...)` surfaces it for tools. |
| Facade/API access | **ADDED** | `stdlib/pbai.sapp` exports `max_regression(dir, n)`, and `tools/pbai_api.sapp` supports the `max_regression` action. |
| Regression coverage | **ADDED** | `tests/semantics/pbai_max_test.sapp` asserts checkpoint, on-demand, and summary-level regression contracts. |

Verification:

```sh
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tools/pbai_api.sapp --allow-all -- max_regression
pblang test tests/semantics --allow-all --quiet
```

Result: passed, including the full pbAI semantics suite
(**13 passed / 0 failed / 0 ignored**). The first run correctly exposed that the
saved checkpoint had `pass_rate: 0.0` because every checked prompt collapsed
into repeated `/` characters. §15 records the decoder-side fix.

## 15. pbAI-Max safe decoding uses pblang anti-collapse generation (2026-06-14)

Cross-repo follow-up with `pl-dev/pblang`: pbAI-Max now uses the shared
`llm_attention.generate_safe(...)` decoder path for Max generation, checkpoint
benchmarks, and prompt regression reports.

| Item | Status | What changed |
|---|---|---|
| pblang substrate | **ADDED** | `stdlib:llm_attention.sapp` now exposes `repetition_penalized_logits(...)` and `generate_safe(...)` to penalize recent tokens and hard-block an over-repeated tail token. |
| Max generation | **IMPROVED** | `stdlib/pbai_max.sapp` uses `ATT.generate_safe(...)` in `generate(...)` and regression generation instead of raw greedy `ATT.generate(...)`. |
| Max benchmark | **IMPROVED** | `train_checkpoint(...)` benchmarks the safe decoder path, so checkpoint metadata reflects the actual Max generation route. |
| Regression contract | **TIGHTENED** | `tests/semantics/pbai_max_test.sapp` now requires the Max regression pass rate to be at least `0.50`, so token-collapse regressions cannot pass as shape-only reports. |

Verification:

```sh
pblang run tools/pbai_api.sapp --allow-all -- max_regression
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang test tests/semantics --allow-all --quiet
```

Result: passed. `max_regression` now reports **pass_rate: 1.0** on the saved
checkpoint; the generated text is still tiny-model quality, but the long `/`
collapse is capped by the pblang safe decoder. The full pbAI semantics suite
passed **13 passed / 0 failed / 0 ignored**.

## 16. pbAI-Max continuation uses regression-aware candidate selection (2026-06-14)

pbAI-Max continuation no longer ranks and accepts candidates by validation loss
alone. It now uses the shared pblang `llm_eval.generation_quality(...)` scorer
inside Max prompt regression and folds regression quality into candidate
selection.

| Item | Status | What changed |
|---|---|---|
| Shared scorer | **IMPROVED** | Max regression score maps now include `source_format: "pblang.llm.generation_quality.v1"`, proving the scoring is delegated to the reusable pblang LLM eval layer. |
| Selection score | **ADDED** | Continuation candidates carry `pbai.max.selection_score.v1`: validation loss, regression pass rate, regression average score, failed regression count, and weighted score. |
| Candidate ranking | **IMPROVED** | `_rank_candidates(...)` now sorts by the validation-plus-regression selection score instead of raw validation loss. |
| Acceptance policy | **IMPROVED** | A candidate is saved if validation improves, or if prompt regression improves while validation stays within a `0.05` tolerance. Otherwise the current checkpoint is kept. |
| API evidence | **VERIFIED** | `max_continue` now returns `before_selection`, `after_selection`, `acceptance_policy`, and selection details for every reported candidate. |

Verification:

```sh
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tools/pbai_api.sapp --allow-all -- max_continue /tmp/pbai-max-continue-smoke.json
pblang test tests/semantics --allow-all --quiet
```

Result: passed. The smoke run rejected the best candidate because validation
worsened while regression was already `pass_rate: 1.0`, so the current
checkpoint was kept. That is the intended behavior: training loss can improve
without saving an overfit or lower-quality Max checkpoint. The full pbAI
semantics suite passed **13 passed / 0 failed / 0 ignored**.

## 17. pbAI-Max generation uses multi-candidate safe reranking (2026-06-14)

pbAI-Max now improves inference/regression behavior without changing the public
text-generation contract: it tries multiple safe decoding settings, scores every
candidate with pblang's shared LLM eval helper, and keeps the best result.

| Item | Status | What changed |
|---|---|---|
| Ranked generation | **ADDED** | `stdlib/pbai_max.sapp` exposes `generate_ranked(...)` / `generate_ranked_saved(...)` with `pbai.max.ranked_generation.v1` metadata. |
| Backward compatibility | **KEPT** | `generate(...)` and `max_generate` still return only the selected text. |
| API access | **ADDED** | `tools/pbai_api.sapp` now supports `max_generate_ranked` for candidate/settings/quality inspection. |
| Regression path | **IMPROVED** | Max prompt regression stores the ranked generation object per case and reports `generation_policy: "multi-candidate-safe-decode-rerank"`. |
| Test contract | **EXPANDED** | `pbai_max_test.sapp` verifies the ranked-generation format, candidate count, selected candidate, and shared pblang scorer delegation. |

Verification:

```sh
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang test tests/semantics --allow-all --quiet
```

Result: focused pbAI-Max verification passed (`pbai_max_pass`). The full pbAI
semantics suite passed **13 passed / 0 failed / 0 ignored**.

## 18. pbAI-Max ranked generation uses model sequence loss (2026-06-14)

The previous ranked generation pass used text-quality scoring only. This pass
adds the model's own sequence loss/accuracy as a second ranking signal, so
pbAI-Max can prefer candidates that are both non-collapsed and more coherent
under the checkpoint.

| Item | Status | What changed |
|---|---|---|
| Sequence loss | **ADDED** | Every Max generation candidate now carries `pblang.llm.sequence_eval.v1` from `llm_eval.sequence_evaluate(...)`. |
| Combined rank | **IMPROVED** | The selected candidate uses `pbai.max.generation_choice.v2` with quality, sequence-loss, and sequence-accuracy weights. |
| Regression policy | **UPDATED** | Regression reports now state `generation_policy: "multi-candidate-safe-decode-loss-rerank"`. |
| API visibility | **KEPT** | `max_generate_ranked` exposes candidate text, safe-decoding settings, quality score, sequence eval, and final rank score. |
| Model training | **RUN** | A fresh pbAI-Max checkpoint was trained and saved through the pblang-only `max_save` path after the code change. |

Verification:

```sh
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tools/pbai_api.sapp --allow-all -- max_save /tmp/pbai-max-train.json
pblang run tools/pbai_api.sapp --allow-all -- max_generate_ranked /tmp/pbai-max-ranked.json
pblang test tests/semantics --allow-all --quiet
```

Result: focused pbAI-Max verification passed (`pbai_max_pass`). A fresh Max
checkpoint was trained and saved, then a conservative continuation candidate was
accepted because validation improved. Final saved checkpoint metrics:
`loss_after: 4.1102`, `valid_loss_after: 4.7510`, regression `pass_rate: 1.0`,
`params: 5040`, generation policy
`multi-candidate-safe-decode-loss-rerank`. The full pbAI semantics suite passed
**13 passed / 0 failed / 0 ignored**.

## 19. pbAI-Max ranked generation uses text-shape scoring (2026-06-14)

This pass adds one more pblang-owned signal to Max inference: deterministic
text-shape scoring from `stdlib:llm_eval.sapp`. It is a practical guard for the
current tiny checkpoint while the learned model is still weak.

| Item | Status | What changed |
|---|---|---|
| Curated data | **EXPANDED** | `source_records()` now includes a clean pblang generation record with functions, maps, loops, conditionals, and prints. |
| Text-shape metadata | **ADDED** | Every ranked generation candidate now carries `pblang.llm.text_shape.v1`. |
| Combined rank | **IMPROVED** | The selected candidate uses `pbai.max.generation_choice.v3` with quality, sequence-loss, sequence-accuracy, and text-shape weights. |
| Regression policy | **UPDATED** | Regression reports now state `generation_policy: "multi-candidate-safe-decode-loss-shape-rerank"`. |
| API visibility | **KEPT** | `max_generate_ranked` exposes quality, sequence eval, text shape, decode settings, and final rank score. |

Verification:

```sh
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tools/pbai_api.sapp --allow-all -- max_save /tmp/pbai-max-train.json
pblang run tools/pbai_api.sapp --allow-all -- max_continue /tmp/pbai-max-continue.json
pblang run tools/pbai_api.sapp --allow-all -- max_summary /tmp/pbai-max-summary.json
pblang test tests/semantics --allow-all --quiet
```

Result: focused pbAI-Max verification passed (`pbai_max_pass`). A fresh Max
checkpoint was trained and saved, then the conservative continuation candidate
was accepted because validation improved. Final saved checkpoint metrics:
`train_loss: 4.1122`, `valid_loss: 4.7803`, `accuracy: 0.0779`,
`valid_perplexity: 119.1395`, regression `pass_rate: 1.0`, `vocab: 224`,
`params: 5040`, generation policy
`multi-candidate-safe-decode-loss-shape-rerank`. The full pbAI semantics suite
passed **13 passed / 0 failed / 0 ignored**.

## 20. pbAI-Max continuation uses fast head-only screening (2026-06-14)

This pass attacks training time directly. The old continuation path ran the
full binary/feedback/deep/regression stack for every quick candidate, which
meant prompt generation and regression scoring were paid six times before the
finalist stage. Now quick candidates are cheap validation screens, and only
finalists run the full path.

| Item | Status | What changed |
|---|---|---|
| Quick screening | **OPTIMIZED** | Quick candidates now run `quick-head-validation-screen`: LM-head training plus train/validation evaluation only. |
| Shared pblang helpers | **ADDED** | Quick screens use pblang `llm_eval.regression_proxy(...)`; candidate ranking delegates to `llm_eval.selection_score(...)`. |
| Full finalists | **KEPT** | Finalists still run binary adapter training, adaptive growth, feedback loop, guessing loop, elite selection, analytic deep-input updates, and prompt regression. |
| Checkpoint safety | **KEPT** | Worker candidates still never write checkpoints; only the parent saves the accepted full finalist. |
| Test contract | **EXPANDED** | `pbai_max_test.sapp` verifies quick-screen mode, full-finalist mode, and pblang-backed selection scoring. |

Verification:

```sh
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tools/pbai_api.sapp --allow-all -- max_save /tmp/pbai-max-train.json
pblang run tools/pbai_api.sapp --allow-all -- max_continue /tmp/pbai-max-continue.json
pblang run tools/pbai_api.sapp --allow-all -- max_summary /tmp/pbai-max-summary.json
```

Result: focused pbAI-Max verification passed (`pbai_max_pass`). A fresh Max
checkpoint was trained and saved in **104.33s**, then optimized continuation ran
in **112.00s** wall time with **252.34s** user CPU, showing parallel worker use.
The continuation candidate was accepted because validation improved. Final saved
checkpoint metrics: `train_loss: 4.0903`, `valid_loss: 4.7510`,
`accuracy: 0.0790`, `valid_perplexity: 115.7005`, regression `pass_rate: 1.0`,
`vocab: 224`, `params: 5040`. Selection scoring records
`source_format: "pblang.llm.selection_score.v1"`. The full pbAI semantics suite
passed **13 passed / 0 failed / 0 ignored**.

## 21. pbAI-Max training uses curriculum-interleaved examples (2026-06-15)

This pass improves model quality by shaping the bounded training set, not by
blindly increasing work. pbAI-Max now reserves part of the training budget for
short, clean pblang curriculum snippets and interleaves them through the broader
curated corpus.

| Item | Status | What changed |
|---|---|---|
| Curriculum records | **ADDED** | `curriculum_records()` adds clean functions, control flow, agent-answer, and assertion examples. |
| Example policy | **UPDATED** | Max checkpoint metadata now reports `balanced-per-record-strong-target-plus-curriculum-interleave`. |
| Shared pblang data helpers | **USED** | Max uses `llm_data.repeat_examples(...)` and `llm_data.interleave_priority_examples(...)`. |
| Small-run safety | **FIXED** | If a continuation run is too small for a finalist stage, candidates use the full regression path instead of saving a quick proxy checkpoint. |
| Test contract | **EXPANDED** | `pbai_max_test.sapp` verifies curriculum metadata and the tiny-run full-candidate fallback. |

Verification:

```sh
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tools/pbai_api.sapp --allow-all -- max_save /tmp/pbai-max-train.json
pblang run tools/pbai_api.sapp --allow-all -- max_continue /tmp/pbai-max-continue.json
pblang run tools/pbai_api.sapp --allow-all -- max_summary /tmp/pbai-max-summary.json
pblang test tests/semantics --allow-all --quiet
```

Result: focused pbAI-Max verification passed (`pbai_max_pass`). A fresh Max
checkpoint was trained and saved in **107.04s**, then optimized continuation ran
in **118.92s** wall time with **259.25s** user CPU, showing parallel worker use.
The continuation candidate was accepted because validation improved. Final saved
checkpoint metrics: `train_loss: 4.0066`, `valid_loss: 4.6635`,
`accuracy: 0.0949`, `valid_perplexity: 106.0116`, regression
`pass_rate: 1.0`, `vocab: 224`, `params: 5040`, example policy
`balanced-per-record-strong-target-plus-curriculum-interleave`. The full pbAI
semantics suite passed **13 passed / 0 failed / 0 ignored**.

## 22. pbAI-Max live growth, personality lessons, and parallel training (2026-06-15)

This pass makes Max less fixed and more alive in the practical pblang sense:
there is still a small seed architecture, but checkpoint training can grow
context/layers/binary signal through policy-gated feedback, Max has a local
live-lessons corpus, and LM-head training now shards across pblang workers.

| Item | Status | What changed |
|---|---|---|
| Larger seed | **ADDED** | Max-v2 seed uses `context=16`, `d_model=12`, `d_ff=24`, `layers=3`, `heads=3`; trained checkpoint has `params: 9644`. |
| Live growth | **EXPANDED** | Checkpoint policy now allows `stronger_binary`, `weaker_binary`, `longer_context`, and `deeper`; training grew context to `32`. |
| Live lessons | **ADDED** | `stdlib/pbai_max_live_lessons.txt` is part of the corpus and `max_teach` appends explicit lessons before validation-gated continuation. |
| Personality profile | **ADDED** | Max status/checkpoints expose `pbai.max.personality.v1` with clear/truthful/useful/curious/calm/evidence/privacy traits. |
| Parallel training | **IMPROVED** | Max LM-head training uses `parallel-cached-hidden-lm-head-backprop`: pblang workers cache transformer hidden states, then a deterministic head update loop minimizes loss over that cache. |
| Safety gate | **KEPT** | A taught personality/learning lesson was remembered, but its candidate checkpoint was rejected because validation worsened. |

Verification:

```sh
./target/debug/pblang test tests/semantics --filter llm --allow-all --quiet --timings --slow=3
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tools/pbai_api.sapp --allow-all -- max_train /tmp/pbai-max-smoke.json
pblang run tools/pbai_api.sapp --allow-all -- max_save /tmp/pbai-max-train-parallel.json
pblang run tools/pbai_api.sapp --allow-all -- max_teach /tmp/pbai-max-teach.json
```

Result: focused pblang LLM tests passed **3 passed / 0 failed**. Focused
pbAI-Max verification passed (`pbai_max_pass`) in **157.01s** wall time with
**478.46s** user CPU. A cached-hidden smoke train with 80 examples used
`parallel-cached-hidden-lm-head-backprop`, `4` workers, `params: 9644`,
`loss: 3.9657`, `valid_loss: 4.8705`, and `accuracy: 0.1944` in **30.97s**
wall time with **33.04s** user CPU. The saved cached-hidden checkpoint has
`params: 9644`, `train_loss: 4.1411`, `accuracy: 0.0949`, `perplexity:
62.8724`, and context grown to `32`. A validation-gated continuation improved
train loss `4.1435 -> 4.0029` but worsened validation `4.4751 -> 4.5019`, so it
was rejected; the previous checkpoint stayed active. `max_teach` remembered a
424-character personality/learning lesson, then rejected the candidate because
validation worsened `4.4872 -> 4.5296`; the live lesson remains in the local
corpus for future training. The full pbAI semantics suite passed **13 passed /
0 failed / 0 ignored** before the cache feature in §23.

## 23. General pbAI persistent cache (2026-06-15)

pbAI now has a pblang-only cache layer for derived artifacts. The cache is
deliberately separate from source-of-truth data: clearing it never removes
checkpoints, sharded weights, or live lessons.

| Item | Status | What changed |
|---|---|---|
| Cache module | **ADDED** | `stdlib/pbai_cache.sapp` stores JSON entries under `target/pbai-cache` and exposes `put/get/has/remove/clear/status`. |
| Path hardening | **ADDED** | Cache kinds are normalized to letters, digits, `_`, and `-` before building paths, so API-supplied kinds cannot escape the cache root. |
| Max training cache | **ADDED** | `pbai_max.train_checkpoint(...)` writes a `training-summary` cache entry with params, loss, validation loss, accuracy, regression pass rate, workers, and live-mutability metadata. |
| Summary visibility | **ADDED** | Max status and checkpoint summaries expose cache status/write metadata. |
| Facade/API access | **ADDED** | `stdlib/pbai.sapp` exports `cache_status()` / `cache_clear(kind)`, and `tools/pbai_api.sapp` adds `cache_status` and `cache_clear` actions. |
| Regression coverage | **ADDED** | `tests/semantics/pbai_cache_test.sapp` verifies write/read/status/clear plus normalized unsafe kind names; `pbai_max_test.sapp` verifies checkpoint summary cache metadata. |

Verification:

```sh
pblang run tests/semantics/pbai_cache_test.sapp --allow-all
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang test tests/semantics --allow-all --quiet
```

Result: the cache-focused test passed (`pbai_cache_pass`). Full-suite evidence
passed with **14 passed / 0 failed / 0 ignored** in **506.25s** wall time. The
`max_summary` API now exposes checkpoint cache metadata and current cache
status, including the `training-summary` entry under `target/pbai-cache`.

## 24. pbAI-Max retrieval/verifier reranking and generation cache (2026-06-15)

This pass uses more of pblang's shared LLM and runtime features inside Max:
safe decoding remains neural, but Max now also gives the reranker a
retrieval-grounded candidate from its curated/curriculum corpus, scores every
candidate with a verifier, and caches saved ranked generations.

| Item | Status | What changed |
|---|---|---|
| Retrieval-grounded candidate | **ADDED** | `_generation_candidates(...)` now adds a `retrieval-grounded-candidate` selected from curated Max records and curriculum records by prompt/token hits. |
| Verifier score | **ADDED** | Every candidate records `pbai.max.generation_verifier.v1`, checking non-empty output, expected/forbidden tokens, brace/paren sanity, and repetition/collapse. |
| Reranker weights | **UPDATED** | Final rank now combines generation quality, sequence loss, sequence accuracy, text shape, and verifier score. |
| Regression contract | **UPDATED** | Regression reports now use `multi-candidate-safe-decode-retrieval-verifier-rerank` and assert neural plus retrieval candidates. |
| Generation cache | **ADDED** | `generate_ranked_cached(...)` stores saved ranked generations under `target/pbai-cache/max-generation`, keyed by checkpoint signature, prompt, token count, and scoring settings. |
| pblang feature use | **EXPANDED** | Max uses pblang `llm_attention`, `llm_eval`, `llm_data`, `spawn`, and `pbai_cache` together instead of duplicating those foundations locally. |

Verification:

```sh
pblang run tests/semantics/pbai_cache_test.sapp --allow-all
pblang run tests/semantics/pbai_max_test.sapp --allow-all
```

Result: cache verification passed (`pbai_cache_pass`) and focused Max
verification passed (`pbai_max_pass`). The Max test also verifies the repeated
saved-generation cache hit path.

## 25. pbAI-Max number-one strategy document (2026-06-15)

This pass turns the AGI/ASI discussion into a concrete project document without
claiming that the current model already has those capabilities.

| Item | Status | What changed |
|---|---|---|
| Strategy doc | **ADDED** | `docs/pbAI_Max_Strategy.md` defines the number-one strategy: `data -> train -> evaluate -> use tools -> solve tasks -> verify -> remember -> improve model/runtime -> repeat`. |
| Roadmap shape | **DOCUMENTED** | The strategy covers model architecture, data, retrieval, tools, agents, verifier, runtime/hardware, safety, evals, and staged milestones. |
| Honest AGI/ASI boundary | **DOCUMENTED** | AGI/ASI are described as research goals, not current claims. |
| Main docs link | **ADDED** | `README.md` and `docs/pbAI.md` now link to the strategy as the top-level pbAI-Max direction. |

Verification:

```sh
git diff --check
```

Result: documentation diff is whitespace-clean.

## 26. Priority #1 started: pbAI-Max pblang-native vector memory (2026-06-15)

The first implementation step from the number-one strategy is now in code:
pbAI-Max has a pblang-native vector-memory retrieval layer instead of only a
word-hit retrieval candidate.

| Item | Status | What changed |
|---|---|---|
| Vector memory index | **ADDED** | `pbai_max.memory_index(...)` builds deterministic text vectors for quality-filtered Max records plus curriculum examples and indexes them through `llm_quant.turbovec_*`. |
| Memory retrieval API | **ADDED** | `pbai_max.memory_retrieve(prompt, top_k, max_records)` returns `pbai.max.memory_retrieval.v1` with sourced `pbai.max.memory_hit.v1` snippets and the underlying Turbovec search metadata. |
| Status visibility | **ADDED** | `pbai_max.status()` exposes `memory: pbai.max.memory_status.v1` with record count, vector dimension, bit width, manifest, and cache kind. |
| Generation grounding | **IMPROVED** | The retrieval-grounded candidate used by ranked generation is now backed by vector memory and carries the full memory retrieval report. |
| Cache integration | **ADDED** | Retrieval reports write derived metadata through `pbai_cache` under `max-memory-retrieval`; checkpoints and live lessons remain the source of truth. |
| Facade/API access | **ADDED** | `stdlib/pbai.sapp` exports `max_memory_status(...)` and `max_memory_retrieve(...)`; `tools/pbai_api.sapp` exposes `max_memory_status` and `max_memory`. |

Verification:

```sh
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tools/pbai_api.sapp --allow-all -- max_memory /tmp/pbai-max-memory.json
pblang run tests/semantics/pbai_test.sapp --allow-all
git diff --check
```

Result: focused Max verification passed (`pbai_max_pass`), the API smoke
returned a `pbai.max.memory_retrieval.v1` report whose top hit was the pblang
`fn add` source record, and the pbAI facade test passed (`pbai_pass`).

## 27. Priority #2 started: pblang LoRA-style adapters for pbAI-Max (2026-06-15)

The second implementation step from the number-one strategy is now started:
pblang owns a reusable low-rank adapter foundation and pbAI-Max consumes it.

| Item | Status | What changed |
|---|---|---|
| pblang adapter module | **ADDED** | `pl-dev/pblang/stdlib/llm_adapters.sapp` adds LoRA-style frozen-base LM-head adapters: config, summary, delta logits, adapter training, state reconstruction, and merge helpers. |
| pblang test coverage | **ADDED** | `llm_foundation_test.sapp` imports `llm_adapters`, trains a small adapter over transformer hidden states, reconstructs adapter state, and verifies merge support. |
| Max status | **ADDED** | `pbai_max.status()` exposes `adapter: pbai.max.adapter_status.v1`, including trainable params versus full LM-head params. |
| Max adapter training | **ADDED** | `pbai_max.train_adapter(...)` trains a low-rank adapter over Max examples while keeping base weights frozen and writes cache metadata under `max-adapter-training`. |
| Facade/API access | **ADDED** | `stdlib/pbai.sapp` exposes `max_adapter_status()` / `max_train_adapter(...)`; `tools/pbai_api.sapp` exposes `max_adapter_status` and `max_train_adapter`. |
| Boundary | **HONEST** | Current adapter target is LM-head only. Q/K/V/O, FFN, embedding, and QLoRA-style quantized adapters remain future work. |

Verification:

```sh
pblang run tests/semantics/llm_foundation_test.sapp --allow-all
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tools/pbai_api.sapp --allow-all -- max_train_adapter /tmp/pbai-max-adapter.json
```

Result: pblang LLM foundation verification passed (`llm_foundation_pass`),
focused Max verification passed (`pbai_max_pass`), and the API smoke trained
24 low-rank adapter steps with 344 trainable params and frozen base weights.

## 28. Priority #3 started: live lessons train adapters first (2026-06-15)

The third implementation step from the number-one strategy is now started:
explicit pbAI-Max live lessons can train a small adapter without rewriting the
base Max checkpoint.

| Item | Status | What changed |
|---|---|---|
| Live adapter training | **ADDED** | `pbai_max.train_live_adapter(...)` reads `stdlib/pbai_max_live_lessons.txt`, builds live-lesson LM examples, and trains the LoRA-style LM-head adapter. |
| Teach adapter route | **ADDED** | `pbai_max.teach_adapter(...)` appends a lesson and immediately trains the low-rank adapter path. |
| Checkpoint boundary | **ENFORCED** | Live adapter reports include `base_weights_frozen: true` and `touches_checkpoint: false`, so live lessons do not silently rewrite the base Max checkpoint. |
| Facade/API access | **ADDED** | `stdlib/pbai.sapp` exposes `max_train_live_adapter(...)` and `max_teach_adapter(...)`; `tools/pbai_api.sapp` exposes `max_train_live_adapter` and `max_teach_adapter`. |
| Regression coverage | **ADDED** | `pbai_max_test.sapp` trains from a temporary live lesson, verifies the checkpoint boundary, and restores the live lessons file after the test. |

Verification:

```sh
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tools/pbai_api.sapp --allow-all -- max_teach_adapter /tmp/pbai-max-teach-adapter.json
```

Result: focused Max verification passed (`pbai_max_pass`). The API smoke
returned `pbai.max.teach_adapter.v1`, trained 24 low-rank live-adapter steps,
and reported `touches_checkpoint: false`.

## 29. Priority #4 started: pbAI-Max deterministic benchmark gate (2026-06-16)

The fourth implementation step from the number-one strategy is now started:
pbAI-Max has a cheap pblang-only benchmark suite that checks the surrounding
LLM system before heavier checkpoint regression is needed.

| Item | Status | What changed |
|---|---|---|
| Benchmark contract | **ADDED** | `pbai_max.benchmark_cases()`, `benchmark_status()`, and `benchmark_suite(ckpt, n)` return versioned Max benchmark records. |
| Category coverage | **ADDED** | The suite covers pblang coding, math, memory recall, vector retrieval, local tool visibility, and secret-redaction safety. |
| Optional checkpoint check | **ADDED** | Passing a checkpoint adds prompt-regression quality into the pblang-coding category. Without a checkpoint, the suite remains deterministic and cheap. |
| Status visibility | **ADDED** | `pbai_max.status()` exposes `benchmark: pbai.max.benchmark_status.v1`. |
| Facade/API access | **ADDED** | `stdlib/pbai.sapp` exposes `max_benchmark(dir, n)`, and `tools/pbai_api.sapp` exposes the `max_benchmark` action. |
| Regression coverage | **ADDED** | `pbai_max_test.sapp` verifies benchmark status, pass rate, category scores, and coding/math/memory/retrieval/tools/safety coverage. |

Verification:

```sh
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tools/pbai_api.sapp --allow-all -- max_benchmark
pblang run tests/semantics/pbai_test.sapp --allow-all
git diff --check
```

Result: focused Max verification passed (`pbai_max_pass`), the `max_benchmark`
API smoke returned 6/6 passing cases with `average_score: 1.0`, and the pbAI
facade test passed (`pbai_pass`).

## 30. Priority #5 started: validation plus benchmark acceptance gate (2026-06-16)

The fifth implementation step from the number-one strategy is now started:
pbAI-Max no longer treats validation/regression as the only acceptance signal.
The deterministic benchmark suite is part of checkpoint and adapter acceptance.

| Item | Status | What changed |
|---|---|---|
| Checkpoint benchmark metadata | **ADDED** | Fresh checkpoints now store `benchmark_gate: pbai.max.benchmark_suite.v1` alongside the existing generation-speed benchmark and prompt regression report. |
| Candidate selection | **IMPROVED** | `pbai.max.selection_score.v1` now includes benchmark pass rate, average score, failures, full benchmark report, and benchmark weights. |
| Continuation acceptance | **TIGHTENED** | `max_continue` saves only when validation improves without benchmark regression, or prompt regression and benchmark both improve within validation tolerance. |
| Rejection evidence | **IMPROVED** | Accepted/rejected continuation reports include `before_benchmark`, benchmark-aware before/after selection scores, and benchmark policy booleans. |
| Adapter acceptance | **ADDED** | `train_adapter(...)` and `train_live_adapter(...)` report `pbai.max.adapter_acceptance.v1`, so adapter training can happen live while merge/save readiness still requires loss plus benchmark checks. |
| Regression coverage | **ADDED** | `pbai_max_test.sapp` checks checkpoint benchmark metadata, adapter gates, benchmark-aware selection, acceptance policy mode, and summary exposure. |

Verification:

```sh
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tools/pbai_api.sapp --allow-all -- max_benchmark
pblang run tests/semantics/pbai_test.sapp --allow-all
git diff --check
```

Result: focused Max verification passed (`pbai_max_pass`), the `max_benchmark`
API smoke returned 6/6 passing cases with `average_score: 1.0`, the pbAI
facade test passed (`pbai_pass`), and the diff is whitespace-clean.

## 31. Benchmark thresholds strengthened after Priority #5 (2026-06-16)

After the initial five-step Max sequence, the benchmark gate was strengthened so
it can catch category-specific weakness instead of relying only on one global
average.

| Item | Status | What changed |
|---|---|---|
| More benchmark cases | **IMPROVED** | `benchmark_suite(...)` now runs at least two deterministic cases in every category: pblang coding, math, memory, retrieval, tools, and safety. |
| Category thresholds | **ADDED** | `benchmark_thresholds()` defines per-category minimum pass-rate and score requirements. |
| Threshold report | **ADDED** | `benchmark_threshold_report(benchmark)` returns `pbai.max.benchmark_threshold_report.v1` with failed categories and threshold details. |
| Selection score | **IMPROVED** | `pbai.max.selection_score.v1` now includes `benchmark_threshold_passed` and `benchmark_failed_categories`; failed categories add a ranking penalty. |
| Acceptance policy | **TIGHTENED** | Continuation acceptance now requires benchmark thresholds, not only global benchmark pass/average non-regression. |
| Regression coverage | **ADDED** | `pbai_max_test.sapp` requires 12+ benchmark cases, 2+ cases per category, threshold pass, and benchmark-threshold fields in adapter and continuation reports. |

Verification:

```sh
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tools/pbai_api.sapp --allow-all -- max_benchmark
pblang run tests/semantics/pbai_test.sapp --allow-all
git diff --check
```

Result: focused Max verification passed (`pbai_max_pass`), the `max_benchmark`
API smoke returned 12/12 passing cases with `average_score: 1.0` and a passing
threshold report, the pbAI facade test passed (`pbai_pass`), and the diff is
whitespace-clean.

## 32. Family distillation contract started (2026-06-16)

The next pbAI-Max improvement after benchmark thresholds is now started:
distillation is no longer only a roadmap note. Max exposes a versioned teacher
record contract that can feed Pro/Mini/Nano students.

| Item | Status | What changed |
|---|---|---|
| Distillation targets | **ADDED** | `distillation_targets()` defines pbAI-Pro, pbAI-Mini, and pbAI-Nano target contracts, including ownership boundary for Nano in pblang. |
| Teacher records | **ADDED** | `distillation_dataset(ckpt, max_records, n)` returns `pbai.max.distillation_record.v1` records with prompts, expected tokens, forbidden tokens, teacher text, target tiers, and verifier/rank metadata. |
| Checkpoint-backed preview | **ADDED** | With a checkpoint, distillation records call Max ranked generation and keep the ranked-generation trace; without a checkpoint, the preview stays deterministic and cheap. |
| Acceptance gate | **ADDED** | Distillation preview carries the same benchmark threshold report used by checkpoint/adapter acceptance. |
| Facade/API access | **ADDED** | `stdlib/pbai.sapp` exposes `max_distillation_preview(...)`, and `tools/pbai_api.sapp` exposes `max_distill_preview`. |
| Regression coverage | **ADDED** | `pbai_max_test.sapp` verifies status, target count, teacher record format, checkpoint-backed traces, and benchmark threshold gate. |

Verification:

```sh
pblang run tools/pbai_api.sapp --allow-all -- max_distill_preview
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tests/semantics/pbai_test.sapp --allow-all
```

Result: the API smoke returned `pbai.max.distillation_preview.v1` with a
passing benchmark threshold gate, focused Max verification passed
(`pbai_max_pass`), and the pbAI facade test passed (`pbai_pass`). Remaining
work at that point was real Pro/Mini/Nano student training; this is now started
in §34 with bounded student checkpoints.

## 33. Verified generation moved into pbAI-Max core (2026-06-17)

The API wrapper no longer owns the strict verifier/repair policy. pbAI-Max owns
the contract directly, so regression, distillation, and external API calls use
the same verified generation path.

| Item | Status | What changed |
|---|---|---|
| Core verifier | **MOVED** | `strict_verify_generation(...)`, `repair_generation_text(...)`, fallback helpers, and `generate_verified_ranked(...)` now live in `stdlib/pbai_max.sapp`. |
| API delegation | **SIMPLIFIED** | `tools/pbai_api.sapp` delegates `max_verify_generation` and `max_generate_verified` to `stdlib/pbai.sapp` / Max instead of carrying its own verifier. |
| Regression gate | **TIGHTENED** | Prompt regression now scores the verified/repaired selection and requires strict verification for pblang-like/code cases. |
| Distillation safety | **TIGHTENED** | Checkpoint-backed distillation records now include `verified_generation` and `teacher_text_source` (`original`, `repair`, or `fallback`) so students do not learn raw malformed Max output. |
| Cache policy | **VERSIONED** | Saved generation cache keys now include `generation_policy_version: "verified-repair-v1"`, invalidating old generation-policy cache slots. |
| Parser boundary | **DOCUMENTED** | Trusted fallback remains a temporary bridge: if `frontend_parse_safe/full` still rejects a known safe repair snippet, the record says `trusted_repair_fallback` rather than pretending the parser accepted it. |
| Server hardening | **ADDED** | `tools/pbai_server.sapp` removes wildcard CORS by default, supports `PBAI_SERVER_TOKEN`, and requires `--allow-ingest` for `ingest_path`. |

Verification:

```sh
pblang run tools/pbai_api.sapp --allow-all -- max_generate_verified
pblang run tools/pbai_api.sapp --allow-all -- max_verify_generation /tmp/no-such-request.json
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tests/semantics/pbai_test.sapp --allow-all
timeout 3 pblang run tools/pbai_server.sapp --allow-all -- 8099
```

Result: `max_generate_verified` returned a passing
`pbai.max.verified_ranked_generation.v1` repair-selected result, malformed empty
input was rejected by `max_verify_generation`, focused Max verification passed
(`pbai_max_pass`), pbAI facade verification passed (`pbai_pass`), and the local
server started cleanly before the timeout stopped it.

## 34. Bounded Pro/Mini/Nano student distillation started (2026-06-17)

The next pbAI-Max step after moving verified generation into core is now
implemented: distillation is no longer only a preview. Max can train bounded
pblang-native student checkpoints for Pro/Mini/Nano from verified teacher records.

| Item | Status | What changed |
|---|---|---|
| Student configs | **ADDED** | `pbai.max.distilled_student_config.v1` creates smaller tier-specific decoder configs: Pro, Mini, and Nano have separate context/layer/width budgets. |
| Student training | **ADDED** | `train_distilled_student(...)` builds a distillation corpus from `pbai.max.distillation_record.v1`, trains a smaller student with parallel cached-hidden LM-head updates, binary-adapter updates, and analytic deep-input updates. |
| Family training | **ADDED** | `distill_family(...)` trains Pro, Mini, and Nano through the same verified-teacher path and reports accepted student count. |
| Acceptance gate | **TIGHTENED** | `accepted_for_save` requires validation loss not to worsen and benchmark category thresholds to pass. |
| API access | **ADDED** | `stdlib/pbai.sapp` exposes `max_distill_train` / `max_distill_family`, and `tools/pbai_api.sapp` exposes matching API actions. |
| Ownership boundary | **KEPT** | Nano remains labelled as `pblang stdlib / Nano foundation`; Max policy and distillation orchestration stay in `AAP/ai/pbai`. |
| Regression coverage | **ADDED** | `pbai_max_test.sapp` now checks the student checkpoint contract, smaller Mini/Nano architectures, parallel training, binary/deep update paths, validation gate, and Nano ownership boundary. |

Verification:

```sh
pblang run tools/pbai_api.sapp --allow-all -- max_distill_train
pblang run tools/pbai_api.sapp --allow-all -- max_distill_family
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tests/semantics/pbai_test.sapp --allow-all
```

Result so far: the API smoke paths return
`pbai.max.distilled_student_checkpoint.v1` and `pbai.max.distillation_family.v1`
with accepted bounded students. Remaining work at that point: add checkpoint file
layout for the family and then scale beyond deterministic expected-token records.

## 35. Distilled family checkpoint layout added (2026-06-17)

Bounded Pro/Mini/Nano distillation can now persist the generated students instead
of returning only transient maps.

| Item | Status | What changed |
|---|---|---|
| Student save/load | **ADDED** | `save_distilled_student(...)` writes `student.json` plus `manifest.json`; `load_distilled_student(...)` reads the saved student checkpoint. |
| Student summary | **ADDED** | `distilled_student_summary(...)` extracts tier, params, vocab, architecture, losses, benchmark gate, teacher-source counts, and save acceptance without loading every detail into callers. |
| Family save | **ADDED** | `save_distilled_family(...)` writes `family.json`, `index.json`, and one saved student directory per tier. |
| Family summary | **ADDED** | `distilled_family_summary_saved(...)` reads the saved `index.json` contract. |
| Facade/API access | **ADDED** | `stdlib/pbai.sapp` exposes `max_distill_save_family` and `max_distill_family_summary`; `tools/pbai_api.sapp` exposes matching API actions. |
| Regression coverage | **ADDED** | `pbai_max_test.sapp` saves a tiny Pro/Mini/Nano family under `target/pbai-max-test/students`, checks `index.json`, loads Mini, and verifies the summary contract. |

Verification:

```sh
pblang run tools/pbai_api.sapp --allow-all -- max_distill_save_family /tmp/pbai-max-distill-save.json
pblang run tools/pbai_api.sapp --allow-all -- max_distill_family_summary /tmp/pbai-max-distill-summary.json
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tests/semantics/pbai_test.sapp --allow-all
```

Remaining work at that point: increase teacher data quality/volume, save larger
students from checkpoint-backed Max generations, and add downstream evaluation
that loads the saved student family as active Pro/Mini/Nano models.

## 36. Saved students can now evaluate and generate (2026-06-17)

The saved Pro/Mini/Nano family is now usable as loaded model artifacts, not only
as stored JSON.

| Item | Status | What changed |
|---|---|---|
| Student eval | **ADDED** | `evaluate_distilled_student(...)` and `evaluate_distilled_student_saved(...)` rebuild the distillation examples and report train/validation eval for a saved tier. |
| Family eval | **ADDED** | `evaluate_distilled_family_saved(...)` loads the saved family index and evaluates every saved student tier. |
| Student generation | **ADDED** | `generate_distilled_student(...)`, `generate_distilled_student_ranked(...)`, and saved wrappers generate from the selected saved student model using the same tokenizer/model path as Max generation. |
| Facade/API access | **ADDED** | `stdlib/pbai.sapp` and `tools/pbai_api.sapp` expose `max_distill_student_eval`, `max_distill_family_eval`, `max_distill_student_generate`, and `max_distill_student_generate_ranked`. |
| Regression coverage | **ADDED** | `pbai_max_test.sapp` loads the saved Mini student, evaluates it, generates ranked text from it, evaluates saved Nano, and evaluates the saved family. |

Verification:

```sh
pblang run tools/pbai_api.sapp --allow-all -- max_distill_student_eval
pblang run tools/pbai_api.sapp --allow-all -- max_distill_family_eval
pblang run tools/pbai_api.sapp --allow-all -- max_distill_student_generate
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tests/semantics/pbai_test.sapp --allow-all
```

Remaining work at that point: route real pbAI conversations through the saved
Pro/Mini/Nano tiers by confidence and task type, then scale teacher data and model
size.

## 37. Saved student family routing started (2026-06-17)

The saved Pro/Mini/Nano family now has a first pblang-native routing policy.

| Item | Status | What changed |
|---|---|---|
| Tier router | **ADDED** | `route_distilled_tier(prompt, dir)` returns `pbai.max.distilled_route.v1` with selected tier, reason, confidence, availability, and policy. |
| Routing policy | **ADDED** | Short/simple prompts route to Nano; general pblang/memory prompts route to Mini; code/tool/long-context prompts route to Pro. |
| Routed generation | **ADDED** | `generate_distilled_route_saved(...)` chooses a saved tier, runs that student's ranked generation, and returns the selected route plus generated text. |
| Routed evaluation | **ADDED** | `evaluate_distilled_route_saved(...)` evaluates the saved tier selected for the prompt. |
| Facade/API access | **ADDED** | `stdlib/pbai.sapp` and `tools/pbai_api.sapp` expose `max_distill_route`, `max_distill_route_generate`, and `max_distill_route_eval`. |
| Regression coverage | **ADDED** | `pbai_max_test.sapp` verifies Nano/Mini/Pro routing cases, routed generation, and routed evaluation. |

Verification:

```sh
pblang run tools/pbai_api.sapp --allow-all -- max_distill_route
pblang run tools/pbai_api.sapp --allow-all -- max_distill_route_generate
pblang run tools/pbai_api.sapp --allow-all -- max_distill_route_eval
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tests/semantics/pbai_test.sapp --allow-all
```

Remaining work: connect the routed student family to normal pbAI conversation
fallbacks, add confidence thresholds against Max/grounded skills, and train
larger student checkpoints from checkpoint-backed teacher generations.

## 38. One-command pbAI-Max pipeline script (2026-06-17)

The Max workflow now has a single pblang entry point for normal repeated runs.

| Item | Status | What changed |
|---|---|---|
| Pipeline script | **ADDED** | `tools/pbai_max_pipeline.sapp` runs the bounded Max workflow in-process. |
| Default workflow | **ADDED** | The default run saves the bounded Pro/Mini/Nano family, reads the saved index, evaluates Mini and the full family, routes a prompt, generates through the selected tier, and evaluates the routed tier. |
| Full mode | **ADDED** | A JSON request can set `"mode":"full"` or `"train_max":true` to train/save a fresh Max teacher checkpoint before distillation. |
| Output contract | **ADDED** | The script prints one `pbai.max.pipeline.v1` JSON report with settings, key outputs, and `pbai.max.pipeline_checks.v1`. |
| Config surface | **ADDED** | Request fields include `output_dir`, `teacher_path`, `epochs`, `lr`, `max_records`, `n`, `max_examples`, `prompt`, and `report_path`. |

Daily command:

```sh
pblang run tools/pbai_max_pipeline.sapp --allow-all
```

Verification:

```sh
pblang run tools/pbai_max_pipeline.sapp --allow-all
```

Result: the pipeline smoke returned `pbai.max.pipeline.v1` with
`checks.ok: true`, `saved_students: 3`, `summary_students: 3`, and
`family_eval_students: 3`.

## 39. Curated teacher answers improve pbAI-Max student data (2026-06-17)

The bounded student family now learns from richer checkpoint-free teacher records
instead of mostly one-token expected labels.

| Item | Status | What changed |
|---|---|---|
| Curated teachers | **ADDED** | `_curated_teacher_text(...)` provides multi-sentence teacher answers for pblang syntax, safe I/O, math, memory, retrieval, tools, safety, feedback, and path filtering cases. |
| Distillation source | **CHANGED** | Checkpoint-free `distillation_dataset(...)` now uses `teacher_text_source: "curated"` and `implementation_status: "curated-teacher-records"`. |
| Dataset metrics | **ADDED** | Distillation records include `teacher_text_chars`; datasets report `teacher_chars` and `avg_teacher_chars`. |
| Source accounting | **ADDED** | `pbai.max.distillation_teacher_sources.v1` now counts curated teacher records separately from expected/original/repair/fallback records. |
| Pipeline guard | **ADDED** | `tools/pbai_max_pipeline.sapp` now requires curated teacher records and `avg_teacher_chars >= 40` before `checks.ok` can pass. |
| Regression coverage | **ADDED** | `pbai_max_test.sapp` verifies curated teacher source and richer teacher text length. |

Verification:

```sh
pblang run tools/pbai_api.sapp --allow-all -- max_distill_preview
pblang run tools/pbai_api.sapp --allow-all -- max_distill_train
pblang run tools/pbai_max_pipeline.sapp --allow-all
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tests/semantics/pbai_test.sapp --allow-all
```

Result: `max_distill_preview` reported `curated-teacher-records` with average
teacher length around 169 chars, and `max_distill_train` still accepted the Mini
student with validation loss improving.

## 40. Shared pblang teacher-quality gate added to pbAI-Max (2026-06-19)

pbAI-Max distillation now depends on pblang's shared LLM-eval teacher-data gate,
not only local text-length checks.

| Item | Status | What changed |
|---|---|---|
| Shared evaluator | **USED** | `distillation_teacher_quality(...)` wraps pblang's `EVAL.teacher_dataset_quality(records, 40, 0.65)`, and `distillation_dataset(...)` stores the `pblang.llm.teacher_dataset_quality.v1` report. |
| Pipeline guard | **TIGHTENED** | `tools/pbai_max_pipeline.sapp` requires `teacher_quality_passed` before `checks.ok` can pass. |
| Core acceptance gate | **ADDED** | `distillation_acceptance_gate(...)` requires validation improvement, benchmark pass, and teacher-quality pass before `accepted_for_save`. |
| Contract coverage | **ADDED** | `pbai_max_test.sapp` asserts that Max distillation exposes and passes the shared pblang teacher-quality gate for every record, that weak/forbidden teacher text fails through the pbAI wrapper, and that acceptance fails independently with explicit reasons for validation regression, benchmark failure, and teacher-quality failure. |
| Boundary | **KEPT** | Generic record scoring lives in `pl-dev/pblang/stdlib/llm_eval.sapp`; pbAI-Max owns only its teacher records and pipeline policy. |

Verification:

```sh
pblang run tests/semantics/pbai_max_test.sapp --allow-all
pblang run tools/pbai_max_pipeline.sapp --allow-all
```

## 41. Ideal / Imaginary Variable NN target recorded (2026-06-20)

pbAI-Max now has an explicit research target for training real pblang neural
networks toward an aspirational mathematical ideal without claiming that the
ideal architecture is implemented.

| Item | Status | What changed |
|---|---|---|
| Status contract | **ADDED** | `ideal_neural_target()` returns `pbai.max.ideal_neural_target.v1` with `implementation_status: "research-target"`. |
| Math target | **DOCUMENTED** | The target names complex-valued state ideas, probability/uncertainty calibration, linear algebra over embeddings/projections/eigenspaces/adapters, and geometry/manifold views of memory, attention, routing, and concept distance. |
| Training direction | **DOCUMENTED** | The current pblang NNs remain the real student networks; future training should measure current behavior, project it into target coordinates, compute distance/error from the ideal, and reduce that distance through validated training. |
| Alignment objective | **ADDED** | `ideal_alignment_objective(...)` returns `pbai.max.ideal_alignment_objective.v1`, using pblang's shared weighted objective-distance helper as an objective proxy for validation loss, benchmark pass-rate, teacher quality, calibration error, and geometry distance. |
| Ideal math contract | **ADDED** | `ideal_math_contract()` exposes `pbai.max.ideal_math_contract.v1`, backed by `stdlib/pbai_ideal_nn.sapp`, and proves that one opt-in ideal step reduces composite ideality distance on a tiny model. |
| Safety boundary | **KEPT** | The contract explicitly says this is a research goal only and must not be reported as AGI/ASI or ideal-brain capability until implemented and verified. |
| Contract coverage | **ADDED** | `pbai_max_test.sapp` asserts the status contract, research-target status, complex-math substrate, train-toward-target wording, default remaining distance, standalone math contract, opt-in training boundary, and a passing custom objective set. |

## 42. Small fast trial/error reinforcement smoke added (2026-06-20)

pbAI-Max now has a cheap deterministic reinforcement-learning substrate for
small training experiments before larger continuation runs.

| Item | Status | What changed |
|---|---|---|
| Fast trial loop | **ADDED** | `fast_trial_reinforcement_smoke()` returns `pbai.max.fast_trial_reinforcement.v1`. |
| Trial/error policy | **ADDED** | The loop compares tiny candidate states against `ideal_alignment_objective(...)`, rewards lower objective distance, punishes worse objective distance, and keeps neutral no-op behavior visible. |
| pblang cooperation | **ADDED** | pbAI-Max now delegates trial candidate labeling and elite selection to pblang's shared `llm_eval.trial_candidate(...)` / `select_trial_elites(...)` helpers while preserving the pbAI status contract. |
| Previous/current state | **ADDED** | The report includes `previous_state`, `current_state`, each trial's previous/current distance, delta, reinforcement label, and selected `best_trial`. |
| Safety boundary | **KEPT** | The smoke loop is deterministic, tiny, and explicitly reports `mutates_checkpoint: false`; it does not save or update Max weights. |
| Status exposure | **ADDED** | `status()` now exposes `fast_trial_reinforcement` so tools and the website can show the small RL/trial contract. |
| Regression coverage | **ADDED** | `pbai_max_test.sapp` verifies reward/punish reporting, accepted improving trial behavior, and no checkpoint mutation. |

Verification:

```sh
pblang run tests/semantics/pbai_max_test.sapp --allow-all
```

Remaining work: wrap real tiny continuation candidates with this same
reward/punish contract, then allow checkpoint writes only after validation,
benchmark, teacher-quality, and verifier gates pass.

## 43. pbAI RL substrate connected to Max status (2026-06-20)

pbAI-Max now exposes the standalone pbAI reinforcement-learning substrate as an
honest status contract without turning it into checkpoint training yet.

| Item | Status | What changed |
|---|---|---|
| RL module integration | **ADDED** | `stdlib/pbai_max.sapp` imports `stdlib/pbai_rl.sapp` and exposes `rl_trial_learning_status()`. |
| Ideal-target reward | **ADDED** | The status smoke uses `pbai_rl.ideal_reward(...)`, where higher reward means lower ideality distance to the Ideal NN target. |
| Deterministic trial loop | **ADDED** | The status smoke runs `pbai_rl.train_toward_ideal_rl(...)` with a fixed seed and reports reward history, accepted count, and trainer format. |
| Safety boundary | **KEPT** | `rl_trial_learning_status()` is a tiny deterministic status smoke and reports `mutates_checkpoint: false`; it does not save or update Max checkpoints. |
| Facade/API access | **ADDED** | `stdlib/pbai.sapp` exposes `max_rl_status()`, and `tools/pbai_api.sapp` exposes the `max_rl_status` action for direct pblang-only scripts. |
| Regression coverage | **ADDED** | `pbai_max_test.sapp` verifies the RL status contract, source module, reward improvement, reward history length, and no checkpoint mutation. |

Verification:

```sh
pblang run tests/semantics/pbai_rl_test.sapp --allow-all
pblang run tools/pbai_api.sapp --allow-all -- max_rl_status
pblang run tests/semantics/pbai_max_test.sapp --allow-all
```
