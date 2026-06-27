# pbAI-Max Strategy - Priority #1

This document is the current strategy for turning pbAI-Max into the strongest
pblang-native AI system we can build. It is intentionally ambitious, but it must
stay honest: AGI and ASI are research goals, not claims about the current model.

The strategy is to build pbAI-Max as an intelligence system, not only as one
checkpoint. The model matters, but the full system is:

```text
data -> train -> evaluate -> use tools -> solve tasks -> verify -> remember -> improve model/runtime -> repeat
```

That loop is Priority #1.

## 1. Priority #1: The Self-Improving Intelligence Loop

pbAI-Max must become a pblang-native system that can improve through measured
feedback:

1. Learn from curated data, live lessons, files, folders, tools, and verified
   task traces.
2. Train a candidate model or adapter.
3. Evaluate it with validation loss, regression prompts, tool tasks, pblang
   coding tests, memory tests, and safety checks.
4. Accept only candidates that improve the combined score.
5. Reject or archive candidates that overfit, hallucinate, regress, or become
   slower without quality gain.
6. Store the lesson, result, failure reason, and checkpoint metadata.
7. Use the result to improve pbAI-Max, pblang, the runtime, and the next
   training run.

This is the practical route toward increasingly general intelligence. Do not
chase AGI/ASI directly. Build the machine that can safely improve itself.

## 2. System Shape

pbAI-Max should be a pblang-native AI operating system:

| Layer | Purpose |
|---|---|
| Core model | Transformer-style decoder, tokenizer, embeddings, attention, generation, checkpointing. |
| Memory | Live lessons, episodic events, long-term corpus, retrieval index, cached derived data. |
| Retrieval | Bring relevant knowledge into context instead of forcing the model to memorize everything. |
| Tools | Run, test, inspect, search, edit, benchmark, summarize, verify, and act through pblang and MCP tools. |
| Agents | Planner, reviewer, retriever, tool runner, verifier, learner, reflector, and domain specialists. |
| Verifier | Reject wrong outputs, bad checkpoints, unsafe actions, malformed code, and overfit candidates. |
| Runtime | Parallel execution, cache, streaming data, quantization, hardware-aware scheduling, checkpoint save/load. |
| Evaluation | The boss: no model or feature is accepted without measured improvement. |

The best AI system is not only bigger parameters. It is a model plus memory,
tools, verification, feedback, and reliable training infrastructure.

## 3. Use pblang as the Full AI Substrate

pbAI-Max should use every useful pblang feature instead of duplicating it:

- `llm_attention`: attention, safe generation, architecture growth, local/global
  attention planning.
- `llm_eval`: loss, perplexity, sequence evaluation, generation quality, text
  shape, candidate selection.
- `llm_data`: dataset records, chunking, filtering, sanitizer, curriculum
  interleaving.
- `llm_quant`: quantization, compressed vectors, approximate vector search.
- `llm_train`: training loops, feedback loops, guessing loops, reward/punish
  records, elite selection.
- `spawn` / channels / parallel test features: parallel candidate training,
  parallel hidden-cache computation, parallel ingestion, and parallel evals.
- `machine_opt`: resource-aware plans for CPU, RAM, cache, old machines, GPU
  routes, WASM, and future devices.
- `pbai_cache`: persistent cache for training summaries, retrieval, generation,
  tokenization, and eval metadata.
- `pb-mcp-server`: broad external tool surface for code, files, shell, git,
  media, data, security, search, agents, and project automation.

Every time pblang gets stronger, pbAI-Max should get stronger by consuming the
new primitive through a stable contract.

## 4. Data Strategy

Data is the fuel. pbAI-Max needs a data engine, not only a static corpus.

Required sources:

- pblang docs, stdlib, tests, compiler, examples, and semantic failures.
- pbAI docs, Max checkpoints, live lessons, prompt regressions, and failed
  generations.
- General code corpora, math, science, engineering, logs, configs, JSON, CSV,
  Markdown, and structured data.
- Tool traces: task, plan, command, output, verification, final fix.
- Human feedback: corrected answers, rejected outputs, lessons, preference
  examples.
- Local private data only through explicit ingestion, redaction, and capability
  gates.

Rules:

- Filter secrets before learning.
- Separate train/validation/test.
- Keep provenance for every record.
- Prefer verified examples over raw text.
- Use curriculum examples for core pblang syntax and safe assistant behavior.
- Use retrieval for broad knowledge before trying to memorize everything.

## 5. Model Strategy

The model should grow in stages:

1. Better tokenizer: BPE/unigram training, domain vocab, code-aware tokens,
   binary/token features.
2. Full transformer training: analytic backprop through embeddings, layernorm,
   residuals, FFN, Q/K/V/O attention, and LM head.
