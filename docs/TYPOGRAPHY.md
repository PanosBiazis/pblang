# Optional typography for Modern UI

Use the existing `UI.heading`, `UI.text` and `UI.button` nodes with ordinary option maps.
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

These options apply to heading/text nodes and buttons. They do not change editable
controls, cursor metrics or button actions. Alignment needs available space;
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

An explicit size reserves `ceil(font_size * line_height)` (default line height 1.25) logical layout height unless
`h` is supplied; `min_h` remains a lower bound. Resizing retains typography.
Labels are single-line unless `wrap:true` is requested. Native bidi shaping and
arbitrary per-element font paths are not supported.

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

## Multiline text

Add `wrap: true` and optional `max_lines` to a text/heading options map. See [Text flow](TEXT_FLOW.md) for sizing, Designer controls and platform behavior.

## Shared reading styles and installed font families

`font_family` accepts `system`, `sans`, `serif` or `monospace`. `line_height` accepts
1–2.5 and participates in native measurement and drawing, Web CSS and Android
`TextView.setLineSpacing`. Optional typography also applies to buttons; editable
controls retain their caret, hit-testing and selection metrics.

```pblang
let view=Type.apply(UI.screen([
    UI.heading("Document",map{font_size:28,font_weight:"bold"}),
    UI.text("Read comfortably.",map{wrap:true}),
    UI.button("Continue",nil,map{font_size:15})
]),map{font_family:"sans",font_size:16,line_height:1.45,text_spacing:"natural"});
```

`Type.apply` returns a detached presentation tree. Explicit node settings win over
common defaults. It accepts only presentation fields and observes the existing
24-level, 1,024-node and 256-child limits. Existing trees are unaffected until the
helper or the new fields are used. It does not restyle editable controls or card
header strings. Web/Android output without typography remains unchanged.

Native `text_spacing:"natural"` connects the existing `ttf.sapp` metrics to the
existing antialiased glyph painter. Supported TrueType runs use actual glyph
advances and a common baseline, including precomposed Greek accents and descenders.
It does not introduce another rasterizer. Ink extending left of the line, unsupported characters,
zero-advance combining marks, CFF/collections, unavailable bounded file access and
runs exceeding 4,096 characters use the retained host fallback. That fallback has
a minimum advance of half the font size and centers individual glyphs vertically.
Neither path promises kerning, ligatures, bidi or full Unicode shaping.

`fixed` remains the native default; Web/Android use their platform's normal shaping
in either case. Wrapping reserves conservative fixed-cell rows for renderer-independent
layout; natural drawing does not compact those reserved rows. Wider native glyphs
are fitted within each row with an ellipsis when necessary. A measured run that
exceeds the available line height falls back to the existing rendering path.

The new `font_metrics.sapp` reads through **existing bounded `secure_file` I/O**:
at most 1 MiB per installed font, 64 table/cmap records, 16,384 glyph metrics and
1,024 cmap4 segments. Unsupported or unavailable reads never retry through unbounded
file I/O. Parsed fonts are retained per window/family; up to 2,048 character metrics
per font and 128 measured runs of at most 1,024 characters are cached. Font parsing
is not repeated per glyph or frame. Glyphs are painted in at most 32 sparse batches
per run, up to 8,192px wide, using the existing native text painter with one-pixel
space cells. Overlapping glyph boxes start a new ordered phase, preserving painting
order; complex shaping remains a fallback, not a new capability.

Cache limits apply to both entries and stored payload: 8 MiB for four parsed font
snapshots/metadata, 128 KiB of character metrics per font, 512 KiB of positioned
runs and 128 KiB of width results, at most **9.125 MiB** in total. The accounting
uses raw font bytes and UTF-8 serialized records plus a metadata allowance; this
is not a bound on the VM's allocator overhead or total process memory. Entry and
record-shape limits bound that overhead separately. Full run/width caches are
cleared before admitting a record that would exceed their budget. Window close
clears all typography caches and later styled calls on that window are rejected.
`window.textCacheStatus()` exposes counts in bytes and the limit without text.
No cache contents are persisted, logged or sent to another process.

`graphics.Window.textMeasure` and `textStyled` resolve only the four family names
through installed font candidates. `system` and `sans` honor the existing
`PBIDE_FONT` / `PBLANG_FONT` startup override; serif/monospace prefer their own local
candidates and fall back to the selected system font. There is no network font
loader. A successful native label draw exposes `widget.text_status` with the
requested family, actual font path, renderer, positioning mode and spacing. This path is diagnostic
local state and is not logged or transmitted automatically. The measurement cache
is window-local, holds at most 128 strings of at most 1,024 characters each, and
uses cached TrueType positioning or the retained host's offscreen metrics. Longer text is measured without caching.
The public styled path bounds text to 65,536 characters, size to 6–256 physical px
and fixed advance to 0–512 px.

Unavailable styled rendering, forced bitmap mode and clipping preserve the existing
fallback. Native bold remains a second drawing pass. Web uses a closed set of CSS
families; Android emits `Typeface.create` and `setTypeface`, including button styles.
These generated contracts do not establish device execution. Windows/macOS font
candidate paths and iOS view metadata remain unverified on those platforms.
