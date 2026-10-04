# Explicit specialist binding

`neural_specialist_bind(provider)` is the public facade's supported computation
route for callers whose working directory differs from the trusted application
installation. Both actual facades (`pblang/stdlib/pbai.sapp` and standalone
`pbai/stdlib/pbai.sapp`) delegate to `stdlib:pbai_specialist_binding.sapp`.

## Trusted setup

```pblang
let AI=import("stdlib/pbai.sapp");
let NEURAL=import("stdlib:pbai_neural_adapter.sapp");
let specialist=AI.neural_specialist_bind(NEURAL);
let reviewed=specialist.student_continue_reviewed(checkpoint,history,training,validation);
// Inspect reviewed.report() before explicit local accept/rollback/close.
```

Keep the imports at application top level, using the intended application manifest
and a materialized dependency lock. The top-level `stdlib:` import retains the
application source context on the tested runtime. The binder holds the selected
exported function values and performs no further imports, cwd changes, path search,
provider calls, training, persistence or network operations. Importing the facade
loads the small binder helper, not the optional neural provider. The selected
neural module imports its serializer once at module top level, so checkpoint and
history decoding also retain their dependency origin. Parsing limits are unchanged.

The provider must be a **native module handle selected by trusted application
code**. A request map, JSON value, path, callback, cyclic value or a module missing
required callable exports throws before a binding is returned. No partial binding
or fallback to a caller-directory provider is returned. An unavailable or broken
selected module fails its top-level import; do not replace that failure with a
user-selected path or a legacy lazy-wrapper retry.

Native module type and callable fields do not authenticate the publisher, validate
function arity/semantics, sandbox malicious module code, or establish model quality.
Module initialization already ran before binding. Select trusted packaged code;
never expose this setup as an arbitrary remote/MCP provider-selection operation.
The binding does not call providers to inspect contracts or run numeric probes.

## Bound surface

The returned map has the existing provider's 14 computational callables and a
fresh `report()` function. Signatures, optional arguments, validation and results
are unchanged from the shared neural implementation:

| Bound method | Corresponding legacy facade method |
| --- | --- |
| `train` | `neural_specialist_train` |
| `predict` | `neural_specialist_run` |
| `session` | `neural_specialist_session` |
| `distill` | `neural_specialist_distill` |
| `distill_supervised` | `neural_specialist_distill_supervised` |
| `student_encode`, `student_decode`, `student_load` | `neural_specialist_student_encode`, `neural_specialist_student_decode`, `neural_specialist_student_load` |
| `student_continue` | `neural_specialist_student_continue` |
| `student_history`, `student_history_encode`, `student_history_decode` | The corresponding `neural_specialist_student_history*` methods |
| `student_continue_reviewed` | `neural_specialist_student_continue_reviewed` |
| `student_tool` | `neural_specialist_student_tool` |

The report declares structural export checking, not provider authentication or
numeric validation. It includes only fixed method names and scope flags; no module
path, caller data or weights. Editing a returned report does not change later
reports. Returned callable maps remain ordinary trusted application objects, not
an authorization boundary against code that can replace its own functions.

Binding performs no training. Calling bound `train`, distillation or continuation
explicitly performs their existing bounded local computation. A reviewed handle
retains its pass/fail gates, one-shot local acceptance/rollback and terminal
`close()`. Earlier exports and other handles keep their own lifetimes. See
[student review](pbAI_Student_Review.md). Binding is not global model activation.

Filesystem checkpoints have a separate opt-in
[checkpoint binding](pbAI_Checkpoint_Binding.md). They remain outside this
computational binding, as do curriculum/sensor wrappers and unrelated lazy imports. Existing convenience wrappers
remain compatible when called from the trusted product directory. Their general
[retained function-origin limitation](pbAI_Modularity.md#installed-execution-and-the-retained-function-origin-limit)
remains; this is not a universal loader repair.

## Reproduction

- `tests/semantics/pbai_specialist_binding_test.sapp`: direct binder/public facade
  rejection, detached reports and actual 14-operation numerical/lifecycle parity.
- `tests/semantics/pbai_student_review_close_test.sapp`: direct module, legacy
  facade and bound facade across all four pre-close states (12 cases/product).
- `examples/pbai_student_review_close.sapp` (PBLang) and
  `demo/pbai_student_review_close.sapp` (standalone): top-level bound setup and
  reviewed acceptance, explicit export, close, refusal and independent reload.
- `tests/semantics/pbai_student_review_close_installed_test.sapp`: Linux copied
  library/example acceptance with both original trees hidden, using `PBLANG_BIN`.
  Run separately with `PBAI_BINDING_TEST_PHASE=installed` (default),
  `PBAI_BINDING_TEST_PHASE=unavailable`, and `PBAI_BINDING_TEST_PHASE=surface`. The first phase checks trusted product
  cwd, foreign cwd without a manifest, a valid caller-owned manifest/lock with
  neural/binder/serializer/JSON/CSV decoys, and the closed error guard. It also retains the legacy
  wrapper failure as a clearly separate known limit. The second phase removes
  or breaks the selected provider, confirms the facade still imports, rejects
  binding without a provider, and refuses to substitute the caller decoy. The
  `surface` phase runs the full 14-operation/codec parity and rejection test from
  the locked foreign cwd for both installed facades.

The outer installed harness needs local fixture/process permissions; numerical
children deny network, HTTP, exec, spawn, FFI and SQLite and run in a network-isolated
namespace. Each phase cleans its private copied fixture. These are Linux fixture
checks under the recorded retained runtime, not evidence for all platforms, the
native bundle command, global installation, AGI/ASI, or representative AI quality.
