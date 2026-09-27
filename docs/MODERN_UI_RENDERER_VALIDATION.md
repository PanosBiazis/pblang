# Current-view validation at Web/Android generation

## Fixed behavior

`modern_ui.compile` returns a bundle that borrows the view. Previously the
Web/Android adapters checked its earlier `records` and then separately traversed
the current `view`. Replacing children after compilation could bypass checks
on callbacks, action targets, duplicate names and adapter record counts.

`stdlib/modern_ui_validation.sapp` is now a shared dependency leaf with no
renderer imports. It contains the existing core validator; public
`modern_ui.validate` keeps its result shape and diagnostics through delegation.
`renderer_bundle` validates the current view, reconstructs card/leaf records in
the same preorder/name scheme as the compiler, and returns a narrow internal
bundle. Incoming records, counts, geometry/profile and executable claims are
not treated as validation evidence or publisher authenticity.

Web generation refreshes before its portable callback/action checks and HTML
rendering. All Web entry points pass through generation. Android's public
`activity_class` refreshes before class construction, so direct callers cannot
bypass it. Android `generate` refreshes as well and reports current widget/action
counts rather than stale metadata. This can perform two bounded validation
passes; it is not an immutable snapshot optimization.

The shared limits remain 1024 source nodes, depth 24, 256 children/container,
128 actions and action depth 8. Android retains its 96 **record-like** nodes
(cards/leaves) and 32 interactive action triggers; its emission traversal also
counts source nodes against 1024. Layout containers and generated headings are
not all counted as records, so 96 is not a claim about total Android View objects.
Existing bytecode/text limits remain in the Android generator.

Valid view changes made before generation are revalidated and used. Invalid
changes fail before successful artifact return or downstream file/SDK work.
Record-only edits are ignored: edit the view, not `bundle.records`, to change
Web/Android output. The original caller bundle is not rewritten.

## Web name collision correction

The browser uses one `data-pb-name` namespace for controls **and** containers.
A same-named row/column/screen/grid previously could shadow a text-capable
control and redirect `set_text` to that container. This was UI target confusion,
not script execution. Web validation now rejects duplicate resolved names across
every rendered node, including collisions between explicit names and generated
names such as `screen_0` or `column_0_0`. Rename conflicting containers/controls.

This intentionally tightens invalid Web definitions. Android's field namespace
and card/leaf record counts are unchanged. Valid names and record preorder keep
their existing meaning.

## Verification

- Both mutation tests failed before the repair because an added callback button
  was accepted behind the earlier records. They now pass without invoking it.
- Replacement/addition, removed action targets, empty/nil forged records,
  duplicate/unknown controls, onchange callbacks, excessive children/depth/nodes,
  deep/cyclic actions and cyclic view structures are checked.
- Direct Android class generation rejects 97 current records; generated widget
  and action metadata follows valid current edits. Exactly 1024 core source
  nodes are accepted, while 1025 are rejected.
- Web container/control name collisions failed their new regression before the
  name fix. All four layout container kinds and explicit/generated collisions
  are now rejected before browser runtime generation.
- The synthetic reference captures the pre-change Web digest and SHA-256 of
  the JSON-serialized Android class-byte array. Both remain exactly equal after
  repair; the Android value is not presented as a raw APK or binary digest.

Focused tests: `modern_ui_validation`, `modern_ui_web_mutation`,
`modern_ui_android_mutation`, `modern_ui_renderer_reference`, existing native/
Web/Android/scaling/affordance tests, UI variants and PBIDE device-preview checks.
Tests generate data in memory except the existing Web static-export regression,
which cleans its own files. No installed APK, live browser, physical device or
complete suite is claimed by these checks.

## Remaining boundaries

This fixes mutation **before** revalidation, not concurrent mutation during a
call. Views/options remain trusted in-process developer values; no locks,
immutable snapshot, VM hard heap/preemption or hostile-code sandbox is added.
Core validation retains its existing value coercion and diagnostic contracts.
The new outer validation errors are generic, but older adapter diagnostics can
still name bounded widget/action targets; they are not a privacy-safe reporting
API for arbitrary private input. Existing renderer-specific functionality,
native relayout behavior and physical-platform support are not expanded here.
