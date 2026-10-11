# pbAI

pbAI is a **pblang-native** AI model and agent stack. The language/runtime primitives
live in the [pblang](../pblang) repository; this repository owns the actual model work —
tokenizer, language model, cognitive "brain" loop, ingestion, tools, the test console,
and the regression suite.

The goal is a small **pbAI LLM family** (Nano → Mini → Pro → Max) with live learning,
multimodal data routing, memory, tools, retrieval, and verified coding ability. Today the
shipped tier is **pbAI-Nano** plus grounded pblang skills; the larger tiers are targets.

## Number-one strategy

The number-one strategy for pbAI-Max is documented in
[`docs/pbAI_Max_Strategy.md`](docs/pbAI_Max_Strategy.md): build a pblang-native
self-improving intelligence loop, not just a single checkpoint.

```text
data -> train -> evaluate -> use tools -> solve tasks -> verify -> remember -> improve model/runtime -> repeat
```

That document is the priority roadmap for making pbAI-Max smarter, faster, more
accurate, broader across fields, and eventually capable of AGI/ASI research work while
keeping current-vs-roadmap claims honest.

## Project layout

| Path | What it is |
| --- | --- |
| `stdlib/pbai.sapp` | Assistant facade: routing, review skills, live memory, model status, tool API. |
| `stdlib/pbai_lm.sapp` | pblang-native char-level Nano LM: corpus, training, sampling, generation, checkpoints. |
| `stdlib/pbai_tokenizer.sapp` | Versioned pblang-native char tokenizer (`pbai.tokenizer.char.v1`). |
| `stdlib/pbai_bpe.sapp` | pblang-native byte-pair-encoding (BPE) subword tokenizer (`pbai.tokenizer.bpe.v1`), drop-in with the char contract. |
| `stdlib/pbai_accel.sapp` | Honest hardware-acceleration policy: CPU core planning, GPU/NPU detection, intensity tiers. |
| `stdlib/pbai_quant.sapp` | Weight quantization: round Nano LM weights to low-bit `fN` (f8/f16/bf16), measure compression + error (via pblang `stdlib:floatn.sapp`). |
| `stdlib/pbai_hw.sapp` | Thin alias for the pblang machine-inventory library `stdlib:hwinfo.sapp` (CPU, RAM, SSD/HDD, every GPU, NPU, network/sound, all PnP classes; vendor-agnostic; honest compute-usability labels). |
| `stdlib/pbai_brain.sapp` | Cognitive loop: perception, memory, planning, critique, verification, reflection. |
| `stdlib/pbai_ingest.sapp` | Bounded local file/folder ingestion into live learning, with shared pblang `llm_data` secret-line sanitization before persistence. |
| `stdlib/pbai_cache.sapp` | pblang-only persistent cache for derived pbAI data: verified SHA-256 keys, TTL, quotas, recovery-safe publication, training summaries, retrieval, and generation metadata. |
| `stdlib/pbai_streaming.sapp` | High-level facade over pblang's integrity-checked low-memory LLM store: on-demand layer/MoE shards, bounded RAM cache, learned hot experts, bounded look-ahead prefetch, weighted disk mirrors, exact-length/SHA-256 verification, and explicit precision semantics. |
| `stdlib/pbai_max.sapp` | pbAI-Max tier policy/orchestration, checkpointing, continuation, and prompt-level regression evaluation (delegates heavy LLM math to pblang `stdlib:llm_*`). |
| `stdlib/pbai_assets.sapp` | Inventory of external data/reference assets discoverable from pblang. |
| `stdlib/pbai_tools.sapp` | Local capability/source map. |
| `tools/pbai_server.sapp` | pblang-only local HTTP server for the website and live API. |
| `tools/pbai_api.sapp` | One-shot JSON bridge for scripts/tools. |
| `tools/pbai_feed.sapp` | Command-line feed modes (lessons, paths, drives, phone/MTP sources). |
| `demo/pbai_demo.sapp` | JSON fixture generator for the browser console. |
| `website/` | Browser test console (host code: HTML/CSS/JS) with a live + offline mode. |
| `tests/semantics/` | Regression contract for assistant, LM, tokenizer, BPE, brain, sampling, accel, and assets. |
| `checkpoints/` | Saved model checkpoints and weights. |