3. Efficient attention: multi-head, grouped-query, local/global windows, KV
   cache, prefix/radix reuse.
4. Adapter training: LoRA/QLoRA-style low-memory adapters for live learning and
   domain specialization.
5. Retrieval-augmented generation: vector search, memory search, code search,
   tool traces, and cached retrieved context.
6. Mixture of experts: domain experts for pblang, Rust, math, systems, security,
   data, web, agents, and multimodal work.
7. Multimodal adapters: image, audio, video, sensors, documents, tables, binary
   artifacts, and device data.
8. Distillation: Max -> Pro -> Mini -> Nano, with each tier benchmarked for its
   use case.

Do not increase parameters blindly. Increase capability only when data,
training speed, validation, and inference cost can support it.

## 6. Evaluation Strategy

Evaluation decides what is real.

Required eval groups:

- Language-model metrics: train loss, validation loss, perplexity, token
  accuracy.
- pblang coding: syntax, repairs, compiler errors, semantic tests, stdlib tasks.
- General coding: read files, modify project, run tests, explain failures.
- Retrieval: find relevant facts, cite source records, avoid unrelated memory.
- Tool use: plan, call tools, inspect outputs, verify success.
- Math/science: exact arithmetic, symbolic reasoning, applied problems.
- Safety: secret leakage, path safety, command safety, capability gates.
- Long tasks: multi-step project improvement with checkpointed state.
- Regression prompts: fixed prompt suite that must not get worse.
- Speed/resource metrics: wall time, CPU use, RAM, cache hits, tokens/sec.

Acceptance rule:

```text
accept candidate only if combined_score improves
and validation does not regress beyond tolerance
and safety/tool/code regressions stay zero
```

## 7. Agent Strategy

pbAI-Max should not be one monolithic chat loop. It should route work through
agents:

- Root pbAI-Max: orchestrates model, memory, tools, verification, and policy.
- Planner: turns goals into steps.
- Retriever: gathers memory, docs, code, and tool context.
- Coder: writes and patches code.
- Reviewer: finds bugs, risks, missing tests, and weak assumptions.
- Tool Runner: executes pblang/MCP actions under capability gates.
- Verifier: runs tests, checks outputs, rejects fake success.
- Learner: turns accepted work into training examples and lessons.
- Reflector: records what worked, what failed, and what to try next.
- Domain experts: math, systems, security, web, data, media, hardware, and
  pblang/compiler specialists.

Agents must share a common trace format:

```text
goal -> plan -> action -> observation -> verification -> result -> lesson
```

## 8. Runtime And Hardware Strategy

pbAI-Max must run everywhere, but use stronger hardware when available:

- Tiny/local: Nano/Mini, retrieval, quantization, small adapters.
- Laptop/desktop CPU: parallel training/eval, cache, chunking, resource-aware
  workers.
- GPU: tensor kernels, batched matmul, attention, future training acceleration.
- NPU/vendor stacks: ONNX/quantized inference only when a real backend exists.
- WASM/browser/mobile: smaller tiers, quantized inference, retrieval client.
- Distributed: shard data/eval/training candidates when pblang has stable
  distributed primitives.

The runtime must choose the best route automatically where possible:

```text
small task -> local model
large context -> retrieval + cache + chunking
many candidates -> parallel workers
memory pressure -> streaming/out-of-core
repeated prompt -> cached ranked generation
heavy tensor work -> optimized tensor backend
```

## 9. Safety And Control

More capable pbAI-Max needs stronger control:

- Capability gates for filesystem, network, shell, FFI, env, and tools.
- Secret redaction before learning.
- Audit logs for tool calls and training sources.
- Human approval for risky actions.
- Revert/rollback for bad checkpoints.
- Strict current-vs-roadmap docs.
- No fake claims about intelligence, AGI, ASI, training scale, or safety.

The goal is not only power. The goal is controllable power.

## 10. Milestones

### Milestone A: Reliable Max

- Full transformer analytic backprop.
- Stronger tokenizer.
- Validation split and prompt regression suite.
- Retrieval-grounded generation.
- Generation cache.
- Strict accept/reject checkpoint policy.

### Milestone B: Useful Max

- Project-level coding tasks.
- pblang compiler/stdlib improvement tasks.
- Tool-use benchmark.
- Memory/retrieval benchmark.
- Local docs/files/folders ingestion with redaction.

### Milestone C: Learning Max

- LoRA/adapter live learning.
- Feedback and preference datasets.
- Accepted-answer distillation.
- Rejected-candidate analysis.
- Small fast trial/error reinforcement loops: run tiny deterministic candidate
  comparisons before larger training, record previous/current state, reward
  improvement, punish regression, and keep checkpoint writes disabled until the
  normal validation/benchmark/teacher-quality gates pass.
