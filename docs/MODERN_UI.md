# Modern UI

`stdlib/modern_ui.sapp` is the PBLang-only declarative UI layer. It combines the
existing application facade, responsive viewport policy, native GUI toolkit,
theme palettes and modern rounded geometry behind one bounded view tree.

## Small application

```pblang
let app = import("stdlib/app.sapp");
let ui = import("stdlib/modern_ui.sapp");

app.modern("Tasks", ui.screen([
    ui.heading("Tasks"),
    ui.card([
        ui.row([
            ui.input("", map{ name: "task", bind: "task" }),
            ui.button("Add", ui.action_set_text("status", "Saved"))
        ]),
        ui.text("Ready", map{ name: "status" }),
        ui.toggle("Show completed", true)
    ], map{ title: "Today" })
]), map{ model: map{ task: "" }, theme: "dark" });
```

This is a convenience layer, not a restricted application model. `ui.build`
returns the normal `gui.sapp` form, so native applications can use lower-level
widget, graphics, event and model APIs when needed. Portable actions
(`action_set_text`, `action_message`, and bounded `action_sequence`) compile to
native callbacks, Android click listeners, and the hardened browser action
runtime.

## One tree and selectable mode

The recommended cross-platform entry point is `app.surface`. Components are
shared; only delivery mode and operation change:

```pblang
let result = app.surface("Tasks", view, map{
    mode: "web",             // auto, desktop, web, android
    operation: "preview",    // preview, run, build, export
    theme: "light"
});
```

`auto` chooses a linked adapter for the current host. Desktop hosts select the
native window adapter, Android hosts select the Android builder, and otherwise
the Web adapter is used. The resolved mode is returned by
`ui.surface_plan(view, options)` before any window, socket, or build starts.
Explicit modes never silently fall back to another platform.

## Layout

The built-in containers are `screen`, `column`, `row`, `grid`, and `card`.
Rows stack vertically on compact viewports unless `responsive: false` is set.
Grids reduce to one column on compact displays and to at most two columns on
medium displays. All geometry is recomputed deterministically after a native
resize event.

The Android adapter does not freeze the preview width into the APK. Generated
Activities query the installed device's `screenWidthDp` and choose stacked rows
and one-column grids below a bounded 600dp breakpoint. Android recreates the
Activity after an orientation change, so the current width is evaluated again.
Set `compact_breakpoint_dp` from 320 through 1200 when a product needs a
different tested breakpoint. Interactive controls default to at least 48dp and
the root `ScrollView` remains the overflow fallback.

The built-in controls are `heading`, `text`, `button`, `input`, `area`,
`toggle`, `choice`, `slider`, `progress`, `image`, `separator`, and `spacer`.
Controls support explicit stable `name` values, model `bind` fields, callbacks,
size overrides, enabled/read-only state and bounded text.

## Spacing

Use `ui_spacing.preset("comfortable")` for shared container gap/padding defaults, or override either value in an ordinary options map. Android now honors explicit root/card padding and responsive child gaps. See [Shared spacing](UI_SPACING.md) for compatibility details.

## Themes

Pass `theme: "dark"` or `theme: "light"`. A `palette` map can override the
RGB/RGBA `bg`, `panel`, `field`, `border`, `accent`, `text`, and `muted` roles.
Unknown roles and non-color values fail before a window opens.

## Card appearance

Use `ui_style.card_preset` or per-card `card_variant`, `card_radius` and `card_elevation` options. Local choices override the corresponding application defaults. See [Card styles](CARD_STYLES.md) for direct GUI use and platform behavior.

## Typography

Headings and text support optional `font_size`, `font_weight` and `text_align`
settings. `ui_typography.preset` provides five reusable styles, with ordinary map
overrides. See [Typography](TYPOGRAPHY.md) for native fallback details and examples.

## Android APK

The same tree can produce a signed debug APK in one call:

```pblang
let result = app.modern_android("Tasks", view, map{
    package: "org.example.tasks",
    min_sdk: 21,
    target_sdk: 34,
    output: "release/tasks-debug.apk"
});
```