The one-shot bridge rejects supplied files outside its bounded UTF-8/JSON and
regular-file policy before importing models or dispatching actions. File admission
currently uses the existing Linux descriptor profile; omitted filenames bypass
that backend. See [request-file admission](docs/pbAI_API_Request_Admission.md) for
limits, error envelopes, cleanup limitations and the exact owning-root command.

The `.sapp` modules are **pblang** source. The `website/` host (`index.html`,
`styles.css`, `pbai-web.js`) is plain static front-end code and needs no toolchain to edit.

## Running the live console

Start the pblang server from the repository root (port is the trailing argument):

```sh
pblang run tools/pbai_server.sapp --allow-all -- 8088
```

Then open:

```text
http://127.0.0.1:8088/website/
```

Regenerate the static fixture used by the offline console:

```sh
pblang run demo/pbai_demo.sapp --allow-all > website/data/pbai-demo.json
```

## The web console

`website/` is a single-page console for exercising pbAI. It runs in two modes:

- **Live pblang pbAI** — when served by `tools/pbai_server.sapp`, every action
  (`respond`, `analyze`, `think`, `generate`, `tokenize`) and all live learning hit the
  real pblang model over `/api/pbai/*`.
- **Browser mirror** — for static hosting (or when the server is unreachable) the same UI
  falls back to a deterministic JavaScript mirror plus the JSON fixture, so the console
  stays useful offline.

Host-side features:

- **Resilient requests** — live calls use timeouts (`fetchWithTimeout`) so a hung or
  missing server degrades to the mirror instead of freezing the UI; in-flight requests are
  guarded against double-submission and surface a busy state.
- **Safe rendering** — server-provided status strings are written with `textContent`, never
  injected as HTML.
- **Efficient visuals** — the brain preview and the 3D architecture map render at device
  resolution, pause their animation loops while the tab is hidden, and honor
  `prefers-reduced-motion` by rendering static frames.
- **Accessible 3D map** — the architecture canvas is keyboard-operable (arrow keys rotate,
  `+`/`-` zoom, `R` resets) in addition to mouse/touch.
- **Quality-of-life** — copy the current output to the clipboard, and the prompt/editor is
  persisted across reloads.
- **Live learning** — teach text, files, folders, or a server-side path into the local
  live corpus (`stdlib/pbai_live_corpus.txt`) and watch the training loss update.
  Text-file ingestion redacts common token/password/API-key/private-key lines before
  writing chunks to live memory and reports `stats.redactions` in the result.

## Hardware acceleration (honest picture)

`stdlib/pbai_accel.sapp` is the policy module for "use more of my machine," and it reports
the truth rather than promises:

- **CPU, multi-core — the real lever today.** The Nano LM does its arithmetic directly in
  pblang (not through `tensor_matmul`), so its scaling path is multi-core *best-of-N*
  training: `pbai_lm.train_nano_parallel` trains N models on N OS threads (`spawn`) and
  keeps the best, using ~N cores for roughly the wall-time of one model. `accel.plan(...)`
  maps an intensity (`eco` / `balanced` / `max` / a number) to a worker count, leaving the
  default with headroom so the machine stays responsive.
- **GPU — opt-in, in pblang core.** pblang ships an opt-in wgpu matmul backend behind
  `cargo build --features gpu` + `PBLANG_GPU=1` (f32; the default CPU path is exact f64).
  It accelerates `tensor_matmul`-based code (the `pbai_max` / `llm_*` heavy path), **not**
  the Nano char LM. See the pblang repo's README "Tensor compute and hardware acceleration."
