# pbAI externalization handoff

Status: active handoff, 2026-06-07.

pbAI development is moving out of the pblang core repository and into:

```text
<AAP>/ai/pbai
```

The pblang repository should keep only the language/runtime primitives that pbAI needs:

- tensor and neural-network builtins
- stdlib support modules used by ordinary pblang programs
- import/package/runtime integration points
- IDE hooks that call pbAI through a stable boundary
- compatibility shims required by existing tests during the transition

New pbAI model work should happen in the external `ai/pbai` folder. In particular,
do not grow the pblang repo with new pbAI checkpoints, tokenizer experiments,
training corpora, model architecture code, or agent-brain implementation modules.

## Current External Implementation

The external folder now owns the active pbAI implementation slice:

- `stdlib/pbai_tokenizer.sapp` — versioned pblang-native char tokenizer.
- `stdlib/pbai_lm.sapp` — pbAI-Nano char language model and checkpoint API.
- `stdlib/pbai_brain.sapp` — measurable cognitive loop with perception, memory,
  planning, critique, and verification parsing.
- `stdlib/pbai.sapp` — assistant/router facade exposing `respond`, `analyze`,
  `generate`, `model_status`, and `think`.
- `demo/pbai_demo.sapp` — JSON fixture generator for browser testing.
- `tools/pbai_server.sapp` — pblang-only local HTTP server for the website and live
  pbAI API endpoints.
- `tools/pbai_api.sapp` — one-shot pblang JSON bridge for tests/tools.
- `website/` — pbAI test console; live when served by `tools/pbai_server.sapp`, with
  browser-mirror fallback for static hosting.

Run the live local server from the external root:

```sh
pblang run tools/pbai_server.sapp --allow-all -- 8088
```

Open:

```text
http://127.0.0.1:8088/website/
```

Regenerate the website fixture from the external root:

```sh
../../pl-dev/pblang/target/debug/pblang run demo/pbai_demo.sapp --allow-all > website/data/pbai-demo.json
```

## Brain Direction

The product goal is a human-inspired cognitive architecture that is better than a
single chat model at engineering work. "Exactly like a human brain" is not an
implementation contract; a literal biological brain clone is neither testable nor
buildable in this codebase. The implementable contract is:

- multimodal working memory, long-term memory, and episodic project memory
- perception over code, docs, tests, terminal output, diffs, and tool results
- planning, reflection, critique, and verification loops
- tool-using motor actions through pb-mcp-server and pblang runtime APIs
- fast local sub-models with escalation to larger pbAI tiers when confidence is low
- continuous evaluation against coding, debugging, review, and self-improvement tasks
- safety boundaries from pblang capabilities, sandbox limits, and explicit tool gates

This keeps the ambition high while making progress measurable. A pbAI brain change is
real only when it improves benchmarked behavior, passes deterministic regression tests,
and stays inside the capability model.

## Current Sync Patch

Because this session can write only inside the pblang workspace and `/tmp`, the
current external-folder update is prepared as:

```text
/tmp/pbai-target-sync.patch
```

Apply that patch from the external pbAI root to bring it up to the current in-tree
pbAI files:

```sh
cd <AAP>/ai/pbai
patch -p0 < /tmp/pbai-target-sync.patch
```

After applying it, continue implementation in the external folder and treat the
pblang copy as transitional compatibility only.
