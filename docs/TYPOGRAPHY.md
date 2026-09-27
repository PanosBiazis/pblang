# Optional typography for Modern UI

Use the existing `UI.heading` and `UI.text` nodes with ordinary option maps.
These settings extend the native GUI, Web and Android adapters; absent options
preserve the previous appearance and generated Web/Android output.

```pblang
let UI = import("stdlib:modern_ui.sapp");
let Type = import("stdlib:ui_typography.sapp");
let title = UI.heading("Your workspace", Type.preset("display", map{
    name: "title", text_align: "center", fill_width: true
}));
let body = UI.text("Ready to create.", map{
    font_size: 18, font_weight: "normal", text_align: "left"
});
```

| Option | Accepted values | Behavior |
| --- | --- | --- |
| `font_size` | Integer 8–64 | Logical text size; CSS px, Android sp, native pixels multiplied by UI scale. |
| `font_weight` | `normal`, `bold` | Explicit text emphasis. |
| `text_align` | `left`, `center`, `right` | Physical alignment within the text element's available width. |

These options apply to heading/text nodes only. They do not change editable
controls, cursor metrics or button behavior. Alignment needs available space;
use the existing `fill_width: true` option for Android full-width labels, and
`grow: 1` on Web row cards that should share available space.

## Five ready-to-use styles

`Type.preset(name, overrides)` returns a fresh options map. Overrides can include
normal UI options such as `name`, `tone`, `h` and `fill_width`, as well as validated
typography. Neither the supplied map nor a shared preset is mutated.

| Name | Size | Weight |
| --- | --- | --- |
| `display` | 40 | bold |
| `title` | 28 | bold |
| `subtitle` | 20 | bold |
| `body` | 16 | normal |
| `caption` | 12 | normal |

All presets start with left alignment. Unknown presets, invalid weights or
alignment strings, fractions and out-of-range sizes fail validation. The adapters
revalidate a compiled tree before generating output, including caller mutations.

## Native behavior and compatibility

An explicit size reserves `ceil(font_size * 1.25)` logical layout height unless
`h` is supplied; `min_h` remains a lower bound. Resizing retains typography.
Native labels remain single-line and use the existing ellipsis behavior. This
feature does not introduce wrapping, bidi shaping or custom font loading.

The existing antialiased renderer receives the explicit font size and a fixed
character advance. Without an available font renderer, the existing bitmap font
uses its bounded integer grid; its size approximates the requested size. Native
bold uses two offset drawing passes rather than a separate bold font file.
Labels shorter than their font height use a clipped bitmap fallback. Existing
viewport clipping also keeps the existing bitmap path. Browser and Android fonts
remain platform-owned, so pixel-identical rendering is not promised.

Unstyled labels retain their previous path and geometry. Explicit typography
settings do not retroactively change card title strings; use a heading inside the
card when its typography needs customization.

## PBIDE visual authoring

In the native Designer, select a label and click `TYPE` to cycle through body,
subtitle, title, display and caption. Click `ALIGN` to cycle left, center and
right. Other selected widget kinds remain unchanged. The canvas uses the same
GUI text renderer as the generated application.

`->CODE` preserves these options for native GUI and Modern UI/Web/Android
outputs. Native code uses `gui.styled_label(name, x, y, w, h, text, options)`;
`<-CODE` recognizes that literal constructor and restores geometry and styles.
The strict importer rejects unsupported or invalid typography expressions.
Existing unstyled constructors, bindings and 2D/3D widgets retain their existing
paths. Arbitrary Modern UI source import is outside this native import subset.

## Example

```sh
target/release/pblang run examples/one_line_apps/modern_typography.sapp --allow-all -- --web
target/release/pblang run examples/one_line_apps/modern_typography.sapp --allow-all -- --android-build
target/release/pblang run examples/one_line_apps/modern_typography.sapp --allow-all -- --native
```

Web exports to `build/modern-typography/`. Android uses the existing SDK pipeline
and produces `build/pblang-type-studio.apk`. Without a delivery flag the example
returns the native preview. The example uses synthetic text and local system
fonts; it adds no remote font downloads, dependencies or telemetry.

Shared native code serves desktop adapters. Windows/macOS and iOS runtime
verification remains separate; these options do not introduce an iOS renderer.