- **NPU — detected, not yet usable for compute.** `accel.probe_npu()` uses pblang's shared
  `stdlib:hwinfo.sapp` inventory layer, so Windows/Linux/macOS now share the same
  best-effort detection path and degrade to explicit notes instead of platform stubs.
  There is no NPU compute backend in pblang: NPUs are driven by vendor stacks
  (ONNX Runtime + Vitis AI / OpenVINO / QNN) on quantized graphs, which don't map to
  pblang's tensor kernels. So a detected NPU is reported honestly but is informational only.

```pblang
// Capability + policy report (pass true to enumerate GPU/NPU; needs --allow-exec):
let accel = import("stdlib/pbai_accel.sapp");
print str(accel.detect(true));     // platform, cpu_cores, recommended workers, gpu, npu, ...
print accel.describe();            // one-line human summary
```

Detection (`probe_gpu` / `probe_npu`) uses pblang `hwinfo` and the exec capability;
without `--allow-exec` it degrades to an honest unavailable/no-device status.

**Full machine inventory** (`stdlib/pbai_hw.sapp`) detects *every* device on the PC —
CPU, RAM, drives (SSD/HDD), every GPU (integrated + discrete), NPU, network and sound
cards, and a class histogram of all other PnP devices/cards on the motherboard —
via `proc_spawn_wait` + CIM (needs `--allow-exec`). `survey()` returns the whole inventory
with an honest label on each section: only the **CPU** (all cores) and **GPU** (opt-in, one
at a time) can be *used for compute*; NPU/RAM/disks/network/sound are detect-only (they are
storage and I/O devices, not processing units).

**Weight quantization** (`stdlib/pbai_quant.sapp`, built on pblang's `stdlib:floatn.sapp`
`fN` engine) shrinks the *stored* precision of model weights and measures the accuracy
cost end-to-end. `quant_report(model, "f8e4m3")` reports compression (≈8× vs f64) and the
mean/max quantization error; `quantize_model(model, fmt)` returns a model whose weights are
rounded to `fN` (stored as f64, so compute is unchanged). Compute stays f64 — this models
real load-low-bit / compute-high-precision inference, honestly.

## Tests

The semantic regression suite lives in `tests/semantics/` (entry:
`tests/semantics/pbai_test.sapp`). A pbAI change is real only when it improves benchmarked
behavior, passes these deterministic tests, and stays inside the pblang capability model
(`--sandbox`, `--allow-exec`, `--allow-ffi`, and fs/http/env gates).

pbAI-Max also exposes a model-level prompt regression gate:

```sh
pblang run tools/pbai_api.sapp --allow-all -- max_regression /tmp/pbai-max-regression.json
```

where the JSON request can include `{"path":"checkpoints/pbai-max","n":16}`. The report
checks deterministic pblang/code prompts for expected tokens, forbidden/collapse signals,
pass rate, and average score, complementing aggregate loss/perplexity. Max generation uses
pblang's shared `llm_attention.generate_safe(...)` path to cap repeated-token collapse,
then reranks several neural safe-decoding candidates plus a retrieval-grounded candidate
from the curated/curriculum corpus. That retrieval candidate now comes through
`pbai_max.memory_retrieve(...)`, a pblang-native Turbovec vector-memory search over
quality-filtered Max records and curriculum examples, and the retrieval metadata is
cached under `target/pbai-cache/max-memory-retrieval`. The selected Max candidate records
`llm_eval.sequence_evaluate(...)` loss/perplexity/accuracy, `llm_eval.text_shape(...)`
output-shape metrics, and a Max verifier score for expected/forbidden tokens,
brace/paren sanity, and repetition. The final rank can prefer candidates that are
grounded, non-collapsed, more likely under the checkpoint, and structurally cleaner as
pblang/code text. Use `max_generate_ranked` when you need the candidate list and
quality/loss/shape/verifier scores instead of only the selected text. Use
`max_generate_verified` when code-like output must pass the Max strict verifier:
raw malformed candidates are rejected, repair candidates are tried, and a trusted
fallback snippet is selected only as a temporary bridge when the self-host parser
still rejects otherwise safe fallback text.

