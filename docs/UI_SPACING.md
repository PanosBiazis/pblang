# Shared spacing presets

`stdlib:ui_spacing.sapp` supplies optional Modern UI container options. Existing builders and renderer defaults remain in use.

```sapp
let UI=import("stdlib:modern_ui.sapp");
let Space=import("stdlib:ui_spacing.sapp");
let view=UI.screen([
    UI.card([UI.heading("Account"),UI.text("Ready")],
        Space.preset("comfortable",map{gap:16,fill_width:true}))
],Space.preset("spacious"));
```

| Preset | Gap | Padding |
| --- | ---: | ---: |
| `compact` | 6 | 12 |
| `comfortable` (default) | 12 | 20 |
| `spacious` | 20 | 28 |

`preset(name, overrides)` returns a fresh options map. Overrides may include normal container options such as `name`, `columns` and `fill_width`. It does not modify the supplied map. Preset spacing accepts integers from 0 through 8192, including explicit zero. Unknown names and invalid spacing values fail early. Presets do not modify font size, control touch targets or every descendant automatically: apply them to the containers you want to customize.

## Platform behavior

Native GUI and Web already apply explicit `gap` and `padding`. Android now applies these same existing options:

- Screen padding controls the root content inset.
- Card padding overrides the former fixed 14dp inset.
- Columns, screens and cards place a gap above each child after the first. A generated card title counts as the first child.
- Rows place the gap horizontally on a wide device and vertically when responsive layout becomes compact. `responsive:false` keeps a horizontal row.
- Grids apply a left gap between columns and a top gap between rows. The existing Android policy of one compact column and up to six wide columns is retained.

Margins are applied to the actual child LayoutParams after attachment. No spacer Views, replacement controls, additional action targets or new dependencies are introduced. Values convert through the existing Android dp helper; fractional values accepted by the existing low-level Modern UI contract are floored. Root/card padding omitted or nil keeps the previous defaults (12dp/14dp); other container padding defaults to 6dp. Missing, nil or zero gap emits no additional margin code.

This intentionally changes Android output when an application already supplied nonzero `gap` or explicit root/card `padding`: those formerly ignored settings now take effect. The frozen example without the options remains byte-identical. The API, widget names, portable actions and build infrastructure are preserved. This is not a promise of pixel-identical fonts, component sizes or layout across platforms.

The helper works through the existing `app.surface`/Modern UI facade. iOS still receives the portable contract; this change does not add an iOS runtime. Use the existing width/grow/fill-width behavior appropriate to each renderer; spacing does not introduce a new sizing engine or RTL layout policy.

See `examples/one_line_apps/modern_spacing.sapp` for native preview, Web export and Android build using `--native`, `--web` and `--android-build`. Regression coverage is in `tests/semantics/ui_spacing_test.sapp` and the existing frozen renderer/Modern UI suites.
