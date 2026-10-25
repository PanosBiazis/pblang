# Optional text flow

Use the existing Modern UI, app.surface and native GUI builders. Text flow is an additive option for `text` and `heading`; omitted options retain legacy renderer output.

```sapp
let UI=import("stdlib:modern_ui.sapp");
let Type=import("stdlib:ui_typography.sapp");
let view=UI.screen([
    UI.heading("A heading that can use more than one line",
        Type.preset("title",map{wrap:true,max_lines:3})),
    UI.text("A longer description that follows the available width.",
        map{wrap:true,max_lines:4,font_size:16,text_align:"left"}),
    UI.button("Continue")
]);
UI.run("Text flow",view);
```

| Option | Accepted values | Effect |
| --- | --- | --- |
| `wrap` | boolean | `true` enables multiline flow; explicit `false` selects single-line rendering. Omitted retains each platform's existing behavior. |
| `max_lines` | integer 1–64 | Maximum visible lines while wrapping; default 64. Retained but inactive when wrapping is off. |
| `h` | existing height option | Native GUI renders only complete rows that fit. With no explicit height, Modern UI reserves the measured wrapped height. |

These options combine with font size, weight, alignment and semantic colors. Native measurement and rendering share logical metrics before screen scaling. Responsive rows and grids measure using the allocated child width. `action_set_text` automatically relayouts wrapped native labels; a direct assignment to a widget's text requires `GUI.relayout(form,form.w,form.h)` when its automatic height should change. The source view is not mutated by these updates.

## Direct GUI and Designer

```sapp
let GUI=import("stdlib:gui.sapp");
let label=GUI.styled_label("description",16,16,260,90,
    "Text inside a fixed rectangle.",map{wrap:true,max_lines:3,font_size:18});
```

PBIDE's **WRAP** toolbar action toggles wrapping for the selected label and expands its height to fit the measured rows within Designer canvas constraints. **TYPE** and **ALIGN** continue to work. Native code generation/import and Modern UI generation preserve `wrap` and `max_lines`. Set a specific line limit through the options map; the toolbar currently exposes the wrapping toggle. Resize a fixed Designer label to allow additional rows.

## Rendering and limits

- Native GUI wraps at spaces, splits long words at the existing `text_layout` grouping boundaries, preserves explicit line breaks, collapses horizontal spaces and replaces tabs with spaces. CRLF/CR normalize to LF. It preserves the existing combining-mark and ZWJ groups; this is not a full Unicode shaping or grapheme-conformance implementation.
- Native row reservation uses the existing fixed scalar advance. Optional `text_spacing:"natural"` uses measured glyph positioning within each row; see [Typography](TYPOGRAPHY.md). Groups wider than a whole line become a bounded dot placeholder. Truncated content ends with `...` (fewer dots for extremely narrow labels). A rectangle too short for one complete line draws no text.
- Web emits optional pre-line wrapping, overflow handling and a line clamp. Android emits TextView single-line, maximum-line and END-ellipsis settings. Each platform uses its existing fonts and layout: identical pixel positions or line breaks across platforms are not promised.
- The pure `text_flow.wrap(text,columns,max_lines)` helper accepts at most 65,536 scalar characters, 0–4,096 columns and 1–64 rows. Native style sizes remain 8–64 logical units. Existing UI tree/depth limits still apply.
- No external fonts, new dependencies, telemetry or network operations are introduced by text flow. Existing glyph availability and rendering backends remain in place.
- iOS receives the existing portable view contract. This change does not add an iOS runtime or demonstrate Windows/macOS/iOS device execution.

Run `examples/one_line_apps/modern_text_flow.sapp` with `--web`, `--android-build` or `--native` through the existing PBLang runner. The Android build uses the existing SDK/toolchain and signing configuration.

Regression tests: `text_flow_test.sapp`, `ui_text_flow_test.sapp`, and the existing typography/Designer/Modern UI suites. The frozen renderer reference test checks unchanged unstyled Web content and Android class bytes; it is not a certification of all products or platforms.