pbAI-Max also exposes a broader deterministic benchmark gate through
`max_benchmark`. The suite currently checks pblang coding, arithmetic, memory
recall, vector retrieval, local tool visibility, and secret-redaction safety. It
now keeps multiple cases per category and category-specific thresholds, so one
weak area cannot hide behind a good global average. It is cheap enough to run
before accepting a checkpoint or adapter, and it complements the heavier prompt
regression gate.

pbAI-Max now also exposes the first real distillation contract for the model
family. `max_distill_preview` returns Pro/Mini/Nano target specs plus
`pbai.max.distillation_record.v1` teacher records. Without a checkpoint it uses
curated multi-sentence teacher answers for cheap inspection and stronger student
data; with a checkpoint path it uses Max ranked generation, strict verified
generation, and records whether the teacher text came from an original, repair, or
fallback candidate. `max_distill_train` trains a bounded pblang-native
Pro/Mini/Nano student checkpoint from verified teacher records with a smaller
tier-specific decoder, parallel cached-hidden LM-head training, binary-adapter
updates, deep input updates, validation metrics, benchmark thresholds, and an
`accepted_for_save` gate. `max_distill_family` runs the same path for Pro, Mini,
and Nano. This is still a small semantic-test-scale distillation path, not a
finished large-family training pipeline. `max_distill_save_family` persists the
family under a directory layout with `family.json`, `index.json`, and one
`student.json` plus `manifest.json` per tier; `max_distill_family_summary` reads
the saved index.
Saved students are now active enough for bounded use: `max_distill_student_eval`
and `max_distill_family_eval` evaluate saved students, while
`max_distill_student_generate` and `max_distill_student_generate_ranked` generate
from a selected saved tier. `max_distill_route`, `max_distill_route_generate`,
and `max_distill_route_eval` add a first policy router that chooses Nano for very
short/simple prompts, Mini for general pblang/memory prompts, and Pro for code,
tool, or long-context prompts.

Continuation training also uses that prompt quality signal: `max_continue` ranks
candidates by validation loss plus regression pass rate/score plus benchmark pass
rate/score plus benchmark category thresholds, and it keeps the current checkpoint
when a candidate only improves training loss while validation, prompt quality, or
benchmark category quality gets worse. To speed this up, Max continuation now uses
a two-stage pblang-only training loop: all learning-rate/epoch candidates first run
a head-only validation screen with `llm_eval.regression_proxy(...)`, then only the
best finalists run the full binary/feedback/deep/regression/benchmark path before
any checkpoint is saved.
Max training also reserves part of the bounded example budget for clean pblang
curriculum snippets, interleaved through the broader corpus with pblang
`llm_data.repeat_examples(...)` and `llm_data.interleave_priority_examples(...)`.
Max-v2 also has `max_teach`: it appends a lesson to
`stdlib/pbai_max_live_lessons.txt`, then runs validation-gated continuation. The
lesson remains in the local corpus even when candidate weights are rejected.
LM-head training now caches transformer hidden states with pblang workers
(`parallel-cached-hidden-lm-head-backprop`) and then applies deterministic head
updates over that cache.

pbAI-Max also consumes pblang's `stdlib:llm_adapters.sapp` low-rank adapter
foundation. `max_adapter_status` reports a LoRA-style frozen-base LM-head
adapter, and `max_train_adapter` trains that adapter over Max examples without
merging it into the base checkpoint by default. This is the first adapter
substrate for live/domain learning; full Q/K/V/O, FFN, embedding adapters, and
QLoRA-style quantized adapters remain later milestones.
Live lessons now use the adapter route first: `max_train_live_adapter` trains
from `stdlib/pbai_max_live_lessons.txt`, and `max_teach_adapter` appends a
lesson then trains only the low-rank adapter. Both report
`touches_checkpoint: false`, so a quick lesson does not rewrite the base Max
checkpoint. Adapter training also reports `acceptance_gate`, which requires loss
not to worsen and the deterministic benchmark to pass before an adapter should be
treated as merge/save-ready.