- Automatic curriculum generation from failures.
- Ideal / Imaginary Variable NN target: define mathematical target coordinates for
  current pblang NNs using probability calibration, complex-valued state ideas,
  linear algebra, and geometry/manifold distance, then train current student
  networks to reduce measured distance from that target under validation and safety
  gates. The current implementation exposes this as a weighted objective proxy;
  future work should replace proxy components with real calibrated model metrics.

### Milestone D: Scaled Max

- Faster tensor kernels.
- Parallel and batched training.
- Quantized inference.
- MoE/domain experts.
- Long-context KV cache and prefix reuse.
- Distributed candidate/eval execution.

### Milestone E: General Max

- Text, code, math, science, tools, images, audio, video, tables, logs, and
  device data.
- Multi-agent long-horizon tasks.
- Self-improvement of pblang and pbAI under verification.
- Distillation into Pro/Mini/Nano.

## 11. Immediate Next Step

The next implementation priority is:

1. Add pblang-native vector retrieval for pbAI-Max memory. **Started:** Max now
   exposes `memory_index`, `memory_retrieve`, and `memory_status`, backed by
   `llm_quant.turbovec_*`, and ranked generation uses that vector-memory path for
   the retrieval-grounded candidate.
2. Add LoRA-style adapters in pblang. **Started:** pblang now has
   `stdlib:llm_adapters.sapp` for frozen-base low-rank LM-head adapters, and
   pbAI-Max exposes `adapter_status` / `train_adapter` plus API actions for the
   first adapter training path.
3. Train adapters from live lessons instead of touching the full base model for
   every lesson. **Started:** pbAI-Max now exposes `train_live_adapter` and
   `teach_adapter`; these use `stdlib/pbai_max_live_lessons.txt` as the data
   source and report `touches_checkpoint: false`.
4. Add a benchmark suite for pblang coding, math, memory, tools, retrieval, and
   safety. **Started:** pbAI-Max now exposes `benchmark_cases`,
   `benchmark_status`, `benchmark_thresholds`, `benchmark_threshold_report`, and
   `benchmark_suite`; the suite is deterministic, pblang-only, available through
   the `max_benchmark` API action, and now uses multiple cases plus thresholds for
   each category.
5. Accept a checkpoint or adapter only if validation plus benchmark score
   improves. **Started:** continuation selection now folds benchmark pass rate,
   average score, failed cases, and failed categories into
   `pbai.max.selection_score.v1`; checkpoint saves require validation improvement
   without benchmark category regression, or regression plus benchmark threshold
   improvement within validation tolerance. Adapter training now reports an
   explicit loss-plus-benchmark merge gate.
6. Start family distillation from Max into Pro/Mini/Nano. **Started and training:**
   pbAI-Max now exposes `distillation_targets`, `distillation_dataset`,
   `distillation_preview`, `train_distilled_student`, `distill_family`, and the
   `max_distill_preview` / `max_distill_train` / `max_distill_family` /
   `max_distill_save_family` / `max_distill_family_summary` API actions.
   The current implementation emits teacher records, target specs, verifier/rank
   traces when a checkpoint is supplied, trains bounded smaller Pro/Mini/Nano
   students from verified teacher records, and gates save-readiness with validation
   plus benchmark thresholds. It can also persist `family.json`, `index.json`, and
   one saved student checkpoint directory per tier. Larger curated teacher datasets
   are the next scale-up.
7. Move verified generation into Max core. **Started:** strict verification,
   repair, fallback, and `generate_verified_ranked` now live in
   `stdlib/pbai_max.sapp`, not the API wrapper. Regression and distillation use
   verified selected text, and the Max generation cache key includes
   `generation_policy_version: "verified-repair-v1"` so old malformed cached
   generations do not silently reuse the new policy slot.
8. Add small fast trial/error reinforcement tests. **Started:** pbAI-Max now exposes
   `fast_trial_reinforcement_smoke()`, a deterministic tiny loop that compares
   reward, punish, and neutral candidate states against the ideal alignment objective,
   delegates reward/punish comparison and elite selection to pblang's shared
   `llm_eval` helpers, reports previous/current state, selects the best improving
   trial, and explicitly keeps `mutates_checkpoint: false`.
9. Connect the standalone pbAI RL substrate to Max status. **Started:** `stdlib/pbai_rl.sapp`
   provides deterministic seeded trial-and-error and ideal-target training helpers;
   pbAI-Max exposes `rl_trial_learning_status()` so the status contract can show
   reward history and accepted improvements without writing checkpoints. The main
   facade/API exposes this as `pbai.max_rl_status()` and the `max_rl_status`
   API action so scripts can check the fast RL loop directly.

This is the concrete path from the current pbAI-Max toward a serious, broad,
local-first AI platform.