The Android adapter is written in PBLang and emits `MainActivity.class`
directly through `jvm_classfile.sapp`. It does not generate Java or Kotlin.
`android_toolchain.sapp` discovers the installed SDK and uses D8, AAPT2,
zipalign, and apksigner only as Android binary-format and signing tools. The
build fails by stage and verifies the signature, alignment, package name, and
launchable Activity before returning success. It publishes `.sha256` and
`.manifest.json` verification sidecars beside the APK.

The renderer uses native Android `ScrollView`, `LinearLayout`, `GridLayout`,
`TextView`, `EditText`, `Button`, `CheckBox`, `SeekBar`, `ProgressBar`,
`ImageView`, and `Space` controls. It preserves nested layout structure, uses
density-independent geometry, enables hardware acceleration, disables
cleartext traffic and backups, and emits accessibility descriptions. JVM
constants use strict Modified UTF-8, so Unicode UI text is retained.

Arbitrary native closures are intentionally rejected by an Android build.
Use portable actions for behavior shared across platforms, or a platform
service adapter for advanced OS functionality. `choice` is emitted as an
interactive native Android spinner. An `image` reserves an accessible native
image surface by default. Set `android_images: true` to embed bounded static
RGB/RGBA frames. Optional per-image `fit` and `image_rendering` settings control
framing and pixel edges; see [Image rendering](IMAGE_RENDERING.md) for examples,
limits and adapter behavior.

## Web application

The same tree produces a terminating browser preview:

```pblang
let result = app.surface("Tasks", view, map{
    mode: "web", operation: "preview", theme: "dark"
});
```

Use `operation: "run"` for the blocking loopback server. Use
`operation: "export"` with `output: "release/tasks-web"` for a deterministic
static site containing `index.html`, external CSS, the external bounded action
table/runtime, and a digest manifest. Application code contains no authored
HTML, CSS, or JavaScript.

The Web renderer HTML-escapes all text and attributes, rejects host callbacks,
checks portable action targets, uses semantic native controls and accessible
labels, supports keyboard focus and reduced-motion preferences, and emits no
inline styles or handlers. Strict CSP permits only same-origin external
scripts/styles. Responsive rows stack and grids collapse on compact screens.
Palette roles and light/dark themes are shared with the native renderer.
Generated CSS also covers 360px-class screens, short landscape viewports,
notched safe areas, dynamic viewport height, long-word wrapping and horizontal
overflow prevention. Controls use a 48px minimum touch height.

## PBIDE visual workflow

The PBIDE Designer `APP` selector now cycles through `gui`, `modern`,
`android`, `web`, `both`, and `doc`. Selecting `modern` and pressing `->CODE`
generates a responsive `modern_ui` tree with native preview and run entry
points. Selecting `android` generates the same tree plus a real Android build
mode. Selecting `web` generates preview, server and static-export entry points
from the same components. Designer buttons use portable
message actions; labels, text boxes, checks, choices, sliders, panels, and
image placeholders map to their declarative equivalents. The generated
PBLang source remains visible and editable.

## Target contract

`ui.compile(view, map{ target: "android" })` produces the same validated,
responsive `pblang.modern-ui.bundle.v1` record contract used by native
desktop. Supported contract targets are `native`, `desktop`, `android`, `ios`,
`linux`, `windows`, `macos`, `web`, and `embedded`.

Native desktop, Android and Web are linked executable adapters. iOS and
embedded compilation remains `executable: false` until a real adapter is
linked. Calling `ui.build` remains the low-level desktop builder; use
`app.surface` for mode-aware execution, build, or export. The bundle remains
the stable input for adapters and the PBIDE visual designer.

## Safety limits

Validation is completed before layout or rendering. A tree is limited to 1,024
nodes, depth 24, 256 children per container, 512 select options, 65,536
characters per text value, 128 portable actions, and action depth 8. The
Android executable adapter tightens this to 96 widgets, 32 interactive
buttons, and 4,096 characters per widget constant to bound class size.
Unsupported controls, duplicate names, invalid callbacks/actions, missing
targets, negative geometry and unknown targets are rejected.

See `examples/one_line_apps/modern.sapp` for a terminating preview,
`modern_live.sapp` for the interactive native window, and
`modern_android.sapp` for a complete APK build. `modern_web.sapp` demonstrates
the secure browser preview, `modern_web_export.sapp` creates the static
artifacts, and `modern_surface.sapp` demonstrates mode selection without
changing components.
