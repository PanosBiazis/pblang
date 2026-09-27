# Image framing and pixel rendering

The existing Modern UI image node supports optional framing and pixel-rendering
preferences. Existing applications can keep their current trees, build tools and
platform adapters. No new rendering engine or external dependency is required.

```pblang
let UI = import("stdlib:modern_ui.sapp");
let pixels = [[[255, 80, 40], [40, 150, 240]]];
let view = UI.screen([
    UI.image(pixels, map{
        name: "preview", h: 160,
        fit: "contain", image_rendering: "pixelated",
        src: "./preview.png", accessibility_label: "Two colored pixels"
    })
]);
```

`src` is the Web asset URL; applications must provide that asset in their export
directory. Native GUI and opted-in Android builds use the pixel frame. The
example below exports its own synthetic PNG, requiring no network downloads.

## Two independent choices

| Option | Values | Meaning |
| --- | --- | --- |
| `fit` | `contain` | Keep the entire image, centered, with unused space when aspect ratios differ. |
| `fit` | `cover` | Fill the frame with a centered crop, preserving proportions. |
| `fit` | `stretch` | Fill both dimensions, allowing aspect-ratio distortion. |
| `image_rendering` | `auto` | Use the adapter's normal rendering: browser default, Android bitmap filtering, existing nearest-neighbor native GUI. |
| `image_rendering` | `pixelated` | Preserve hard pixel edges: browser pixelated rendering, unfiltered Android bitmap scaling, existing nearest-neighbor native GUI. |

Set image dimensions such as `h`, `w` and `min_h` through the existing options
map. Preferences are per image and can be assembled with ordinary PBLang maps.
Unknown modes and explicit invalid values are rejected during validation.

Omitting both options preserves each adapter's previous behavior. In particular,
the existing Web rule uses `contain`, while native GUI uses `stretch`. Explicit
`fit` is recommended when matching layouts across adapters. Native GUI currently
does not add smooth interpolation for `auto`; the preference is retained on the
widget for downstream extensions. Browser scaling at fractional ratios follows
the browser's implementation and is not guaranteed to be pixel-identical to Android.

## Embedded Android images

```pblang
let result = UI.build_android("Preview", view, map{
    android_images: true,
    package: "org.example.preview",
    output: "build/preview.apk"
});
```

`android_images` defaults to `false`. Disabled embedding retains the existing
accessible placeholder and generated Activity bytes. Enabled embedding uses the
existing PBLang PNG encoder and JVM class-file generator, then the unchanged
Android packaging pipeline. It supports planar RGB/RGBA arrays and packed
`pblang.image.rgb8.v1` / `pblang.image.rgba8.v1` maps, including validated offset
and row stride. RGB pixels become opaque; RGBA alpha is preserved in the PNG.

This facility is for small static artwork, icons and pixel previews:

- At most 16,384 source pixels per image and 65,536 per Activity.
- Each dimension is at most 1,024; packed input is at most 262,144 bytes.
- Channels must be integer bytes from 0 through 255.
- Encoded Base64 is bounded to 131,072 characters and split into 16,000-character
  JVM string constants. The existing class-file method-size limits still apply.

Validation runs before image encoding; malformed inputs fail instead of silently
falling back. This does not add remote image loading, streaming textures or large
photo assets. It introduces no telemetry, network requests or new permissions.

## Runnable example

From the repository root, using the installed PBLang runtime:

```sh
target/release/pblang run examples/one_line_apps/modern_image_fit.sapp --allow-all -- --web --pixelated
target/release/pblang run examples/one_line_apps/modern_image_fit.sapp --allow-all -- --android-build --pixelated
target/release/pblang run examples/one_line_apps/modern_image_fit.sapp --allow-all -- --native --pixelated
```

The Web output is `build/modern-image-fit/`. Android requires the existing SDK
toolchain and produces `build/pblang-image-studio.apk`. Omit `--pixelated` to
compare `auto`. Running without a delivery flag returns the native preview.

## Platform and test scope

The implementation extends shared PBLang source, native GUI, Web and Android.
It does not introduce an iOS adapter or change desktop window backends. Windows
and macOS consume the shared native code but require their own runtime checks.
The semantic tests cover framing geometry, native draw commands, strict options,
PNG round trips, packed frames, Android embedding and unchanged legacy output.
