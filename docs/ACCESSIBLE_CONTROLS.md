# Contextual names for controls

Use the existing `accessibility_label` option when a short visible label needs context:

```sapp
UI.button("Open", UI.action_set_text("status", "Opened Atlas"),
    map{name:"open_atlas", accessibility_label:"Open project Atlas"});
UI.toggle("Notifications", true, nil,
    map{accessibility_label:"Notifications for project Atlas"});
```

Prefer the visible label alone when it is sufficient. An explicit name replaces the button content in the accessibility tree, so use it only when the extra context is needed and translate it together with the UI. See [W3C guidance on names and descriptions](https://www.w3.org/WAI/ARIA/apg/practices/names-and-descriptions/).

Keep the visible wording in the accessible name (for example, `Open project Atlas`, not `Atlas details`). This helps users associate the spoken name with what is shown. Labels are names, not private data or long instructions; do not put passwords, tokens or hidden account values in them.

## Behavior and compatibility

- Web buttons now honor explicit nonempty labels with `aria-label`. Checkbox labels are emitted on the actual input, preserving the enclosing visible label and normal click/keyboard semantics. No extra tab stop, wrapper role, CSS or event handler is added.
- Omitted or empty labels on these Web controls retain their visible text as the accessible name and generate the same HTML as before. Existing input, textarea, choice, slider, progress and image label behavior is unchanged.
- Android already emits control labels through `View.setContentDescription`; the generator and platform behavior are unchanged. This work adds shared input validation, not a new Android accessibility service.
- Native GUI widgets now retain explicit label metadata. Keyboard focus and activation remain unchanged. Metadata preservation is **not** an OS screen-reader bridge; this work does not add Windows UI Automation, macOS accessibility or Linux AT-SPI integration.
- iOS retains metadata in the portable contract only; no iOS runtime is implemented or verified here.
- Labels on containers/headings/plain text do not acquire new semantics in this change. Do not use this option to hide descendants or as a replacement for visible field labels. Dynamic status announcements require a live-region mechanism; changing text alone is not a claim of screen-reader announcement.

## Validation

If present, `accessibility_label` must be a string of at most 1024 PBLang characters. Empty text is allowed for existing conventions. Tab and newline are accepted; other C0 controls and DEL are rejected. Explicit nil, numbers, booleans, arrays and maps are rejected rather than stringified. Invalid labels fail shared compilation; Web/Android also revalidate a bundle whose view was mutated after compilation. This deliberately tightens previously unvalidated input; valid existing labels and omitted options retain behavior.

Names are escaped as HTML attribute text, not interpreted as markup. The Android path retains its existing text encoding. No external library, font, telemetry, network call or stored personal data is introduced.

Run `examples/one_line_apps/modern_accessible_controls.sapp` with `--web`, `--native` or `--android-build`. The example includes two Open buttons with distinct contextual names, a checkbox, a disabled button and a working text update. `tests/semantics/ui_accessible_controls_test.sapp` covers escaping, limits, malformed/mutated input, native keyboard behavior, portable metadata and Android class generation. Class generation is not an installed-device or TalkBack test.
