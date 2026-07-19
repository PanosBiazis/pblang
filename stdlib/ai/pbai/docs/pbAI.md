# pbAI — pblang's native AI model family

> **Status: VISION / DESIGN (2026-05-30).** pbAI is a forward-looking design, not
> yet a shipped product. It is grounded in primitives pblang **already has today**
> — see [Foundations already in place](#foundations-already-in-place) — but the
> model family, training pipeline, and agent loop described here are the roadmap,
> not current reality. This document is the contract that future pbAI work builds
> toward. Where something exists now it is marked **(today)**; everything else is
> **(planned)**.

---

## Priority #1 Strategy

The current number-one strategy for pbAI-Max is
[`pbAI_Max_Strategy.md`](pbAI_Max_Strategy.md): build a pblang-native
self-improving intelligence loop, not just a single model checkpoint.

```text
data -> train -> evaluate -> use tools -> solve tasks -> verify -> remember -> improve model/runtime -> repeat
```

That loop is the project direction for making pbAI-Max stronger across all fields:
model architecture, memory, retrieval, tools, agents, verifier, training, evaluation,
runtime optimization, and safety all improve together. AGI/ASI remain research goals,
not current claims.

## 0. North Star Requirements

The target for pbAI is explicit:

- **A real LLM written in pblang** — not just a wrapper around another provider, and
  not just a rule-based chatbot. The runnable model path, training contracts, memory,
  agent loop, and model metadata are pblang-native.
- **Smart, expandable, portable, powerful, accurate, fast, efficient, and optimized** —
  these are product requirements, not slogans. Every model tier needs benchmarks for
  quality, latency, memory use, tokens/sec, loss, and task success.
- **Feature breadth like modern assistants** — chat, coding, debugging, refactoring,
  generation, summarization, review, test synthesis, documentation, retrieval, memory,
  tool use, planning, multimodal extensions, and self-improvement are in scope.
- **Understand all kinds of data** — text, code, docs, logs, diffs, tests, JSON, CSV,
  Markdown, binary/model artifacts, images, audio, video, sensor streams, and future
  modalities should all have a routing path through pbAI. Deep understanding comes from
  native parsers, embeddings, retrieval, model adapters, and plugin backends.
- **Human-brain-like but buildable** — pbAI should have perception, attention, working
  memory, episodic memory, long-term memory, goals, planning, critique, verification,
  reflection, consolidation, and learning. It should be better than a human at
  repeatable engineering tasks because it can use tools, tests, exact memory, and
  deterministic verification.
- **Neural brain core that runs everywhere** — the cognitive layer should use portable
  pblang neural networks, not host-specific services. Today `pbai_brain.sapp` includes
  a pure pblang `pbai.neural.brain.v1` MLP that scores attention, memory, planning,
  verification, tool-use, and learning needs from the current input.
- **Hierarchical agents** — pbAI should decompose work into sub-agents, atomic-agents,
  and sub-atomic-agents. Sub-agents handle planning/review/learning/tools, atomic-agents
  handle perception/attention/retrieval/verification/reflection, and sub-atomic-agents
  handle tokens, signals, gradients, and capability gates.
- **Substrate-like from tiny parts upward** — like physical systems emerging from very
  small particles, pbAI should compose from tiny pblang primitives into neural layers,
  memory layers, cognitive layers, agent behavior, and model-family tiers.
- **Ideal / Imaginary Variable NN target** — pbAI-Max should define an aspirational
  mathematical target space for neural behavior: complex-valued state ideas,
  probability and uncertainty calibration, linear algebra over embeddings/projections,
  and geometry/manifold views of memory, attention, routing, and concept distance.
  Current pblang neural networks are real student models; the ideal NN is a research
  target used to define losses, curricula, evaluations, and future architecture growth.
  Training should measure the current NN, project behavior into that target space,
  compute distance/error from the ideal, and improve only through validation,
  benchmark, teacher-quality, and safety gates. Current Max status exposes this as
  an `objective-proxy` alignment report, not as an implemented ideal architecture.
- **Small fast trial/error learning loop** — pbAI-Max should be able to test tiny
  training ideas quickly before spending time on larger continuation runs. Current Max
  status exposes `fast_trial_reinforcement_smoke()`, a deterministic no-checkpoint-write
  reward/punish/neutral loop that compares candidate states against the ideal alignment
  objective through pblang's shared `llm_eval.trial_candidate(...)` and
  `llm_eval.select_trial_elites(...)` helpers. This is a fast reinforcement-learning
  substrate for tests, not a claim that full RLHF/RLAIF training is complete.
- **Deterministic pbAI RL smoke** — `stdlib/pbai_rl.sapp` provides seeded
  trial-and-error primitives and an ideal-target trainer. Max exposes this through
  `rl_trial_learning_status()`, which runs a tiny deterministic no-checkpoint-write
  candidate loop and reports whether reward improved toward the Ideal NN target.
- **Honest current-state boundary** — today’s Nano is the first pblang-native neural LM
  and learning substrate. The goal is ChatGPT/Claude-class breadth over time, but the
  docs and tests must never claim that Nano already has frontier-model capability.
- **Build pbAI, not just pbAI-Nano** — pbAI-Nano is the small runnable boot layer used to
  prove pblang-native learning, serving, memory, and tests. The product is the full
  pbAI LLM family: tokenizer + transformer-scale decoder blocks + retrieval + tools +
  brain loop + memory + Max/Pro/Mini/Nano tiers.

This document tracks both sides: what exists now, and the concrete path to the larger
LLM/brain platform.

---

## 1. What pbAI is

**pbAI** is a family of AI models that are **small, powerful, fast, smart, accurate,
and efficient** — and, distinctively, **written in pblang itself** and **tool-driven
through the pb-mcp-server**. pbAI is the language's own intelligence: a model you can
run inside a pblang program, embed in the toolchain, and point at the language's own
source to **make pblang better**.

pbAI is an **agent, not just a text generator**: its hands are the **pb-mcp-server**
tool catalog (code indexing/search, git, build/test/run, AI/reasoning, algorithms,
and more — see [§7](#7-pb-mcp-server-tools)). It proposes; the tools observe and act on
the real project, under the server's own capability gating.

The product target is deliberately ambitious: pbAI should become a real LLM platform
written in pblang, with the breadth users expect from systems like ChatGPT, Claude, or
other modern assistants, while staying local-first and inspectable. That means:

- **Smart and accurate** — trained on curated pblang/code/general corpora, evaluated by
  deterministic benchmarks, and grounded by compiler/test/tool feedback.
- **Expandable** — new model tiers, adapters, retrieval corpora, tools, skills, and
  backends register without forking the core.
- **Portable** — the runnable path is pblang over stable tensor/file/runtime primitives;
  optional FFI/provider backends are plugins, not requirements.
- **Powerful** — coding, debugging, review, generation, summarization, retrieval,
  planning, tool use, memory, and self-improvement are first-class goals.
- **Fast, efficient, and optimized** — Nano/Mini tiers handle local interactive work;
  larger tiers are used only when the router needs more capability.

The current `pbai.lm.char.v1` Nano model is not presented as equivalent to frontier
LLMs. It is the first working pblang-native neural LM and live-learning substrate. The
roadmap scales that substrate toward transformer decoder blocks, richer tokenization,
larger corpora, better sampling, retrieval, tool use, and benchmarked assistant quality.

### Substrate-To-Brain Architecture

pbAI should grow from tiny composable primitives into higher intelligence, similar to
how physical systems build from very small parts into larger behavior. The engineering
version is:

| Layer | Role |
|-------|------|
| **Primitives** | pblang values, arrays, maps, files, tensor kernels, tokenizer atoms, random seeds, losses, gradients. |
| **Neural substrate** | embeddings, context windows, hidden states, logits, sampling, training examples, checkpoints. |
| **Memory substrate** | working memory, episodic events, long-term lessons, live corpus, retrieval corpora. |
| **Cognitive substrate** | portable pblang neural cognitive core, multimodal data perception, attention, goal state, planning, critique, verification, reflection, consolidation. |
| **Agent hierarchy** | root pbAI orchestrator, sub-agents, atomic-agents, and sub-atomic-agents. |
| **Agent substrate** | tools, tests, code search, patches, docs, project feedback, capability gates. |
| **Model family** | Nano/Mini/Pro/Max tiers sharing contracts, distillation paths, adapters, and benchmarks. |

This is the practical route to "human-brain-like but better": not by pretending to
simulate biology cell-for-cell, but by building reliable pblang-native primitives that
compose into attention, memory, self-correction, learning, tool use, and increasingly
capable model tiers.

Its primary jobs:

- **Coding** — generate, complete, and refactor pblang (and other) code.
- **Debugging** — read a failing run, a stack trace, or a diff and propose a fix.
- **And more** — code review, test synthesis, documentation, migration, semantic
  search over the codebase, and self-improvement of pblang's own compiler/stdlib —
  all carried out by **calling pb-mcp-server tools** to index, search, run, and edit.

pbAI is built to be **efficient first**: small enough to run locally and fast, while
staying accurate enough to be trusted on real engineering tasks. It is the opposite
of "call a giant remote model for everything" — though it can *also* delegate to
larger providers when a task genuinely needs them (see [§7](#7-pb-mcp-server-tools)).

---

## 2. The model family — a flagship and its distilled sub-models

pbAI is not a single model. It is a **family**:

```
                 ┌──────────────────────────────┐
                 │   pbAI-Max  (the flagship)    │   the best, largest pbAI model:
                 │   powerful · smart · accurate │   highest quality, trained on the
                 └──────────────┬───────────────┘    full corpus; the source of truth
                                │  distillation
        ┌───────────────────────┼───────────────────────┐
        ▼                       ▼                        ▼
  ┌───────────┐          ┌───────────┐            ┌───────────┐
  │  pbAI-Pro │          │ pbAI-Mini │            │ pbAI-Nano │   distilled SUB-MODELS:
  │  balanced │          │  fast     │            │  tiny     │   smaller, faster, cheaper —
  └───────────┘          └───────────┘            └───────────┘   each a focused student of
   IDE/agent use         inline complete          edge / embedded   pbAI-Max
```

- **pbAI-Max** — the flagship, the *best large model* in the family. It is the
  quality ceiling: the most powerful, smart, and accurate pbAI, trained on the full
  dataset. Everything else descends from it.
- **Sub-models (pbAI-Pro / -Mini / -Nano)** — each is a **sub-model of pbAI-Max**:
  a smaller, faster, more efficient model **distilled** from the flagship. A sub-model
  trades some capability for speed, footprint, and cost, and is specialized to a tier
  of use (interactive agent, inline completion, tiny/embedded). Because they share the
  flagship's behavior by construction, the family stays **consistent**: a fix learned
  by pbAI-Max propagates to its students on the next distillation.

This flagship-plus-distilled-students shape is the core of pbAI: **one best model
defining quality, many small efficient models delivering it where it's needed.**

### Repository ownership

The split is deliberate:

- **pblang repo** owns the reusable language/runtime foundation and the small
  pbAI-Nano boot layer: generic `stdlib:llm_*` libraries, the char/Nano proof path,
  tokenizer/data/math primitives, and semantic tests that keep pblang capable of
  running models.
- **`AAP/ai/pbai` repo** owns the pbAI product family, especially **pbAI-Max**:
  flagship tier policy, curated corpora, checkpoints, evaluation, memory/tools/agents,
  website/server integration, and distillation targets for Pro/Mini/Nano.

`stdlib/pbai_max.sapp` is the first pbAI-Max product module in this repo. It imports
pblang's `stdlib:llm_*` foundation, but the tier metadata and checkpoint contract remain
owned here.

---

## 3. Why pbAI is written in pblang

Writing the model in pblang is deliberate, and it pays off in three ways:

1. **Self-hosting all the way up.** pblang already self-hosts its compiler (it
   tokenizes, parses, compiles, and runs pblang — see `docs/CURRENT_STATUS.md`).
   pbAI extends self-hosting from the *toolchain* to the *intelligence*: the language
   ships the model that improves the language, all in one source tree.
2. **Code-native understanding.** Because pbAI is a pblang program, it has
   first-class access to pblang's own AST, IR, and bytecode via the self-hosted
   compiler (`stdlib/compiler/`). It can reason about code as structured data, not
   just text — parse a snippet, type-check it, even run it through `frontend_run`.
3. **One capability/security model.** pbAI runs under the same `VmLimits` /
   `FfiPolicy` / `StdlibConfig` sandbox as any other pblang program, so an embedded
   pbAI inherits pblang's capability-based security for free.

### Foundations already in place

pbAI is buildable because the numeric and integration primitives exist **today** as
pblang-callable builtins and crates:

| Need | What exists today |
|------|-------------------|
| Transformer math | `tensor_matmul`, `tensor_softmax`/`log_softmax`, `tensor_layer_norm`, `tensor_gelu`/`relu`/`gelu`/`sigmoid`/`tanh`, `tensor_embedding_lookup`, `tensor_cross_entropy`, `tensor_argmax`, `tensor_add/mul/scale/mean/sum` |
| Provider fallback / remote models | `ai_chat`, `ai_embed`, `ai_token_count` (OpenAI / Anthropic / Ollama / custom) in the `pblang-ai` crate; `stdlib/provider_registry.sapp` (`call_best`, `call_with_fallback`) |
| Retrieval (RAG) | `chunk_text`, `semantic_search`, vector math (`cosine_similarity`, `dot_product`) in `pblang-ai` |
| Classic ML helpers | `stdlib/machine_learning.sapp`, `stdlib/ml.sapp`, `stdlib/tensor.sapp` |
| Code understanding | the self-hosted compiler in `stdlib/compiler/` (tokenize → parse → typecheck → run) |
| Tool use | the **pb-mcp-server** tool catalog (see [§7](#7-pb-mcp-server-tools)) |

The tensor builtins above are exactly the kernels a decoder-only transformer needs
for inference (embedding lookup → N×(attention via matmul+softmax+layer_norm) →
projection → softmax/argmax sampling). pbAI's runtime is, at its core, a pblang
program orchestrating these kernels.

**Shipped today — pbAI's first working models (§65, §86, §87).**
- `stdlib/nn.sapp` — a trainable MLP (learns XOR).
- **`stdlib/pbai_tokenizer.sapp`** — pbAI's first native tokenizer contract:
  a deterministic, versioned `pbai.tokenizer.char.v1` codepoint tokenizer with
  explicit `<pad>` / `<bos>` / `<eos>` / `<unk>` special tokens, encode/decode,
  token strings, metadata, and model I/O descriptors. It is pure pblang and
  locked by `tests/semantics/pbai_tokenizer_test.sapp`.
- **`stdlib/pbai_brain.sapp`** — pbAI's first measurable cognitive loop:
  a portable pblang neural cognitive core, multimodal data perception, attention, goal state, bounded working memory, episodic
  memory, long-term memory promotion, task planning, risk critique, deterministic
  verification-signal parsing, reflection, and memory consolidation. It currently
  detects text, code, logs, diffs, tests, JSON, CSV, Markdown, image/audio/video hints,
  and binary/model artifact hints. It also exposes a versioned hierarchy:
  root pbAI -> sub-agents -> atomic-agents -> sub-atomic-agents. It is pure pblang and locked by
  `tests/semantics/pbai_brain_test.sapp`.
- **`stdlib/pbai.sapp`** — pbAI's IDE **assistant**: a neural intent classifier (on `nn.sapp`)
  plus pblang-grounded skills (explain / find-bugs / improve / comment / knowledge),
  **no external model, no network**. It **backs the IDE 🤖 AI panel** via a pblang gateway
  (`editors/ide/tools/ai/gateway.sapp`). It also exposes `self_eval(prompt)` for
  structured quality, confidence, risk, findings, and repair suggestions. Locked by
  `tests/semantics/pbai_test.sapp`.
- **`stdlib/pbai_lm.sapp`** — pbAI's first **generative** model: a character-level neural
  **language model** (embedding → tanh hidden → vocab logits → softmax), trained on
  next-char prediction with **manual backprop** (the `tensor_*` kernels are forward-only),
  generating autoregressively with the `tensor_argmax` kernel. It learns (cross-entropy
  3.29 → 0.58 on a small pblang corpus) and generates coherent pblang
  (`fn ` → `fn main() { let r; }`). The implementation now has a bottom-up pbAI-Nano
  path: `source_corpus()` (pblang-native training source), `train_nano()` (source →
  versioned `pbai.lm.char.v1` checkpoint), `checkpoint_generate()`, and
  `generate_nano()`. It also has **live local learning**: a staged baby→kid→teen→adult
  curriculum is built into the source corpus, and `learn_text(sample)` appends new
  examples to `stdlib/pbai_live_corpus.txt`, invalidates the Nano checkpoint cache, and
  retrains from the expanded corpus. Locked by `tests/semantics/pbai_lm_test.sapp`.
- **Top-level pbAI generation (today)** — `stdlib/pbai.sapp` exposes the lower model
  through `pbai.generate(prompt, n)` and reports the stack through `pbai.model_status()`:
  top = intent router / grounded skills / tools / brain loop; bottom = `pbai_lm.sapp`
  checkpoint, architecture metadata, and live-learning corpus stats.
  `pbai.learn(text, epochs)` is the facade for teaching the native LM new local examples.
  `pbai.think(prompt, observed_output)` exposes the cognitive-loop trace and answer.
  Generation requests to `pbai.respond(...)` and the gateway's `ai_ollama_generate`
  hint route to native pbAI-Nano instead of an external model.
- **`stdlib/pbai_max.sapp` (today)** — the first pbAI-Max product module in `AAP/ai/pbai`.
  It imports pblang's generic `stdlib:llm_*` foundation and owns the Max tier contract:
  source records, BPE tokenizer training, stacked multi-head decoder config, LM-head
  backprop training, analytic binary-adapter backprop into the bit-derived
  `binary_projection`, an analytic deep-input update stage for LM head, token
  embeddings, positional embeddings, and binary projection, live architecture update signals
  (`deeper`, `longer_context`, `stronger_binary`, `weaker_binary`),
  controlled growth policy parameters, adaptive error-reduction traces, feedback loops,
  previous/current feedback-state comparison, guessing loops, previous/current
  guess-state comparison, reward/punish/neutral reinforcement records for every
  comparison, elite selection that keeps the best candidate and throws lower-ranked
  results, train/validation split metrics, eval/perplexity, generation benchmark,
  prompt-level regression suite (`pbai.max.regression_suite.v1`) with expected-token,
  forbidden-token, collapse/repetition, pass-rate, and average-score checks,
  safe anti-collapse decoding through pblang `llm_attention.generate_safe(...)`,
  multi-candidate generation reranking through `llm_eval.choose_generation(...)`,
  sequence-loss scoring through `llm_eval.sequence_evaluate(...)`, output-shape
  scoring through `llm_eval.text_shape(...)`, a retrieval-grounded candidate pulled
  through `pbai_max.memory_retrieve(...)`, and a Max verifier score for expected/forbidden
  tokens, brace/paren sanity, and repetition. Max memory retrieval is pblang-native:
  `memory_index(...)` builds deterministic text embeddings for quality-filtered Max
  records and curriculum examples, indexes them with `llm_quant.turbovec_*`, returns
  sourced snippets through `pbai.max.memory_retrieval.v1`, and caches retrieval metadata
  under `target/pbai-cache/max-memory-retrieval`,
  LoRA-style low-rank adapter support through pblang `stdlib:llm_adapters.sapp`
  (`adapter_status`, `train_adapter`, and API actions `max_adapter_status` /
  `max_train_adapter`). The adapter path freezes base weights and trains a small
  LM-head delta first; Q/K/V/O, FFN, embedding, and quantized adapter targets remain
  future work. Live lessons now use this adapter route first:
  `train_live_adapter(...)` learns from `stdlib/pbai_max_live_lessons.txt`, and
  `teach_adapter(...)` records a lesson then trains only the low-rank adapter while
  reporting `touches_checkpoint: false`,
  deterministic benchmark suite support through `benchmark_cases`,
  `benchmark_status`, `benchmark_suite`, and the API action `max_benchmark`
  covering pblang coding, math, memory, retrieval, tools, and safety with
  category-specific thresholds,
  core verified generation through `strict_verify_generation`,
  `repair_generation_text`, `generate_verified_ranked`, `generate_verified_saved`,
  and the API action `max_generate_verified`; this rejects malformed code-like
  raw candidates, tries repairs, and records the temporary trusted-fallback parser
  boundary explicitly,
  a small fast trial/error reinforcement smoke through
  `fast_trial_reinforcement_smoke`; this compares reward, punish, and neutral
  candidate states against `ideal_alignment_objective`, delegates objective
  reinforcement and elite selection to pblang `llm_eval`, reports previous/current
  objective state, and never mutates checkpoints,
  deterministic pbAI RL status through `rl_trial_learning_status`; this calls
  `stdlib/pbai_rl.sapp` to run a tiny seeded trial-and-error loop toward the Ideal NN
  target and reports reward history without mutating checkpoints,
  distillation preview support through `distillation_targets`,
  `distillation_dataset`, `distillation_preview`, and the API action
  `max_distill_preview`; this emits Pro/Mini/Nano target specs plus
  `pbai.max.distillation_record.v1` teacher records, using curated multi-sentence
  teacher answers without a checkpoint or Max ranked-generation plus
  verified-generation traces with a checkpoint; datasets also carry the shared
  pblang `pblang.llm.teacher_dataset_quality.v1` gate through
  `distillation_teacher_quality(...)` so weak teacher records are rejected before
  student training,
  bounded student distillation through `train_distilled_student`,
  `distill_family`, `save_distilled_family`, saved index summaries, and API
  actions `max_distill_train` / `max_distill_family` /
  `max_distill_save_family` / `max_distill_family_summary`;
  these train smaller tier-specific pblang decoder students from verified teacher
  records with parallel LM-head training, binary-adapter updates, deep input
  updates, validation metrics, benchmark thresholds, teacher-quality gates, and
  a `pbai.max.distillation_acceptance_gate.v1` / `accepted_for_save` gate, then
  persist `family.json`, `index.json`, and one student checkpoint
  directory per tier; saved students can now be evaluated and used for bounded
  generation through `max_distill_student_eval`, `max_distill_family_eval`,
  `max_distill_student_generate`, and `max_distill_student_generate_ranked`;
  `max_distill_route`, `max_distill_route_generate`, and `max_distill_route_eval`
  choose Nano/Mini/Pro by task shape before using a saved student,
  and `tools/pbai_max_pipeline.sapp` runs the normal Max workflow from one pblang
  entry point: save the bounded student family, summarize/evaluate it, route a
  prompt, generate through the selected tier, verify rich curated teacher data,
  and emit `pbai.max.pipeline.v1`,
  quantized LM head, bit-level token metadata and binary LM example views from
  `stdlib:llm_binary.sapp`, sharded weight manifest, persistent checkpoint save/load
  under `checkpoints/pbai-max`, and a distillation contract for Pro/Mini/Nano.
  `pbai.model_status()` exposes this as `max`, and `tools/pbai_api.sapp` supports
  `max_status`, `max_rl_status`, `max_memory_status`, `max_memory`, `max_adapter_status`,
  `max_train_adapter`, `max_train_live_adapter`, `max_teach_adapter`, `max_train`,
  `max_save`, `max_load`, `max_summary`, `max_continue`, `max_generate`,
  `max_generate_ranked`, `max_generate_verified`, `max_eval`, `max_regression`,
  `max_benchmark`, `max_distill_preview`, `max_distill_train`,
  `max_distill_family`, `max_distill_save_family`, and
  `max_distill_family_summary`.
  `max_generate` returns the selected text for compatibility; `max_generate_ranked`
  returns every neural safe-decoding candidate plus the retrieval-grounded candidate,
  its decode settings or vector-memory retrieval source, text-shape score, verifier score,
  sequence loss/perplexity/accuracy, and final combined rank score. Repeated saved
  generation calls use the pblang `pbai_cache` layer (`max-generation`) keyed by the
  checkpoint signature, prompt, token count, and scoring settings, so unchanged
  checkpoints can answer repeated prompts without rerunning the full reranker.
  `bottom.learns` reports the live-learning capability while
  `bottom.training_signal.improved` reports the latest tiny status-training
  measurement; this keeps model status stable even when a cached/checkpointed
  smoke run does not lower loss.
  `max_continue`
  loads the current checkpoint, screens
  candidate learning-rate/epoch combinations in parallel on a quick sample with a
  head-only validation pass and `llm_eval.regression_proxy(...)`, ranks them through
  `llm_eval.selection_score(...)` plus the Max benchmark threshold gate, fully
  trains only the top finalists in parallel, reruns feedback/guess/elite selection,
  full prompt regression, and the deterministic benchmark suite, and saves only the
  best candidate when it improves validation loss without benchmark category
  regression or improves prompt regression plus benchmark thresholds within a small
  validation-loss tolerance;
  worse candidates are written to
  `candidate-rejected.json` and the best checkpoint is kept. Worker
  candidates never write the checkpoint; only the parent process saves the selected
  winner. This raises CPU utilization on multi-core machines while avoiding prompt
  generation/regression work for candidates that will be discarded, at the normal
  memory tradeoff that parallel candidates hold separate model copies. Tiny smoke
  runs with no finalist stage fall back to full candidates, so quick proxy
  checkpoints are never saved. Max training now uses a larger quality-filtered curated
  corpus plus a clean pblang curriculum interleaved through the bounded example budget;
  this includes selected pblang `llm_*`
  docs/modules/tests plus dense pblang function/error/test/agent-loop snippets. Examples
  are balanced per source record and prefer non-whitespace targets before falling back
  to filler examples, then use a representative validation split instead of taking a
  single contiguous validation block.
  Max-v2 also exposes live lessons and personality as model data: `max_teach` appends
  explicit lessons to `stdlib/pbai_max_live_lessons.txt`, then runs validation-gated
  continuation. Max's personality profile is stored as pblang-visible traits/rules
  (`clear`, `truthful`, `useful`, `curious`, `calm`, evidence-seeking, privacy-aware)
  rather than as a hard-coded chat line. LM-head training uses pblang workers to
  cache transformer hidden states, then applies deterministic head updates over that
  cache; deeper shared-parameter reducers are still future work.
  Derived training and generation metadata is cached through `stdlib/pbai_cache.sapp`
  under `target/pbai-cache`: Max writes training-summary entries, caches ranked
  generation results for repeated prompts, exposes cache status in `status()` and
  checkpoint summaries, and the API exposes `cache_status` / `cache_clear`. This cache
  is intentionally not the source of truth; clearing it does not delete checkpoints,
  weights, or live lessons. New entries use domain-separated SHA-256 metadata
  verification, preserve cached nil, support TTL and hard per-kind entry/byte
  quotas, and publish through staged rename with interrupted-write recovery.
  Existing v1 entries remain compatibility-readable but are reported as
  `legacy: true, verified: false` because their former 31-bit hash cannot prove
  collision resistance.
  Checkpoint inspection uses the pblang-native `max_summary` action, and quality gating
  uses `max_regression` plus `max_benchmark`; neither path needs Node.js or external JSON helper scripts.
  The training corpus now pulls bounded slices from pbAI docs, stdlib, tests, and
  built-in Max records. Locked by `tests/semantics/pbai_max_test.sapp`.
- **Web test console (today)** — `website/index.html` loads the pblang-generated
  fixture from `website/data/pbai-demo.json` and provides browser controls for
  response, analysis, generation, tokenizer, and brain-loop checks. When served through
  `tools/pbai_server.sapp`, the website calls live pblang pbAI APIs for every action
  and falls back to the browser mirror only when the server is unavailable. Refresh the
  fixture with `pblang run demo/pbai_demo.sapp --allow-all > website/data/pbai-demo.json`.
  The LLM structure canvas supports drag rotation, mouse-wheel zoom, plus/minus/reset
  controls, double-click reset, and live data packets for respond/analyze/think/learn.
- **pblang-only local server (today)** — `tools/pbai_server.sapp` serves the website
  and API without Node, Python, or an external model:
  `pblang run tools/pbai_server.sapp --allow-all -- 8088`, then open
  `http://127.0.0.1:8088/website/`. API endpoints include `/api/health` and
  `/api/pbai/respond`, `/api/pbai/analyze`, `/api/pbai/think`, `/api/pbai/generate`,
  `/api/pbai/tokenize`, `/api/pbai/learn`, `/api/pbai/ingest_path`, and
  `/api/pbai/status`.

These are the small end of the family — analysis **and** generation, all in pblang. The
transformer scale-up (attention/more layers over the tensor kernels above, larger corpora,
temperature sampling) is the roadmap below.

### Live Learning Contract Today

pbAI now learns in the same conservative way a child learns from curriculum plus
experience, but implemented as deterministic local model training:

1. **Baby curriculum** — symbols, tiny facts, identity functions, boolean returns.
2. **Kid curriculum** — pblang syntax, `print x;`, `map_has`, safe division, fallible I/O.
3. **Teen curriculum** — tests, code review patterns, capabilities, reusable helpers.
4. **Adult curriculum** — engineering knowledge: small functions, named constants,
   capability review, deterministic verification, and pblang gotchas.
5. **Live experience** — explicit calls to `pbai.learn(text, epochs)` append new local
   examples to `stdlib/pbai_live_corpus.txt` and retrain pbAI-Nano from the full corpus.

The built-in corpus now includes pblang facts, safe coding idioms, assistant behavior
lessons, multimodal data-routing lessons, and review/fix examples. In the live website,
teach pbAI by sending:

```text
learn: <new pblang fact, project rule, correction, example, or test>
learn <short fact such as 1+1=2>
```

The local server stores the lesson in `stdlib/pbai_live_corpus.txt`, retrains Nano, and
reports the before/after loss. This is intentionally explicit: pbAI learns when the user
chooses to teach it, not by silently absorbing every chat message.

The website also has a **Live Learning** panel. `Learn Text` sends the textarea lesson
directly to `/api/pbai/learn`. The `Files` and `Folder` controls read selected browser
files locally and stream them to the same pblang endpoint:

- text/code/docs are learned as bounded content chunks
- large or binary/unknown files are learned as metadata entries until a deep parser is
  added for that modality
- common token/password/API-key/private-key lines are redacted before text chunks are
  persisted to live memory; ingestion reports the count as `stats.redactions`
- folder ingestion is capped per batch so the browser and local pblang server remain
  responsive

After ingestion, learned filenames, paths, code symbols, and text chunks are available to
conversation through the live-memory retrieval path.

For server-side local paths, the website's `Learn Path` field calls
`/api/pbai/ingest_path`, which maps to `pbai.ingest_path(path, max_files, max_depth,
epochs)`. This lets the pblang server learn bounded folders such as `ai/max`,
`datasets/data`, or `../projects/Sp_AIModel` without using a browser file picker.

The same data-feed path is available from the command line:

```bash
pblang run tools/pbai_feed.sapp --allow-all -- lesson "project rule: prefer small verified pblang examples"
pblang run tools/pbai_feed.sapp --allow-all -- path tests/fixtures 10 2
pblang run tools/pbai_feed.sapp --allow-all -- quick
pblang run tools/pbai_feed.sapp --allow-all -- drives_survey
pblang run tools/pbai_feed.sapp --allow-all -- drives 10 2
```

`quick` samples bounded slices from pbAI docs/stdlib, AAP datasets/training folders, and
AI reference folders. It is deliberately capped; feeding an entire drive should be a
curated training job, not an accidental chat action.

The current local source map is documented in `docs/pbAI_source_audit.md`. That audit
lists the pbAI modules, useful AAP corpora/projects, nearby sibling projects, configurable
drive/device feed roots, and the exclusion rules for credentials and generated noise.

`drives_survey` learns a bounded top-level map of local data roots from
`PBAI_FEED_ROOTS`, using semicolon-separated paths so MTP/device paths can contain
colons. If the variable is unset, the tool uses safe project-relative defaults plus
`/mnt/win`.
`drives N D` then samples at most `N` files per root to depth `D`, skipping dot folders,
caches, build outputs, common generated directories, installers/archives, and obvious
credential/token/key files. This unlocks laptop-wide data discovery while keeping
full-drive training explicit and controlled.

Live behavior is not only retraining. `stdlib/pbai.sapp` also performs live-memory
retrieval on every response: it searches `stdlib/pbai_live_corpus.txt` for lessons whose
words overlap the current prompt and can answer from those local lessons. `analyze(...)`
and `think(...)` expose the matched live-memory entries so the website can show when a
response is grounded in learned local state.

The small intent classifier no longer writes trained weights into the installed
or locked stdlib tree. `stdlib:model_cache.sapp` validates model version, layer
dimensions, numeric values, and serialized-size bounds before loading, then uses
staged publication in the per-user PBLang cache. `PBLANG_PBAI_LEGACY_MODEL_CACHE`
can override the compatibility classifier path. The bundled
`stdlib/pbai_model.json` is read-only reference data.

This is not yet a large human-level model. It is a real pblang-written neural LM with
live local adaptation, a versioned checkpoint format, and regression tests proving that
training lowers loss after new experience is added. The next scale step is a decoder
block with attention over the same pblang tensor kernels, larger curated corpora, and
benchmarked sampling quality.

---

## 4. What pbAI is for — the self-improvement loop

pbAI exists to **make pblang better**, and the cleanest way to express that is a loop
where the language's own model improves the language:

```
   pblang source / tests / diffs / failures
                 │
                 ▼
        ┌──────────────────┐    reads code as AST/IR via stdlib/compiler
        │      pbAI         │    runs/inspects via frontend_run
        │ (code · debug ·   │    searches via semantic_search + code_index
        │  review · tests)  │    acts via pb-mcp-server tools
        └────────┬─────────┘
                 │  proposes patches, tests, docs
                 ▼
   pblang compiler / stdlib / docs  ──►  better pblang  ──►  better training data
                 ▲                                                   │
                 └───────────────────────────────────────────────────┘
```

Concretely, pbAI targets:

- **Coding** — write new stdlib modules, fill in compiler passes, scaffold tests.
- **Debugging** — given a `RuntimeError` + span + stack trace (pblang already emits
  these), localize the fault and propose a minimal fix; re-run via `frontend_run` to
  verify.
- **Review** — diff-aware critique using the lint/typeck crates as oracles.
- **Test synthesis** — generate `tests/semantics/*.sapp` cases for new behavior.
- **Docs** — keep documents like this one in sync with the code (exactly the kind of
  audit this file was produced under).

Every improvement pbAI lands becomes cleaner code and better examples — which become
better training data for the next pbAI-Max, closing the loop.

---

## 5. pbAI is expandable

pbAI is designed to **grow without rewrites** — expandable along every axis:

- **More models / bigger sizes.** New tiers slot into the family (a larger pbAI-Max,
  or new distilled sub-models) without changing callers — the same pbAI API serves
  any tier.
- **Pluggable backends.** Run a pblang-native model (tensor builtins), an ONNX/GGUF
  model via FFI/`onnx_infer`, or a remote provider via `ai_chat` — chosen per task,
  swappable behind one interface.
- **Skills / tools / plugins.** New capabilities are added as tools and skills (see
  [§7](#7-pb-mcp-server-tools)), not as core surgery. pbAI discovers and uses them at
  runtime.
- **Domain adapters.** Fine-tune or adapter-tune a sub-model for a domain (a specific
  stdlib area, a downstream project's codebase) and register it alongside the others.
- **Retrieval corpora.** Point pbAI's RAG layer at new sources (a new repo, new docs)
  to expand what it knows without retraining.

The contract: **anything pbAI can do should be extendable by adding a model, a
backend, a tool, an adapter, or a corpus — never by forking the core.**

---

## 6. Architecture sketch (planned)

```
┌──────────────────────────────────────────────────────────────────┐
│                            pbAI (pblang)                          │
│                                                                    │
│  ┌──────────────┐   ┌───────────────┐   ┌──────────────────────┐  │
│  │  Agent loop  │──▶│  Model router │──▶│  Inference backends   │  │
│  │ plan→act→    │   │ pick tier:    │   │ • pblang tensor model │  │
│  │ observe→     │   │ Max/Pro/Mini/ │   │ • ONNX/GGUF via FFI   │  │
│  │ verify       │   │ Nano + remote │   │ • remote (ai_chat)    │  │
│  └──────┬───────┘   └───────────────┘   └──────────────────────┘  │
│         │                                                          │
│         ├─▶ Code sense:  stdlib/compiler (AST/IR), frontend_run    │
│         ├─▶ Retrieval:   semantic_search, code_index, chunk_text   │
│         └─▶ Tools:       pb-mcp-server catalog (see §7)            │
│                                                                    │
│  Runs under pblang's VmLimits / FfiPolicy / StdlibConfig sandbox.  │
└──────────────────────────────────────────────────────────────────┘
```

- **Agent loop** — plan → act (call a tool / edit / run) → observe → verify, with the
  compiler and test suites as ground-truth oracles (a proposed fix is only "done"
  when `frontend_run` / the relevant cargo suite agrees).
- **Model router** — picks the cheapest model tier that will succeed, escalating to
  pbAI-Max (or a remote provider) only when a sub-model is not confident enough.
- **Inference backends** — the pluggable layer from [§5](#5-pbai-is-expandable).

---

## 7. pb-mcp-server tools

pbAI is not just a text generator — it is an **agent**, and its hands are the
**pb-mcp-server** tool catalog (`AAP/tools/rust/pb-mcp-server`, a separate companion
server). The current local audit shows a 500+ tool surface plus a Cargo workspace for
the MCP core, gateway, LSP bridge, network node, and dashboard. pbAI is designed to
drive that surface, including:

- **Code intelligence** — `code_index_build/search`, `code_index_file_symbols`,
  `grep_code`, `find_files`, `codebase_index`, `ast` dumps, complexity/dead-code hints.
- **Version control** — the `git_*` family (status, diff, log, blame, branch, commit,
  worktree) and PR/MR helpers.
- **AI / reasoning** — `ai_*` (chain-of-thought, debate, self-critique, semantic
  search, structured extract), `reasoning_*`, `thinking_*`.
- **Build / test / run** — `project_cargo_test`, `project_pytest`, `sandbox_*`,
  `terminal_run`.
- **Algorithms / data** — `algo_*`, `data_*`, `db_*`, `convert_*`, plus domains like
  audio/image/OCR/net/protect.
- **Media / neural / analysis** — OCR, screenshots, speech/audio, image tools,
  `neural_inference`, tokenization/embedding helpers, log clustering, and charts.
- **Agent orchestration** — `agent_send`, `ask_agents`, fan-out/pipeline, handoffs,
  and consensus proposals/voting for sub-agent work.

pbAI uses these tools to **observe and act on the real project**: index the codebase,
read diffs, run tests, search semantically, and apply changes — under the same
capability gating the MCP server already enforces (e.g. write tools blocked on the
`public` network profile). The model proposes; the tools verify and execute.

> Note: pb-mcp-server is a **separate companion repository**, versioned and developed
> independently of the pblang core. pbAI integrates with it as a client; it does not
> require modifying that server.

The pblang-native side of the same audit is exposed through
`stdlib/pbai_tools.sapp` and `pbAI.model_status().tool_substrate`. It records:

- `pl-dev/pblang/stdlib` data readers: `json.sapp`, `csv.sapp`, `dataframe.sapp`,
  `serialization.sapp`, `encoding.sapp`, `regex.sapp`.
- portable neural/math modules: `nn.sapp`, `ml.sapp`, `machine_learning.sapp`,
  `tensor.sapp`, `matrix.sapp`, `vector.sapp`, `stat.sapp`.
- compiler/code intelligence modules: tokenizer, parser, type checker, optimizer,
  dataflow and call-graph analysis.
- runtime/server modules: HTTP/server/web framework, `pb_mcp.sapp`, filesystem,
  process, thread pool, and async support.
- AAP learning corpora: TinyStories, books, PG19, BookCorpus-clean,
  combined-books text, GloVe embeddings, and tokenizer assets.
- AAP domain folders: `training/*` language examples, `cyber-security/*`,
  `data-science/*`, `robotics/*`, `quantum/*`, `web/job-tracker` docs,
  and `system-tools/*` as reference/evaluation data for cross-language and domain
  reasoning.
- AAP AI references: Delphi/C# `AIChatService`, chat-history managers,
  `UniversalFileParserUnit`, `FinancialRAG`, `aipy` scripts, and `ai/max`
  tokenizer/artifact files as design lessons only.
- Nearby projects outside AAP, such as `Sp_AIModel`, `aipy`, `max`, `chatbot`,
  `TesterStudio`, `WordDetect`, and `progrCS/RealAIVisualizer`, are reference-only
  sources for examples, visualizer ideas, and evaluation prompts.

---

## 8. External data and prior work boundaries

Local AI folders outside this repo — for example **`../AAP/ai/`** and
**`../AAP/datasets/data/`** on the development machine — are useful inputs, but they are
not pbAI itself. pbAI must be redesigned from the beginning with a pblang-native
architecture whose first priorities are expandability, accuracy, speed, efficiency,
portability, scalability, and clean tool integration.

The boundary is strict:

- **Use external folders as data/reference only.** They can provide corpora,
  tokenizer experiments, training scripts, checkpoint-conversion lessons, evaluation
  prompts, and implementation notes.
- **Do not use or replace pbAI with an old model.** Existing checkpoints, Pascal
  prototypes, Python trainers, or provider wrappers are not the pbAI architecture and
  are not the source of truth for pbAI behavior.
- **Do not make a checkpoint the flagship.** pbAI-Max is trained or assembled for the
  new architecture; older models may be used only as baselines, regression fixtures, or
  optional comparison teachers during evaluation.
- **Keep pbAI portable.** The runnable inference path lives in pblang over stable tensor
  and file-format primitives. Optional ONNX/GGUF/remote backends remain plugins behind
  the backend interface, never required by the core.

| External input | Path | Allowed use |
|----------------|------|-------------|
| **Text/code datasets** | `AAP/datasets/data/`, the pblang repo itself, stdlib/tests/docs/compiler sources | Pretraining and evaluation corpora. pblang code/debugging data is mandatory for the coding-focused tiers. |
| **Tokenizer experiments** | `AAP/ai/max/max_model/tokenizer.json`, tokenizer training scripts | Reference material for designing a pblang-native tokenizer. The final tokenizer should be versioned, testable, and decoupled from any old checkpoint. |
| **Training pipeline scripts** | `AAP/ai/max/trainer_max.py`, `train_tokenizer.py`, `prep_books.py`, `tokenize_data.py` | Data preparation and reproducibility patterns. Reuse ideas and formats, not the old model contract. |
| **Prior checkpoints** | `AAP/ai/max/max_model/model.safetensors` and similar artifacts | Optional evaluation baselines or temporary teacher signals. They must not define pbAI-Max, pbAI's architecture, or the public API. |
| **Prototype inference code** | `AAP/ai/max/NeuralNetwork.pas`, `AAP/ai/Sp_AIModel/SpAI/NeuralNetwork.pas` | Historical reference for simple inference loops and file I/O. Do not direct-port it as pbAI's architecture. |
| **Provider/tool-call prototypes** | `AAP/ai/max/AIChatService.pas` | Reference for retry/backoff and JSON tool envelopes. Core pbAI tool use is through the pblang API and pb-mcp-server. |

**Practical first step (P0):** specify the new pbAI inference contract first: tokenizer
format, weight format, decoder block interface, sampling API, model metadata, benchmark
suite, prompt-regression suite, and golden tests. This is now started: pblang owns the reusable `llm_*`
foundation and Nano path, while `AAP/ai/pbai` owns `stdlib/pbai_max.sapp`, the Max
checkpoint contract, persistence, eval, generation, and distillation policy. Existing
checkpoints are used only to compare quality and speed.

## 9. Roadmap (planned)

| Phase | Goal |
|-------|------|
| **P0 — Fresh inference contract** | Started: pblang `llm_*` foundation + `AAP/ai/pbai/stdlib/pbai_max.sapp` define tokenizer, decoder, sampling, checkpoint, persistence, eval, benchmark, prompt regression, binary-token, live-architecture, feedback-loop, and guessing-loop contracts. No old checkpoint defines this contract. |
| **P1 — pbAI-Nano/-Mini** | Train the first fresh tiny + small models for pblang completion/debugging on pblang/code data plus selected external corpora; benchmark accuracy, latency, memory, and tokens/sec; lock with drift tests. |
| **P2 — Agent loop + tools** | Wire the plan→act→verify loop to `frontend_run`, the lint/typeck oracles, and the pb-mcp-server tool catalog. |
| **P3 — pbAI-Max + distillation** | Train/assemble the new flagship on the full curated corpus; scale the implemented saved Pro/Mini/Nano student-family checkpoints from bounded semantic-test data into larger curated teacher datasets. |
| **P4 — Self-improvement loop** | pbAI lands reviewed patches to pblang's own compiler/stdlib/docs; its successes feed the next pbAI-Max. |
| **P5 — Expandability surface** | Stable plugin/adapter/backend/corpus registration APIs so the family grows without core forks. |

Each phase is gated the pblang way: a feature is "done" only when locked by tests
(`tests/*.rs` / `tests/semantics/*.sapp` / a new `tests/pbai_*` suite) and verified
green, exactly as the self-hosting milestones (PrjAnalysis §§40–45) were.

---

## 9.1 Shipped assistant intelligence

The current `stdlib/pbai.sapp` assistant is a pblang-grounded local model plus
deterministic skills:

- `classify(prompt)` routes to explain/bugs/improve/comment/general through the cached
  neural intent classifier.
- `classify_scores(prompt)` returns the intent name and raw score vector for IDE/tooling
  diagnostics.
- `analyze(prompt)` returns structured metadata: intent, scores, whether code was found,
  quality score, sensitive-capability risk summary, and scan metrics.
- Reviews flag pblang-specific issues: invalid `print(x)`, missing-key map reads,
  string ordering comparisons, unchecked `exec`/`ffi`/`env`, unbalanced delimiters,
  recursion, nesting depth, TODO/FIXME markers, and I/O without `try/catch`.
- The native IDE exposes this through EXPLAIN / BUGS / IMPROVE / QUALITY / COMMENT
  quick actions.

Locked by `tests/semantics/pbai_test.sapp`, `pbai_lm_test.sapp`, and the native
IDE UI smoke test.

## 9.2 pblang-native MCP bridge

`stdlib/pb_mcp.sapp` is the first conversion slice of the Rust
`editors/ide/tools/mcpservers/pb-mcp-server` into pblang. It implements the safe
core-code tools directly in pblang and exposes them to pbAI:

- `workspace_context`
- `tool_catalog`
- `read_file`
- `read_file_tail`
- `list_files`
- `project_tree`
- `grep_code`

pbAI imports this bridge and exposes `pbAI.tool_catalog()` / `pbAI.tool_call(name,args)`.
The full Rust MCP protocol daemon still owns the larger 500+ tool catalog, gateway,
terminal/local-power profile, bus, OCR/ML/network features, and IDE integrations; those
should be ported incrementally after the pblang API stabilizes.

Locked by `tests/semantics/pb_mcp_bridge_test.sapp`.

---

## 10. Relationship to existing pieces

- **`pblang-ai` crate / `ai_chat` / `ai_embed`** — pbAI's *remote* backend and
  embedding/RAG layer; pbAI is the native, pblang-written counterpart that can run
  without a remote provider.
- **`stdlib/compiler/` (self-hosted compiler)** — pbAI's code-understanding engine
  and its ground-truth oracle for "does this code compile/run?".
- **tensor builtins** — pbAI's native inference kernels.
- **pb-mcp-server** — pbAI's tool/agent substrate.
- **`docs/CURRENT_STATUS.md` / `docs/Future_plans.md`** — where pbAI's status will be
  tracked as it moves from this design into shipped phases.

---

*pbAI: the language's own model — small, powerful, fast, smart, accurate, efficient;
written in pblang; a flagship and its distilled sub-models; expandable; tool-driven
through pb-mcp-server; built to make pblang better.*
