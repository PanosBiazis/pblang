# Procedural Galaxy Scene

This PBLang-only example replaces a dependency-heavy NumPy, Matplotlib and
Tkinter scatter script with a deterministic, bounded procedural galaxy model.
It generates a repeatable spiral point cloud from a validated seed and groups
stars into five local color palettes without remote assets or services.

The shared presentation model compiles into responsive Web, native desktop and
Android UI records. The same data also produces a portable PBLang 3D scene with
a skybox, point clouds and camera. The visual engine output is a render contract;
actual GPU drawing and interaction depend on a compatible host renderer and are
not claimed by the headless tests.

The density presets contain 128, 512 or 2,048 stars. The public model accepts
only 16..4,096 stars, 2..8 arms and bounded geometric parameters. Its small PRNG
is deliberately reproducible, not cryptographically secure and not suitable
for passwords, keys or security decisions.

Run the summary:

```text
pblang run examples/procedural_galaxy_scene/main.sapp
```

Run the single-pass maximum-workload check:

```text
pblang run examples/procedural_galaxy_scene/benchmark.sapp
```

No network, subprocess, credential, personal path, foreign runtime or copied
binary asset is used. The source inspiration is represented only by its SHA-256
content digest for provenance checking.
