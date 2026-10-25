# pbide GPU Render Plan — real wgpu surface backend

Concrete, honest roadmap to move pbide's rendering from CPU (minifb / optional SDL2
texture-blit) to a **real wgpu surface + swapchain** path, ending in GPU text. This is
the render counterpart to the existing wgpu **compute** backend in
`crates/pblang-vm/src/vm/gpu.rs`. See also the pure-pblang codegen stack in
[`GPU_Stack.md`](./GPU_Stack.md) — that emits GPU host programs; this doc is about a
*native Rust* wgpu backend inside `pblang-graphics` that draws the pbide UI directly.

> **STATUS — M1 COMPILES (2026-07-05, Round 8); runtime still UNTESTED on real GPU.**
> M1 is now **compiler-verified** against wgpu 24 + raw-window-handle 0.6 — both the crate
> alone and the full `pblang` binary via a new top-level `gpu-render` feature:
>
> ```sh
> cargo build -p pblang-graphics --features wgpu-render   # crate alone
> cargo build --features gpu-render                        # full pblang/pbide binary
> cargo build --features gpu-render-shared                 # + reuse the compute wgpu device
> ```
>
> The minifb-0.28 ↔ raw-window-handle-0.6 risk (below) is **resolved**: minifb 0.28 exposes
> the rwh-0.6 `HasWindowHandle`/`HasDisplayHandle` traits, so `SurfaceTargetUnsafe::RawHandle`
> builds with no winit shim. What remains is **frame-testing on a machine with a display+GPU**
> (the dev sandbox is headless) and M2 (text via a glyph atlas — today text is still the CPU
> path). With `gpu-render` **off** (the default), none of this code compiles and the default
> build is byte-for-byte unchanged.

## Non-interference guarantees

- New optional deps (`wgpu` 0.24, `raw-window-handle` 0.6, `pollster`, `bytemuck`) are
  declared `optional = true` and pulled in **only** by the off-by-default `wgpu-render`
  feature. `default = []` does not include it.
- `crates/pblang-graphics/src/wgpu_backend.rs` is wrapped in `#![cfg(feature = "wgpu-render")]`
  and only `pub mod`-declared in `lib.rs` under `#[cfg(feature = "wgpu-render")]`.
- Every dispatch hook in `graphics.rs` is guarded by `#[cfg(feature = "wgpu-render")]`
  **and** a runtime `crate::wgpu_backend::use_wgpu()` / `::has(id)` check, placed *before*
  the existing sdl2/minifb paths. With the feature off, the file is identical to before.
- The shared-device accessor `pblang_vm::vm::gpu::shared_device()` lives entirely inside
  the existing `#[cfg(feature = "gpu")]` module and only *reads* the already-built ctx; the
  `gpu`-feature build is otherwise unchanged.

## Runtime selection

The backend is chosen at runtime the same way sdl2 is (`PBLANG_GFX=sdl2`):

- `PBIDE_GRAPHICS=wgpu`  (pbide launch convention), or
- `PBLANG_GFX=wgpu`      (generic convention)

…**and** the binary must have been built `--features gpu-render` (or
`--features pblang-graphics/wgpu-render` for the crate alone).

## Milestones

### M1 — surface + cleared frame + solid rects  *(feature-gated; COMPILES; runtime-untested on GPU)*
Files: `crates/pblang-graphics/src/wgpu_backend.rs` (new),
`crates/pblang-graphics/src/lib.rs`, `crates/pblang-graphics/src/graphics.rs`,
`crates/pblang-graphics/Cargo.toml`, `crates/pblang-vm/src/vm/gpu.rs` (shared_device).

- Create `wgpu::Surface` from the minifb window's raw handles (raw-window-handle 0.6,
  via `Instance::create_surface_unsafe` + `SurfaceTargetUnsafe::RawHandle`).
- Configure a swapchain: sRGB-preferred format, `Fifo` present mode, resize-reconfigure.
- Per frame: acquire → one render pass that **clears** to the background color → draw all
  queued **solid rects** via a single **instanced-quad** pipeline → present.
- `gfxclear` / `gfxdrawrect` / `gfxdrawrects` / `gfxdrawpoint` / `gfxdrawline` route to
  wgpu. `gfxdrawline` decomposes into 1px rect instances along a viewport-clipped
  Bresenham walk (reuses the unit-tested `clip_line`, so it stays bounded).
- `gfxdrawtextaa` returns a graceful `ok:false` "not implemented (M2)" result — it does
  **not** error or fall through to the CPU path.

WGSL (embedded in `wgpu_backend.rs`, `RECT_SHADER`):
- Vertex: `@builtin(vertex_index)` picks one of 6 corners of a unit quad; per-instance
  `rect: vec4<f32>` (x,y,w,h pixels, top-left origin) + `color: vec4<f32>` (rgba 0..1);
  a `screen: vec2<f32>` uniform maps pixel space (+y down) to clip space (+y up).
- Fragment: passthrough color (with alpha blending enabled on the color target).

