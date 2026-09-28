# Toggle actions across GUI and Web

A checkbox can run the same bounded portable action as a button:

```sapp
UI.toggle("Notifications", false,
    UI.action_set_text("status", "Notifications changed"),
    map{name:"notify", accessibility_label:"Notifications for this project"});
```

The established `UI.toggle(text, checked, nil, map{action: ...})` form remains supported and produces the same output as this shorthand. No builder, product integration or control hierarchy is replaced.

## Event behavior

- Native GUI connects the portable action to the checkbox's existing onchange hook. The state is changed and its binding pushed before the action runs. Space, Enter and pointer activation use the existing GUI event path; disabled controls stay inactive and are skipped by keyboard focus.
- An explicit native onchange callback retains precedence over `options.action`, matching existing button callback precedence. Native callbacks remain nonportable and are rejected by Web/Android adapters. Invalid callback/action maps fail normal shared validation.
- Web checkbox actions now run on the actual input's `change` event. Clicking a label can forward another click to its input; listening to both bubbling clicks previously ran an action twice. The new path executes once per checkbox change. Buttons continue to use click, including native keyboard activation.
- A disabled Web checkbox does not execute an action when its visible label is clicked. The change listener also checks the real input's disabled state. This is interaction correctness, not an authorization boundary for business operations.
- Views without checkbox actions retain their previous generated runtime exactly. Existing HTML, CSS, labels and action payload formats are unchanged. Views with checkbox actions intentionally receive the corrected event dispatch.
- Assigning the checked property programmatically does not itself dispatch a change or action. Ordinary actions keep their existing set-text/message/sequence meaning; this change does not add arbitrary JavaScript callbacks or expose checkbox values to external services.

## Platform and accessibility limits

Android already emits checkbox click listeners for portable actions. The new shorthand normalizes to the same view contract and generated class as `options.action`; this does not constitute a new device, TalkBack or APK installation test. iOS retains the portable action contract only. Native GUI keyboard interaction is not an OS screen-reader bridge. Updating status text is not a promise that assistive technology will announce it automatically.

The existing `accessibility_label` validation, node/action limits, HTML escaping and bounded action recursion remain in force. No new dependency, telemetry, network operation, credential or personal-data storage is introduced.

Run `examples/one_line_apps/modern_toggle_actions.sapp` with `--web`, `--native` or `--android-build`. It includes an active checkbox, a disabled checkbox and a reset button. `tests/semantics/ui_toggle_actions_test.sapp` covers native keyboard/pointer behavior, disabled controls, callback precedence, shorthand compatibility, invalid actions and existing Android/iOS contracts. Real browser checks should exercise clicks on both the input and its visible label, because those have different click propagation.
