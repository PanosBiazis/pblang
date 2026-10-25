# Per-card presentation

Cards may share a theme while choosing their own corners, border emphasis and depth. The existing Modern UI, app.surface and platform builders remain in use.

```sapp
let UI=import("stdlib:modern_ui.sapp");
let Style=import("stdlib:ui_style.sapp");
let view=UI.screen([
    UI.card([UI.heading("Overview")],Style.card_preset("surface")),
    UI.card([UI.text("Highlighted outline")],
        Style.card_preset("outlined",map{card_radius:0,padding:20})),
    UI.card([UI.text("A quiet surface")],Style.card_preset("tonal"))
]);
UI.run("Cards",view,Style.options("light"));
```

| Option | Values | Omitted or inherited behavior |
| --- | --- | --- |
| `card_variant` | `surface`, `outlined`, `tonal` | Existing platform fill/border |
| `card_radius` | integer 0–32; nil | Inherits application's `card_radius`, then existing platform fallback |
| `card_elevation` | integer 0–8; nil | Existing platform shadow/depth |

Zero means square corners or no elevation; it is distinct from omission/nil. `card_variant: nil` is invalid; omit it to retain the default. Local radius overrides global radius, including explicit zero.

`ui_style.card_preset(name, overrides)` returns a fresh map. The default name is `surface`. Presets are `surface` (radius 14, elevation 2), `outlined` (radius 14, elevation 0) and `tonal` (radius 20, elevation 0). Overrides may contain normal container options, including padding, gap and name. The caller's map is unchanged. Unknown presets, variants and out-of-range values fail before rendering.

`surface` uses palette panel/border roles; `outlined` uses panel/accent; `tonal` uses field for its fill and border. They reuse existing colors rather than introduce fixed color strings. A theme may intentionally use the same panel and field color; customize `palette.field` for a distinct tonal surface. Existing text and muted roles remain unchanged. Default light/dark palette text contrasts against these surfaces are covered by tests; custom palettes remain the application's responsibility.

## Existing native GUI

```sapp
let GUI=import("stdlib:gui.sapp");
let panel=GUI.panel("summary",16,16,280,160,"Summary");
GUI.style_panel(panel,Style.card_preset("outlined",map{card_radius:8}));
```

`style_panel` validates and copies only card presentation fields onto an existing panel, returning that panel. It does not change geometry, binding or other widgets. This round does not add a PBIDE visual control or promise Designer source round-tripping for the new helper.

## Rendering

- Native GUI keeps its existing rounded-card and rectangular edge-shadow helpers. Explicit radius/elevation follows the UI scale; the legacy fallback retains its previous geometry.
- Web emits scoped CSS with enough specificity for a local radius to override the global token. Explicit depth zero removes the CSS shadow; positive values choose a bounded shadow offset/blur.
- Android reuses GradientDrawable for backgrounds, corner radius and border, and View.setElevation for explicit depth. Widgets, actions and the APK toolchain are retained. When no shared palette is supplied, the adapter keeps its own existing color roles. Native Android elevation can be clipped by parent bounds; leave suitable spacing around raised cards.
- Shadows are backend-specific approximations, not identical pixels or a new compositing engine. Child clipping, hit areas, spacing and sizing are unchanged.
- iOS retains these values in the portable UI contract; this change does not implement or validate an iOS runtime.

Omitted local options retain the old rendering behavior. The unstyled frozen Web/Android reference is unchanged. Global-only card styles are also compared with the previous Android generator. No new dependencies, fonts, Rust implementation or network operations are introduced.

Run `examples/one_line_apps/modern_cards.sapp` with `--web`, `--android-build` or `--native`; add `--dark` to select the dark theme. The example uses an ordinary palette.field override to illustrate a light tonal surface. See `tests/semantics/ui_card_style_test.sapp` for preset, input, palette contrast, inheritance, rendering and portable-contract checks.
