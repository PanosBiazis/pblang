# Responsive Shape Gallery

This PBLang-only example migrates a fixed Windows GDI drawing exercise into a
portable, responsive vector-scene model. It retains the source rectangle,
ellipse, line, triangle and arc while removing the Win32-only window procedure,
fixed 400 x 400 layout, clipped arc and native graphics-object lifetime risks.

The example also extends `stdlib:visual_engine.sapp` with validated first-class
`ellipse2d` and `arc2d` node contracts. A shared PBLang UI tree compiles into
responsive Web, native desktop and Android records, while the geometry adapts
to compact phone, desktop and ultrawide viewports.

Run the summary:

```text
pblang run examples/responsive_shape_gallery/main.sapp
```

The visual output is a portable render plan. Live rasterization still depends
on a host renderer that supports the node contracts and is not claimed by the
headless test. No network, subprocess, credentials, personal paths, copied
binary assets or foreign runtime are used.
