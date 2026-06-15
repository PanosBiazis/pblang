# pbAI — pblang's native AI model family

> **Status: VISION / DESIGN (2026-05-30).** pbAI is a forward-looking design, not
> yet a shipped product. It is grounded in primitives pblang **already has today**
> — see [Foundations already in place](#foundations-already-in-place) — but the
> model family, training pipeline, and agent loop described here are the roadmap,
> not current reality. This document is the contract that future pbAI work builds
> toward. Where something exists now it is marked **(today)**; everything else is
> **(planned)**.

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
- **`stdlib/llm_core.sapp`** — reusable LLM math/training substrate: deterministic
  initialization, vector/matrix kernels, softmax, cross-entropy/perplexity, SGD, Adam,
  checkpoint save/load, and benchmark wrappers. Larger dot/vector/matrix operations
  plus selected softmax/argmax/ReLU/layernorm helpers can route through native VM
  `tensor_*` kernels while the public API stays normal pblang arrays.
- **`stdlib/llm_tokenizer.sapp`** — pblang-native tokenizer contracts: char tokenizer,
  small trainable BPE tokenizer, encode/decode, metadata, and JSON save/load.
- **`stdlib/llm_data.sapp`** — corpus helpers: text detection, secret/noise exclusion,
  chunking, next-token example packing, train/validation split, corpus records,
  curriculum example repetition/interleaving, and bounded representative sampling
  for faster pbAI-Max diagnostics without hiding
  original corpus sizes. Semantics tests use these bounded samples for fast subsystem
  diagnostics; real training/evaluation should still use larger curated train/valid
  sets.
- **`stdlib/llm_transformer.sapp`** — the first reusable decoder-transformer substrate:
  token/position embeddings, causal self-attention, feed-forward block, layer norm,
  vocab logits, sampling hooks, generation, quantization helpers, and summary metadata.
  Locked by `tests/semantics/llm_foundation_test.sapp`.
- **Scale-up LLM foundation modules (today)** — `stdlib/llm_attention.sapp` adds stacked
  multi-layer/multi-head decoder attention, Gemma-3-style local/global attention
  planning (`gemma3_config`, `attention_plan`) with grouped-query KV sharing, and
  Gemma-4-style planning (`gemma4_config`) for per-layer embeddings, MoE loaded-vs-active
  parameter accounting, MTP/speculative decoding, native system prompts, thinking-mode
  prompt templates, multimodal direct projections, and memory estimates. It also keeps
  live architecture updates (`deeper`, `longer_context`, `stronger_binary`,
  `weaker_binary`); `stdlib/llm_train.sapp` adds
  next-token backprop for the LM head and analytic binary-adapter backprop into
  `binary_projection` over frozen transformer states, plus an analytic deep-input
  update stage for token embeddings, positional embeddings, binary projection, and the
  LM head. This replaces the active finite-difference probe in the pbAI-Max training
  path; full cached Q/K/V/O attention, FFN, residual, and layernorm gradients are the
  next training stage. `llm_train` also provides adaptive error reduction,
  feedback loops with previous/current feedback-state evaluation, and guessing loops
  with previous/current guess-state evaluation. Both comparison paths emit
  reinforcement records: reward, punish, or neutral; `select_elite` ranks candidates,
  keeps the best, throws lower-ranked results, and exposes the elite state as the next
  generation seed;
  `stdlib/llm_stream.sapp` adds bounded streaming corpora; `stdlib/llm_weights.sapp`
  adds sharded weight manifests; `stdlib/llm_tokenizer_train.sapp` adds bounded
  corpus tokenizer training; `stdlib/llm_eval.sapp` adds loss/perplexity/accuracy,
  generation benchmarks, sequence-level scoring, text-shape scoring for cleaner
  generated code, cheap regression proxies for fast candidate screening,
  validation/regression selection scoring, generation-quality scoring, and
  candidate-generation reranking;
  `stdlib/llm_quant.sapp` adds row-wise quantized inference helpers plus pblang
  TurboQuant-style vector compression, first-batch TQ+ calibration, length-renormalized
  vector scoring metadata, 1-bit QJL residual sketches, and TurboVec-style compressed
  vector search with online add, stable ids, filtered search, removal, and manifest
  metadata; `stdlib/llm_binary.sapp` adds the bit-level token substrate: token-id bit
  width, ID<->bits round-trips, token bitstreams, binary LM example views, byte-bit
  conversion, and hamming/transition features; `stdlib/llm_offload.sapp` adds an
  AirLLM-style low-memory inference planner: KV-cache reservation, compressed KV-cache
  sizing, resident-layer selection, layer streaming, prefetch hints, offload manifests,
  and bounded microbatch sizing. This is the planning layer for future shard/device execution,
  not a claim that full GPU kernels are implemented in pblang today. Locked by
  `tests/semantics/llm_max_foundation_test.sapp`. These are generic pblang libraries;
  the pbAI-Max product lives outside pblang in `AAP/ai/pbai`.
- **Resource use note (today)** — pbAI-Max continuation now uses pblang `spawn`/`join`
  to evaluate independent candidates in parallel: quick screening workers run first,
  then only top finalists run on the full example set. The parent process is the only
  checkpoint writer. This raises CPU utilization above a single core without changing
  pblang's VM semantics; memory use rises while workers hold separate candidate models.
- **`stdlib/pbai.sapp`** — pbAI's IDE **assistant**: a neural intent classifier (on `nn.sapp`)
  plus pblang-grounded skills (explain / find-bugs / improve / comment / knowledge),
  **no external model, no network**. It **backs the IDE 🤖 AI panel** via a pblang gateway
  (`editors/ide/tools/ai/gateway.sapp`). Locked by `tests/semantics/pbai_test.sapp`.
- **`stdlib/pbai_lm.sapp`** — pbAI's first **generative** model: a character-level neural
  **language model** (embedding → tanh hidden → vocab logits → softmax), trained on
  next-char prediction with **manual backprop** (the `tensor_*` kernels are forward-only),
  generating autoregressively with the `tensor_argmax` kernel. It learns (cross-entropy
  3.29 → 0.58 on a small pblang corpus) and generates coherent pblang
  (`fn ` → `fn main() { let r; }`). The implementation now has a bottom-up pbAI-Nano
  path: `source_corpus()` (pblang-native training source), `train_nano()` (source →
  versioned `pbai.lm.char.v1` checkpoint), `checkpoint_generate()`, and
  `generate_nano()`. Locked by `tests/semantics/pbai_lm_test.sapp`.
- **Top-level pbAI generation (today)** — `stdlib/pbai.sapp` exposes the lower model
  through `pbai.generate(prompt, n)` and reports the stack through `pbai.model_status()`:
  top = intent router / grounded skills / tools; bottom = `pbai_lm.sapp` checkpoint and
  architecture metadata. Generation requests to `pbai.respond(...)` and the gateway's
  `ai_ollama_generate` hint route to native pbAI-Nano instead of an external model.

These are the small end of the family — analysis, generation, and now the first reusable
transformer foundation, all in pblang. Full training scale-up (multi-layer attention,
larger corpora, benchmarks, and distillation) is the roadmap below.

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
  any tier. The current outside-project Max tier in `AAP/ai/pbai` owns checkpoint
  policy and prompt-level regression gates, while this pblang repo owns the reusable
  `llm_*` foundations and Nano-compatible primitives.
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
**pb-mcp-server** tool catalog (`editors/ide/tools/mcpservers/pb-mcp-server`, a
separate companion server). That server already exposes a large, structured tool
surface that pbAI is designed to drive, including:

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

pbAI uses these tools to **observe and act on the real project**: index the codebase,
read diffs, run tests, search semantically, and apply changes — under the same
capability gating the MCP server already enforces (e.g. write tools blocked on the
`public` network profile). The model proposes; the tools verify and execute.

> Note: pb-mcp-server is a **separate companion repository**, versioned and developed
> independently of the pblang core. pbAI integrates with it as a client; it does not
> require modifying that server.

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
suite, and golden tests. This has started in pblang with `llm_core.sapp`,
`llm_tokenizer.sapp`, `llm_data.sapp`, `llm_transformer.sapp`, and the scale-up modules
listed above, including the new `llm_binary.sapp` bit-level token substrate,
`llm_offload.sapp` low-memory inference planning, and live-mutable architecture hooks.
pblang keeps the reusable foundation and Nano path; the
pbAI-Max product module, training policy, corpora, and tier metadata live in
`AAP/ai/pbai`. Existing checkpoints are used only to compare quality and speed.

## 9. Roadmap (planned)

| Phase | Goal |
|-------|------|
| **P0 — Fresh inference contract** | Started: `llm_core.sapp`, `llm_tokenizer.sapp`, `llm_data.sapp`, `llm_transformer.sapp`, `llm_attention.sapp`, `llm_train.sapp`, `llm_stream.sapp`, `llm_weights.sapp`, `llm_tokenizer_train.sapp`, `llm_eval.sapp`, `llm_quant.sapp`, `llm_binary.sapp`, `llm_offload.sapp`, and semantic tests define tokenizer/data/decoder/sampling/checkpoint/training/eval/binary-token/live-architecture/feedback/guessing-loop/low-memory-offload primitives. No old checkpoint defines this contract. |
| **P1 — pbAI-Nano/-Mini** | Train the first fresh tiny + small models for pblang completion/debugging on pblang/code data plus selected external corpora; benchmark accuracy, latency, memory, and tokens/sec; lock with drift tests. |
| **P2 — Agent loop + tools** | Wire the plan→act→verify loop to `frontend_run`, the lint/typeck oracles, and the pb-mcp-server tool catalog. |
| **P3 — pbAI-Max + distillation** | Train/assemble the new flagship on the full curated corpus; stand up the distillation pipeline that produces consistent Pro/Mini/Nano sub-models from pbAI-Max. |
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
The full Rust MCP protocol daemon still owns the larger 484+ tool catalog, gateway,
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