### M2 — glyph atlas texture + instanced text  *(DONE 2026-07-05, Round 9; compiles, GPU-untested)*
Files: `wgpu_backend.rs`, reuse of `graphics.rs` rasterizer. **Implemented exactly as planned:**

- ✅ Glyphs rasterized through the **existing** `fontdue` path via the new
  `graphics::rasterize_glyph_for_atlas` (wraps `rasterize_cached_glyph`, so GPU + CPU share the
  font/glyph caches and produce identical coverage) — no new font code.
- ✅ Packed into a single **R8Unorm** atlas (`ATLAS_SIZE`=1024) with a shelf packer + 1px gutter
  (`ensure_glyph`), keyed by `(font_path, char, px.to_bits())`; on overflow the atlas clears and
  repacks. Upload via `queue.write_texture` (wgpu-24 `TexelCopyTextureInfo`/`TexelCopyBufferLayout`).
- ✅ Second **instanced pipeline** (`TEXT_SHADER`): per-instance `rect`+`uv`+`color`; samples the
  R8 coverage as alpha, `alpha *= cov`, alpha-blended; group(0)=screen uniform (shared),
  group(1)=atlas texture+sampler (Nearest). Drawn after rects in the same render pass.
- ✅ `draw_text` mirrors the CPU `draw_text_aa_into` layout (same advance, same vertical
  centering), and `gfxdrawtextaa` now emits one instance per glyph and returns real
  `TextDrawStats` (width/height/glyphs + accurate glyph-cache deltas).
- **Remaining:** frame-test on a real display+GPU; M3 damage/scissor + persistent buffers.

### M3 — damage / scissor + persistent buffers  *(persistent buffers DONE 2026-07-05, Round 11)*
- ✅ **Persistent, grow-on-demand instance buffers.** `present()` no longer calls
  `create_buffer_init` every frame; both the rect and glyph instance data now go into
  reused `VERTEX|COPY_DST` buffers (`rect_ibuf`/`glyph_ibuf`) via `queue.write_buffer`, growing
  (doubling) only when a frame needs more room (`ensure_and_write`). Cuts per-frame GPU
  allocation to zero in steady state. Compiler-verified.
- ⏳ **Damage / scissor** (deferred): set `RenderPass` scissor rects to redraw only changed
  panels. Needs damage info the current immediate-mode API (pending cleared each frame) doesn't
  carry — a small API addition (dirty-rect hints from pbide) is the prerequisite.
- Optional: coalesce rect + text passes; measure vs the CPU path.

### M4 — make it a first-class (eventually default) backend
- Wire `shared_device()`: if `pblang_vm::vm::gpu::shared_device()` returns a device that
  reports this surface **compatible**, reuse it instead of requesting a second adapter
  (the compute device was created with `compatible_surface: None`, so this is best-effort
  with a fallback — see the TODO in `wgpu_backend::create`).
- Broaden test coverage (headless adapter in CI via `force_fallback_adapter`), benchmark
  against minifb/sdl2, then consider flipping pbide's default when a GPU is present.

## How the device is shared from `vm/gpu.rs`

`crates/pblang-vm/src/vm/gpu.rs` now exposes (under `#[cfg(feature = "gpu")]`):

```rust
pub fn shared_device() -> Option<(wgpu::Device, wgpu::Queue)>
```

It clones the `Device`/`Queue` out of the lazily-initialized `GpuCtx` (both are cheap,
`Arc`-backed handles). It does **not** change `ctx()`, `matmul()`, or any existing
behavior. Because the compute device is requested with `compatible_surface: None`, it is
not guaranteed presentable to an arbitrary window; the renderer treats it as a fast path
and falls back to its own surface-compatible device (M1 always requests its own; wiring
this in is M4).

## Known risks (must be validated by an actual build)

1. ~~**UNCOMPILED.**~~ **RESOLVED (2026-07-05).** The wgpu 24 API shapes (surface creation,
   `SurfaceConfiguration`, `RenderPassColorAttachment`, pipeline descriptors) are now
   compiler-checked — `cargo check -p pblang --features gpu-render` is green.
2. ~~**minifb raw-window-handle support.**~~ **RESOLVED (2026-07-05).** minifb 0.28 pulls
   raw-window-handle 0.6.2 and implements `HasWindowHandle`/`HasDisplayHandle`, so the
   `SurfaceTargetUnsafe::RawHandle` path builds with no winit shim. (Was flagged as the most
   likely build break; it wasn't.)
3. **`unsafe` surface lifetime.** `create_surface_unsafe` yields `Surface<'static>`; we
   keep the minifb `Window` co-located in `WgpuWindow` so it outlives the surface. Field
   drop order (surface before window is NOT guaranteed by declaration order across all
   cases) should be double-checked; if needed, add an explicit `Drop` or use a safe
   windowing crate.
4. **Version pinning.** `wgpu` is pinned to `24` to match pblang-vm so a device can be
   shared without a type mismatch. If either crate bumps wgpu, bump both together.

Cross-references: [`GPU_Stack.md`](./GPU_Stack.md) (pure-pblang codegen GPU stack) and
`crates/pblang-vm/src/vm/gpu.rs` (wgpu compute backend + `shared_device`).
