# pblang GPU Stack — 2D · 3D · nD (pure pblang, codegen-based)

A GPU rendering stack written **entirely in pblang**, in the same spirit as the `codegen_*`
backends: pblang doesn't call the GPU at runtime, it **generates the whole program** — draw
buffers, shaders, and the host glue — which a thin generated host (Rust/C/Python/JS) or an
FFI then runs. The identical draw data also renders **today** on all CPU cores (`render_soft`),
so nothing is blocked on having a GPU.

## Why this shape

pblang has no GPU-draw builtin (that would be a Rust/shader backend). Rather than hand-write
that backend, pblang **emits** it — exactly how `codegen_x64` emits real x86 that the OS runs.
You keep "no more Rust," and the emitted host is inspectable, multi-target, and guarded.

## Modules

| Module | Role |
|--------|------|
| `gpu2d.sapp` | 2D draw-command IR (rect / rounded-rect / glyph) → one **instanced** 14-float buffer; parallel bake |
| `gpu2d_tess.sapp` | Mesh path: thick lines, polylines, convex fills, **gradients** (per-vertex colour) → triangle buffer |
| `gpu2d_shader.sapp` | Emits WGSL + GLSL (SDF rounded corners, atlas glyphs, mesh) + the vertex-layout contract |
| `glyph_atlas.sapp` | Shelf packer — all glyphs in one texture (pblang's `stb_rect_pack`) |
| `glyph_raster.sapp` | Outline → alpha bitmap (even-odd scanline, **antialiased** supersampling), blit into the atlas |
| `ttf.sapp` | TrueType decode: readers, table directory, `glyf` simple + composite outlines, `cmap` format 4 |
| `transform2d.sapp` | 2D affine + matrix stack + clip-rect stack |
| `mat4.sapp` | 4×4 MVP math (model/view/projection) for 3D |
| `gpu3d.sapp` | 3D meshes: tri/quad/box/plane/sphere/cylinder, auto face-normals, MVP + Lambert shaders |
| `gpu_nd.sapp` | **Every dimension**: nD points, plane rotation, project n→…→2D; hypercubes (tesseract, 5-cube…) |
| `gpu2d_host.sapp` / `gpu3d_host.sapp` | Polyglot host codegen: Rust/wgpu, C/OpenGL, Python/moderngl, JS/WebGPU |
| `gpu2d_pipeline.sapp` | Capstone facade: build a scene, bake, and **`guarded_run`** (safe execution) |

## The dimensional progression

`transform2d` → `mat4` → `gpu_nd` are the same idea (a linear map + a projection) at 2, 3,
and n dimensions. `gpu_nd` projects any dimension down to 2D, which the 2D tessellator then
strokes — so a spinning tesseract is just `project_cube_2d(4, ...)` feeding `gpu2d_tess.line`.

## Safety: nothing foreign runs unguarded

The host emitters are **pure codegen** — every `emit_*` returns a string and never executes.
Foreign code runs only through:

- `polyglot_guard.sapp` — scans code for shell/exec/network/destructive/eval patterns
  (canonicalising with `deobfuscate` first, so zero-width/homoglyph tricks can't hide an
  `eval`), plus a dependency allowlist.
- `polyglot_policy.sapp` — execution **modes**: `secure` (scan-safe plus explicit sandbox), `sandbox`
  (run all, isolated runner), `audit` (run all, log findings), `unrestricted` (run all, no
  scan), `report` (analyse only).

Honest boundary: a denylist is defense-in-depth, not proof. Run untrusted polyglot code
sandboxed with no network and a scratch FS. And because pblang is memory-safe, a malformed
input (e.g. a corrupt font) **throws** — catchable — rather than corrupting memory.

## Quick start

```
let GP = import("stdlib/gpu2d_pipeline.sapp");
let sc = GP.new(800, 600);
GP.rrect(sc, 10, 10, 200, 120, 8, panelColor);   // rounded panel  (instanced quad)
GP.gradient(sc, 0, 570, 800, 30, top, bottom, true);   // gradient footer (mesh)
let baked = GP.bake_par(sc);                       // parallel bake, 2 draw calls
GP.guarded_run(sc, "python-moderngl", POLY.run);   // assert safe, THEN run
```

## Status

- Complete + tested (TDD): all modules above, 2D/3D/nD, shaders, hosts, atlas, rasterizer,
  guard + modes.
- Runnable today with no GPU via `render_soft` (CPU, all cores); GPU present needs the
  emitted host to run on a machine with a GPU + toolchain.
- Backlog: `cmap` formats beyond 4, live PBIDE GPU backend, more 3D primitives.
