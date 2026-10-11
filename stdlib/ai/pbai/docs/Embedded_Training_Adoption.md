# Embedded bounded reconstruction training

This subtree adopts HF-13A/B/C from standalone pbAI commit
`cf5f1998918c221b1a88e67070e5a1ea3d17cf97`. The production module is byte-identical,
SHA-256 `01471c50018cdfcc7bc03f5989c92c6083ccc6752cc2231c2aa1efecc0ef1891`.
Its only exports are `gradients`, `step`, and `train_epochs`. It imports no facade,
model provider, filesystem or network module. Existing inference is unchanged.

## Owning paths and invocation

Run from the PBLang repository root, not the nested snapshot directory:

```sapp
let T = import("stdlib:ai/pbai/stdlib/pbai_feedback_train.sapp");
let F = import("stdlib:ai/pbai/stdlib/pbai_feedback.sapp");
let layer = map{n_in:1,n_out:1,w:[[0.2]],b:[0.1]};
let encoder = F.stack([layer]);
let decoder = F.stack([layer]);
let candidate = T.train_epochs(encoder, decoder, [[0.2]], [[0.3]], 0.01, 3);
print candidate.final_eval_loss;
// The caller may inspect candidate before separately adopting its models.
```

For a dependency resolver pointing `stdlib` at this checkout:

```sh
pblang run stdlib/ai/pbai/tests/semantics/pbai_feedback_gradients_test.sapp --deny-fs --deny-net --deny-env --deny-http --deny-sqlite --deny-spawn --deny-gfx --deny-exec --deny-ffi
pblang run stdlib/ai/pbai/tests/semantics/pbai_feedback_step_test.sapp --deny-fs --deny-net --deny-env --deny-http --deny-sqlite --deny-spawn --deny-gfx --deny-exec --deny-ffi
pblang run stdlib/ai/pbai/tests/semantics/pbai_feedback_epochs_test.sapp --deny-fs --deny-net --deny-env --deny-http --deny-sqlite --deny-spawn --deny-gfx --deny-exec --deny-ffi
```

The negative-case module exposes fixtures for an external runner; merely loading
it does not execute its 34 cases. Each `run_case(name)` must terminate with exit 1
and the exact payload `pbai.feedback.gradients: invalid or unsupported request`.
Reject additional stdout or a different first diagnostic payload. Runtime source
excerpts are separate from the fixed API payload and may contain caller content.

The copied semantic suites differ from standalone only in owning imports. They
cover independent scalar/finite-difference references, bounded work, deep detached
outputs, original preservation and no partial result on late failures. Passing
source-visible tests alone does not establish use of this embedded path: qualify
an isolated staged subtree with the original source roots unavailable as well.

## Explicit adoption plan

1. Pin the selected PBLang source tree, the production-module hash above and the
   runtime executable. The qualification runtime is retained native SHA-256
   `72d534a27cd500362db59b7b81e77b3270652a2f9562b5bb4a1a62f723bf9ed8`.
2. Build/resolve a disposable PBLang project and run the three training suites,
   inference regression and exact negative diagnostics under explicit capability
   denials, bounded process resources and network isolation. Confirm module origin
   with a staged-only import/behavior control, and compare full deterministic
   outputs against standalone on the same runtime.
3. Review numerical results, errors and ownership checks before source integration.
   The module only returns candidates: set a task-specific evaluation criterion
   and independently decide whether to adopt both returned models together.
4. A later bundled/installed adoption must stage the actual consumer package,
   preserve its old version, verify package/runtime/dependency hashes, repeat
   source-hidden consumer checks and demonstrate rollback before activation.
   This source change does not install or activate that package.

Production code, tests and documentation change only in the embedded
`stdlib/ai/pbai` subtree; the root `pblang.lock` records the resulting stdlib
content digest with its existing repository-relative dependency path.
The standalone active release,
canonical root stdlib, neural runtime and facade dispatch are unchanged. These
library calls do not add CLI/MCP actions, checkpoints or training on conversations.
Linux CPU execution on the pinned runtime is the qualification target; other
platforms, accelerators, latency/RSS/energy improvements, generalization and
convergence require separate evidence. Traversal limits are accounting proxies,
not exact wall-time or physical-memory guarantees. See the three API contracts
for shapes, bounds and truthful loss reporting, including accepted worsening.