Derived data is cached under `target/pbai-cache` through pure pblang helpers.
Cache entries are safe to clear because checkpoints and live lessons are stored
separately. Max also caches ranked generation results under the `max-generation`
cache kind using the checkpoint signature, prompt, token count, and scoring settings,
so repeated prompts can skip reranking when the active checkpoint has not changed. API
actions:

```sh
pblang run tools/pbai_max_pipeline.sapp --allow-all
pblang run tools/pbai_api.sapp --allow-all -- max_memory /tmp/pbai-max-memory.json
pblang run tools/pbai_api.sapp --allow-all -- max_memory_status /tmp/pbai-max-memory.json
pblang run tools/pbai_api.sapp --allow-all -- max_adapter_status
pblang run tools/pbai_api.sapp --allow-all -- max_train_adapter /tmp/pbai-max-adapter.json
pblang run tools/pbai_api.sapp --allow-all -- max_train_live_adapter /tmp/pbai-max-adapter.json
pblang run tools/pbai_api.sapp --allow-all -- max_teach_adapter /tmp/pbai-max-teach-adapter.json
pblang run tools/pbai_api.sapp --allow-all -- max_benchmark
pblang run tools/pbai_api.sapp --allow-all -- max_generate_verified
pblang run tools/pbai_api.sapp --allow-all -- max_distill_preview
pblang run tools/pbai_api.sapp --allow-all -- max_distill_train
pblang run tools/pbai_api.sapp --allow-all -- max_distill_family
pblang run tools/pbai_api.sapp --allow-all -- max_distill_save_family /tmp/pbai-max-distill-save.json
pblang run tools/pbai_api.sapp --allow-all -- max_distill_family_summary /tmp/pbai-max-distill-summary.json
pblang run tools/pbai_api.sapp --allow-all -- max_distill_student_eval /tmp/pbai-max-student-eval.json
pblang run tools/pbai_api.sapp --allow-all -- max_distill_family_eval /tmp/pbai-max-family-eval.json
pblang run tools/pbai_api.sapp --allow-all -- max_distill_student_generate /tmp/pbai-max-student-generate.json
pblang run tools/pbai_api.sapp --allow-all -- max_distill_route /tmp/pbai-max-route.json
pblang run tools/pbai_api.sapp --allow-all -- max_distill_route_generate /tmp/pbai-max-route-generate.json
pblang run tools/pbai_api.sapp --allow-all -- cache_status
pblang run tools/pbai_api.sapp --allow-all -- cache_clear /tmp/pbai-cache-clear.json
```

For daily Max work, prefer the single pipeline command above. It runs the bounded
distillation workflow in-process, saves the Pro/Mini/Nano family, evaluates the
saved students, routes a prompt through the selected tier, and returns one
`pbai.max.pipeline.v1` JSON report with `checks.ok`. The pipeline now also checks
that checkpoint-free teacher records are curated and rich enough, so the workflow
fails if distillation regresses to one-token labels. Pass a JSON request file to
change `mode`, `output_dir`, `teacher_path`, `epochs`, `lr`, `max_records`, `n`,
`max_examples`, `prompt`, or `report_path`. Use `"mode":"full"` or
`"train_max":true` only when you want the script to train/save a fresh Max teacher
checkpoint first.

## Boundary with pblang

Keep model artifacts out of the pblang core repo. New tokenizers, checkpoints, training
corpora, architecture code, and brain modules belong here in `pbai`. The pblang repo keeps
only the tensor/NN builtins, stdlib support, import/runtime integration, and IDE hooks that
call pbAI through a stable boundary. See [`docs/pbAI_externalization.md`](docs/pbAI_externalization.md).

## Embedded bounded encoder/decoder training

The embedded subtree includes HF-13A/B/C reconstruction gradients, detached SGD
and bounded full-batch epochs. From the PBLang root import
`stdlib:ai/pbai/stdlib/pbai_feedback_train.sapp`; adoption is explicit and no
installed runtime or facade action changes. See [contracts and adoption plan](docs/Embedded_Training_Adoption.md).
