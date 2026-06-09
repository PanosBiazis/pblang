# pbAI externalization handoff

Status: active handoff, 2026-06-07.

pbAI development is moving out of the pblang core repository and into:

```text
/media/panosboy/ExtHD/panag/Documents/projects/AAP/ai/pbai
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
cd /media/panosboy/ExtHD/panag/Documents/projects/AAP/ai/pbai
patch -p0 < /tmp/pbai-target-sync.patch
```

After applying it, continue implementation in the external folder and treat the
pblang copy as transitional compatibility only.
