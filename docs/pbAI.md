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
- **`stdlib/pbai.sapp`** — pbAI's IDE **assistant**: a neural intent classifier (on `nn.sapp`)
  plus pblang-grounded skills (explain / find-bugs / improve / comment / knowledge),
  **no external model, no network**. It **backs the IDE 🤖 AI panel** via a pblang gateway
  (`editors/ide/tools/ai/gateway.sapp`). Locked by `tests/semantics/pbai_test.sapp`.
- **`stdlib/pbai_lm.sapp`** — pbAI's first **generative** model: a character-level neural
  **language model** (embedding → tanh hidden → vocab logits → softmax), trained on
  next-char prediction with **manual backprop** (the `tensor_*` kernels are forward-only),
  generating autoregressively with the `tensor_argmax` kernel. It learns (cross-entropy
  3.29 → 0.58 on a small pblang corpus) and generates coherent pblang
  (`fn ` → `fn main() { let r; }`). Locked by `tests/semantics/pbai_lm_test.sapp`.

These are the small end of the family — analysis **and** generation, all in pblang. The
transformer scale-up (attention/more layers over the tensor kernels above, larger corpora,
temperature sampling) is the roadmap below.

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

## 8. Existing assets to build on (reference)

A local sandbox of prior AI work — **`../AAP/ai/`** (outside this repo) — already
contains a working small language model, a tokenizer, a from-scratch inference engine,
a training pipeline, and datasets. pbAI does **not** start from zero; it should reuse
these. (Paths below are on the dev machine, not committed to the pblang repo.)

| Asset | Path | How it maps onto pbAI |
|-------|------|------------------------|
| **Trained GPT-2 (4L/4H, d=256, ffn=1024, ctx=128, vocab=16384, ~7–8M params, 29 MB fp32)** | `AAP/ai/max/max_model/model.safetensors` (+ `config.json`) | The first **teacher / flagship checkpoint** for distillation, and the proven **sub-model architecture blueprint**. Its forward pass is exactly: `tensor_embedding_lookup` → N×(attention = `tensor_matmul`+`tensor_softmax`+`tensor_layer_norm`) + FFN(`tensor_matmul`+`tensor_gelu`) → `tensor_matmul` → `tensor_softmax`/`tensor_argmax`. Every kernel already exists as a pblang builtin. |
| **BPE tokenizer (16384 vocab, ByteLevel, `[UNK]/[PAD]/[BOS]/[EOS]`)** | `AAP/ai/max/max_model/tokenizer.json` | Parse the merge table + vocab → port to a pure-pblang BPE encoder/decoder. This is pbAI's tokenizer. |
| **Pure-from-scratch inference engine (zero deps)** | `AAP/ai/max/NeuralNetwork.pas` and the pblang-era `AAP/ai/Sp_AIModel/SpAI/NeuralNetwork.pas` | **Highest direct-port value.** Object-Pascal with explicit embedding-lookup → forward-loop → softmax skeleton and no framework. Port the skeleton to pblang tensor builtins, swapping the LSTM cell for multi-head attention + layer-norm. `TChatbotModel.Predict` is the generation-loop template. (`Sp_AIModel` = "SpAI", from pblang's former name *pblang* — the original proof-of-concept.) |
| **Binary checkpoint I/O pattern** | `AAP/ai/max/TextProcessor.pas` (`SaveToFile`/`LoadFromFile`), `AAP/ai/Sp_AIModel/SpAI/Win32/Debug/tokenizer.dat` | Template for pbAI's **native weight/tokenizer file format** that pblang can load (and eventually mmap) without a Python dependency. |
| **Multi-provider agent / tool-call loop** | `AAP/ai/max/AIChatService.pas` (Gemini + OpenAI, retry/backoff, JSON I/O) | Pattern for pbAI's **provider/tool dispatcher** — maps onto `ai_chat`/`provider_registry.sapp` for remote fallback and onto the pb-mcp-server tool dispatch in [§7](#7-pb-mcp-server-tools). |
| **Training + tokenizer pipeline** | `AAP/ai/max/trainer_max.py`, `train_tokenizer.py`, `prep_books.py`, `tokenize_data.py` | The pipeline that produced the checkpoint above; reuse to train/scale the flagship and to **distill** sub-models. Add a code corpus for the coding/debugging specialization. |
| **Datasets (on disk)** | `AAP/datasets/data/` — TinyStories (~1.9 GB), BookCorpus (~4.8 GB), PG19, GloVe 6B | Pretraining corpora. For pbAI's coding/debugging focus, augment with a pblang/code corpus (the pblang repo itself: stdlib, tests, compiler). |

**Practical first step (P0):** load `max_model/model.safetensors` + `config.json`, convert
the weights to a flat pblang-loadable binary, and reimplement its GPT-2 forward pass in
pblang on the existing `tensor_*` builtins — using `NeuralNetwork.pas` as the structural
skeleton and `tokenizer.json` for encode/decode. That yields a real, runnable pbAI-Nano
inference path before any new training is needed.

## 9. Roadmap (planned)

| Phase | Goal |
|-------|------|
| **P0 — Inference runtime** | A pblang module that runs a small decoder-only transformer purely on the existing tensor builtins (embedding → attention → sampling); load the existing `max_model` GPT-2 weights ([§8](#8-existing-assets-to-build-on-reference)) from a converted flat file; deterministic, testable forward pass. |
| **P1 — pbAI-Nano/-Mini** | First tiny + small models with weights, focused on pblang completion/debugging; benchmark accuracy + tokens/sec; lock with drift tests. |
| **P2 — Agent loop + tools** | Wire the plan→act→verify loop to `frontend_run`, the lint/typeck oracles, and the pb-mcp-server tool catalog. |
| **P3 — pbAI-Max + distillation** | Train/assemble the flagship; stand up the distillation pipeline that produces the sub-models from it. |
| **P4 — Self-improvement loop** | pbAI lands reviewed patches to pblang's own compiler/stdlib/docs; its successes feed the next pbAI-Max. |
| **P5 — Expandability surface** | Stable plugin/adapter/backend/corpus registration APIs so the family grows without core forks. |

Each phase is gated the pblang way: a feature is "done" only when locked by tests
(`tests/*.rs` / `tests/semantics/*.sapp` / a new `tests/pbai_*` suite) and verified
green, exactly as the self-hosting milestones (PrjAnalysis §§40–45) were.

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
