//! Experimental wgpu RENDER backend (M1: surface + cleared frame + solid rects).
//!
//! This whole file compiles to NOTHING unless the `wgpu-render` cargo feature is on
//! (see the crate-level `#![cfg(feature = "wgpu-render")]` guard below). It is also
//! only *selected* at runtime when `PBIDE_GRAPHICS=wgpu` (or `PBLANG_GFX=wgpu`) is set —
//! mirroring the `sdl2` backend's `use_sdl2()` gate. With the feature off the default
//! build is byte-for-byte unaffected.
//!
//! ## Status — COMPILES (M1, 2026-07-05)
//! Compiler-verified against wgpu 24 + raw-window-handle 0.6 (minifb 0.28 exposes the rwh-0.6
//! traits, so the `SurfaceTargetUnsafe::RawHandle` + `create_surface_unsafe` path builds):
//!   cargo build -p pblang-graphics --features wgpu-render      # crate alone
//!   cargo build --features gpu-render                          # full pblang/pbide binary
//! Runtime rendering still needs a real display+GPU to validate on the target machine (the CI
//! sandbox is headless); the code paths are reviewed but not yet frame-tested. See
//! `docs/GPU_RENDER_PLAN.md` for the roadmap and the M2 glyph-atlas plan (text is still CPU).
//!
//! ## What M1 does
//! - Creates a `wgpu::Surface` from a raw window handle (from minifb, via raw-window-handle).
//! - Configures a swapchain (`Fifo` present mode, sRGB-preferred format).
//! - Each frame: acquires the next texture, runs one render pass that CLEARS to the
//!   background color, then draws N solid-color rectangles through a single instanced-quad
//!   pipeline (one 6-vertex-less quad expanded in the vertex shader; per-instance rect +
//!   color), and presents.
//!
//! ## Device sharing
//! When built with pblang-vm's `gpu` feature, `pblang_vm::vm::gpu::shared_device()` can
//! hand us the *same* `Device`+`Queue` the compute matmul path uses, avoiding a second
//! adapter. Because that compute device was requested with `compatible_surface: None`, it
//! may not be presentable to this surface; so we treat it as a best-effort fast path and
//! fall back to requesting our own surface-compatible device when needed. (M1 keeps things
//! simple and always requests its own device; wiring in `shared_device()` is a small,
//! clearly-marked follow-up — see the TODO in `Backend::new`.)
//!
//! ## Text — M2 IMPLEMENTED (glyph atlas)
//! Glyphs are rasterized through the existing `fontdue` path
//! (`crate::graphics::rasterize_glyph_for_atlas`, shared with the CPU renderer so GPU and CPU
//! text land on identical pixels) into a single R8 glyph-atlas texture via a shelf packer
//! (`ensure_glyph`), then drawn as a second instanced pipeline (`TEXT_SHADER`; per-instance
//! dst rect + atlas uv rect + color) sampling the atlas as alpha coverage, in the same render
//! pass after the rects (`present`). `draw_text` mirrors the CPU layout exactly. Compiler-
//! verified; not yet frame-tested on a real GPU (headless dev env).

#![cfg(feature = "wgpu-render")]

use pblang_ir::{RuntimeError, Value, VmHost};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static GFX_WGPU_WINDOWS: RefCell<HashMap<usize, WgpuWindow>> = RefCell::new(HashMap::new());
}

/// Runtime gate — mirrors `sdl2_backend::use_sdl2()`. Accepts either env var so it slots
/// into both the pbide (`PBIDE_GRAPHICS`) and generic (`PBLANG_GFX`) launch conventions.
pub fn use_wgpu() -> bool {
    matches!(std::env::var("PBIDE_GRAPHICS").as_deref(), Ok("wgpu"))
        || matches!(std::env::var("PBLANG_GFX").as_deref(), Ok("wgpu"))
}

/// One instance = one solid rectangle. `rect` is (x, y, w, h) in pixels (top-left origin);
/// `color` is linear-ish RGBA in 0..1. `repr(C)` + `Pod` so it maps straight into a vertex
/// buffer. The vertex shader turns the pixel rect into clip space using `screen` (uniform).
#[repr(C)]
#[derive(Clone, Copy)]
struct RectInstance {
    rect: [f32; 4],
    color: [f32; 4],
}

// SAFETY: `RectInstance` is a plain-old-data struct of f32 arrays with no padding.
unsafe impl bytemuck::Zeroable for RectInstance {}
unsafe impl bytemuck::Pod for RectInstance {}

/// Screen-size uniform (so the shader can map pixel coords -> clip space). vec2 + pad to 16.
#[repr(C)]
#[derive(Clone, Copy)]
struct ScreenUniform {
    size: [f32; 2],
    _pad: [f32; 2],
}
unsafe impl bytemuck::Zeroable for ScreenUniform {}
unsafe impl bytemuck::Pod for ScreenUniform {}

/// Embedded WGSL. One instanced quad pipeline:
/// - `@builtin(vertex_index)` selects one of 6 corners of a unit quad (two triangles).
/// - per-instance `rect`+`color` position and tint it.
/// - `screen` uniform maps pixel space (top-left origin, +y down) to clip space (+y up).
const RECT_SHADER: &str = r#"
struct Screen { size: vec2<f32>, _pad: vec2<f32>, };
@group(0) @binding(0) var<uniform> screen: Screen;

struct VsIn {
    @builtin(vertex_index) vidx: u32,
    @location(0) rect: vec4<f32>,   // x, y, w, h  (pixels, top-left origin)
    @location(1) color: vec4<f32>,  // rgba 0..1
};
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    // Unit-quad corners for two triangles (0,0)-(1,1).
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[in.vidx];
    // Pixel position of this corner within the rect.
    let px = in.rect.xy + c * in.rect.zw;
    // Pixel -> normalized device coords. +y down in pixels, +y up in clip.
    let ndc = vec2<f32>(
        (px.x / screen.size.x) * 2.0 - 1.0,
        1.0 - (px.y / screen.size.y) * 2.0,
    );
    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

/// Side length (px) of the square R8 glyph-atlas texture (M2). 1024² holds a few thousand
/// cached glyph cells at typical editor sizes; on overflow we clear and repack (rare).
const ATLAS_SIZE: u32 = 1024;

/// One text glyph instance: destination `rect` (x,y,w,h px, top-left origin) — same convention
/// as `RectInstance` so text aligns pixel-for-pixel with rects — plus the glyph's `uv` box in
/// the atlas (u0,v0,u1,v1 in 0..1) and its `color`.
#[repr(C)]
#[derive(Clone, Copy)]
struct GlyphInstance {
    rect: [f32; 4],
    uv: [f32; 4],
    color: [f32; 4],
}
unsafe impl bytemuck::Zeroable for GlyphInstance {}
unsafe impl bytemuck::Pod for GlyphInstance {}

/// Where a rasterized glyph lives in the atlas + the metrics the layout needs. `uv` is the
/// pre-divided 0..1 box; `w`/`h` are the coverage-bitmap size, `xmin` the left bearing.
#[derive(Clone, Copy)]
struct AtlasGlyph {
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
    w: u32,
    h: u32,
    xmin: i32,
}

/// Embedded WGSL for the text pipeline: an instanced quad (like the rect pipeline) that samples
/// the R8 atlas as alpha coverage. group(0)=screen uniform (shared with rects); group(1)=atlas
/// texture + sampler.
const TEXT_SHADER: &str = r#"
struct Screen { size: vec2<f32>, _pad: vec2<f32>, };
@group(0) @binding(0) var<uniform> screen: Screen;
@group(1) @binding(0) var atlas_tex: texture_2d<f32>;
@group(1) @binding(1) var atlas_samp: sampler;

struct VsIn {
    @builtin(vertex_index) vidx: u32,
    @location(0) rect: vec4<f32>,   // x, y, w, h  (pixels, top-left origin)
    @location(1) uv: vec4<f32>,     // u0, v0, u1, v1  (atlas 0..1)
    @location(2) color: vec4<f32>,  // rgba 0..1
};
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_text(in: VsIn) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[in.vidx];
    let px = in.rect.xy + c * in.rect.zw;
    let ndc = vec2<f32>(
        (px.x / screen.size.x) * 2.0 - 1.0,
        1.0 - (px.y / screen.size.y) * 2.0,
    );
    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = vec2<f32>(
        in.uv.x + c.x * (in.uv.z - in.uv.x),
        in.uv.y + c.y * (in.uv.w - in.uv.y),
    );
    out.color = in.color;
    return out;
}

@fragment
fn fs_text(in: VsOut) -> @location(0) vec4<f32> {
    // R8 atlas stores alpha coverage in the red channel; modulate the glyph color's alpha.
    let cov = textureSample(atlas_tex, atlas_samp, in.uv).r;
    return vec4<f32>(in.color.rgb, in.color.a * cov);
}
"#;

/// Split a packed 0xAARRGGBB u32 (the format the rest of the crate uses) into 0..1 rgba.
fn argb_to_rgba_f32(argb: u32) -> [f32; 4] {
    let a = ((argb >> 24) & 0xff) as f32 / 255.0;
    let r = ((argb >> 16) & 0xff) as f32 / 255.0;
    let g = ((argb >> 8) & 0xff) as f32 / 255.0;
    let b = (argb & 0xff) as f32 / 255.0;
    [r, g, b, a]
}

pub(crate) struct WgpuWindow {
    // minifb window: we reuse it purely for event polling + a live OS surface. Drawing goes
    // through wgpu, not minifb's software buffer, so we never call `update_with_buffer`.
    win: minifb::Window,
    // wgpu owns a surface tied to `win`; keep `win` alive at least as long as the surface.
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    screen_buf: wgpu::Buffer,
    screen_bind_group: wgpu::BindGroup,
    // M2 text: a second instanced pipeline that samples an R8 glyph atlas.
    text_pipeline: wgpu::RenderPipeline,
    atlas_texture: wgpu::Texture,
    atlas_bind_group: wgpu::BindGroup,
    // (font_path, char, px.to_bits()) -> where that glyph sits in the atlas.
    atlas_map: HashMap<(String, u32, u32), AtlasGlyph>,
    // Shelf packer cursor: next free x on the current shelf, the shelf's top y, its height.
    pack_x: u32,
    pack_y: u32,
    pack_row_h: u32,
    // Glyph instances queued since the last present (drawn after rects, so text is on top).
    text_pending: Vec<GlyphInstance>,
    // M3: persistent, grow-on-demand instance buffers (VERTEX|COPY_DST). Reused every frame via
    // `queue.write_buffer` instead of allocating a fresh buffer per frame. `*_cap` is byte capacity.
    rect_ibuf: Option<wgpu::Buffer>,
    rect_ibuf_cap: u64,
    glyph_ibuf: Option<wgpu::Buffer>,
    glyph_ibuf_cap: u64,
    w: u32,
    h: u32,
    clear: wgpu::Color,
    // Rectangles queued since the last present (mirrors how the CPU backends batch into a buffer).
    pending: Vec<RectInstance>,
    open: bool,
    last_mouse: Option<(f32, f32)>,
    last_left: bool,
    last_right: bool,
}

/// Create a wgpu-backed window. Real surface + swapchain + pipeline.
pub(crate) fn create(
    host: &mut dyn VmHost,
    width: u32,
    height: u32,
    title: &str,
) -> Result<WgpuWindow, RuntimeError> {
    let opts = minifb::WindowOptions {
        borderless: false,
        title: true,
        resize: true,
        ..minifb::WindowOptions::default()
    };
    let mut win = minifb::Window::new(title, width as usize, height as usize, opts)
        .map_err(|e| host.err(format!("wgpu window: {e}")))?;
    win.set_target_fps(60);

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

    // Build a 'static surface from the window's raw handles. minifb implements the
    // raw-window-handle 0.6 traits. SAFETY: the surface must not outlive `win`; we keep
    // `win` in the same struct as `surface` and drop them together, and we transmute the
    // borrow to 'static only after guaranteeing that co-location.
    let surface = {
        let display_handle = win
            .display_handle()
            .map_err(|e| host.err(format!("wgpu display handle: {e}")))?
            .as_raw();
        let window_handle = win
            .window_handle()
            .map_err(|e| host.err(format!("wgpu window handle: {e}")))?
            .as_raw();
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: display_handle,
            raw_window_handle: window_handle,
        };
        // SAFETY: handles are valid for as long as `win` lives; `win` is co-located with the
        // returned surface in `WgpuWindow` and outlives it (fields drop top-to-bottom, and we
        // never move `win` out). See the TODO about migrating to a fully-safe windowing crate.
        unsafe {
            instance
                .create_surface_unsafe(target)
                .map_err(|e| host.err(format!("wgpu create_surface: {e}")))?
        }
    };

    // TODO(device-sharing): try `pblang_vm::vm::gpu::shared_device()` first and, if that
    // device reports this surface as compatible, reuse it (skips a second adapter). For M1
    // we always request a fresh surface-compatible device for correctness-by-construction.
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .ok_or_else(|| host.err("wgpu: no compatible adapter for surface".to_string()))?;

    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .map_err(|e| host.err(format!("wgpu request_device: {e}")))?;

    // Pick an sRGB surface format if available (nicer color), else the first supported one.
    let caps = surface.get_capabilities(&adapter);
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or_else(|| caps.formats[0]);

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width,
        height,
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
    };
    surface.configure(&device, &config);

    // Screen-size uniform + bind group.
    let screen = ScreenUniform {
        size: [width as f32, height as f32],
        _pad: [0.0, 0.0],
    };
    let screen_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pblang-screen-uniform"),
        size: std::mem::size_of::<ScreenUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&screen_buf, 0, bytemuck::bytes_of(&screen));

    let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("pblang-screen-bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let screen_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("pblang-screen-bg"),
        layout: &bind_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: screen_buf.as_entire_binding(),
        }],
    });

    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("pblang-rect-shader"),
        source: wgpu::ShaderSource::Wgsl(RECT_SHADER.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pblang-rect-pl-layout"),
        bind_group_layouts: &[&bind_layout],
        push_constant_ranges: &[],
    });

    // Per-instance vertex buffer layout: rect (vec4) at location 0, color (vec4) at location 1.
    let instance_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RectInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 1,
            },
        ],
    };

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("pblang-rect-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[instance_layout],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    // ── M2 glyph atlas + text pipeline ───────────────────────────────────────────────
    // An R8 coverage atlas: glyphs are rasterized on the CPU (fontdue, shared with the CPU
    // text path) and uploaded once per (font, char, size); the text pipeline then samples it.
    let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("pblang-glyph-atlas"),
        size: wgpu::Extent3d {
            width: ATLAS_SIZE,
            height: ATLAS_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
    // Nearest sampling: glyphs are rasterized at their exact pixel size and drawn 1:1, so
    // nearest keeps them crisp and avoids bleeding neighbouring atlas cells.
    let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("pblang-glyph-sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let atlas_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("pblang-atlas-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("pblang-atlas-bg"),
        layout: &atlas_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&atlas_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&atlas_sampler),
            },
        ],
    });

    let text_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("pblang-text-shader"),
        source: wgpu::ShaderSource::Wgsl(TEXT_SHADER.into()),
    });
    let text_pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pblang-text-pl-layout"),
        bind_group_layouts: &[&bind_layout, &atlas_bgl],
        push_constant_ranges: &[],
    });
    // Per-instance: rect (vec4) @0, uv (vec4) @1, color (vec4) @2.
    let glyph_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GlyphInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 2,
            },
        ],
    };
    let text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("pblang-text-pipeline"),
        layout: Some(&text_pl_layout),
        vertex: wgpu::VertexState {
            module: &text_module,
            entry_point: Some("vs_text"),
            compilation_options: Default::default(),
            buffers: &[glyph_layout],
        },
        fragment: Some(wgpu::FragmentState {
            module: &text_module,
            entry_point: Some("fs_text"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    Ok(WgpuWindow {
        win,
        surface,
        device,
        queue,
        config,
        pipeline,
        screen_buf,
        screen_bind_group,
        text_pipeline,
        atlas_texture,
        atlas_bind_group,
        atlas_map: HashMap::new(),
        pack_x: 0,
        pack_y: 0,
        pack_row_h: 0,
        text_pending: Vec::new(),
        w: width,
        h: height,
        clear: wgpu::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        pending: Vec::new(),
        rect_ibuf: None,
        rect_ibuf_cap: 0,
        glyph_ibuf: None,
        glyph_ibuf_cap: 0,
        open: true,
        last_mouse: None,
        last_left: false,
        last_right: false,
    })
}

pub(crate) fn insert(id: usize, win: WgpuWindow) {
    GFX_WGPU_WINDOWS.with(|tbl| tbl.borrow_mut().insert(id, win));
}

pub fn has(id: usize) -> bool {
    GFX_WGPU_WINDOWS.with(|tbl| tbl.borrow().contains_key(&id))
}

pub fn remove(id: usize) {
    GFX_WGPU_WINDOWS.with(|tbl| {
        tbl.borrow_mut().remove(&id);
    });
}

pub(crate) fn with_window_mut<F, R>(
    host: &mut dyn VmHost,
    id: usize,
    what: &str,
    f: F,
) -> Result<R, RuntimeError>
where
    F: FnOnce(&mut WgpuWindow) -> Result<R, RuntimeError>,
{
    GFX_WGPU_WINDOWS.with(|tbl| {
        let mut t = tbl.borrow_mut();
        let w = t
            .get_mut(&id)
            .ok_or_else(|| host.err(format!("{what}: invalid handle")))?;
        f(w)
    })
}

impl WgpuWindow {
    pub fn is_open(&self) -> bool {
        self.open && self.win.is_open()
    }

    pub fn size(&self) -> (usize, usize) {
        (self.w as usize, self.h as usize)
    }

    /// Reconfigure the swapchain + screen uniform after a resize.
    fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 || (w == self.w && h == self.h) {
            return;
        }
        self.w = w;
        self.h = h;
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        let screen = ScreenUniform {
            size: [w as f32, h as f32],
            _pad: [0.0, 0.0],
        };
        self.queue
            .write_buffer(&self.screen_buf, 0, bytemuck::bytes_of(&screen));
    }

    /// Set the clear color (used by the next present) and drop any queued rects — matches the
    /// CPU backends' `clear()` which fills the whole buffer with one color.
    pub fn clear(&mut self, color: u32) {
        let [r, g, b, a] = argb_to_rgba_f32(color);
        self.clear = wgpu::Color {
            r: r as f64,
            g: g as f64,
            b: b as f64,
            a: a as f64,
        };
        self.pending.clear();
    }

    pub fn draw_rect(&mut self, x: i64, y: i64, rw: i64, rh: i64, color: u32) {
        if rw <= 0 || rh <= 0 {
            return;
        }
        self.pending.push(RectInstance {
            rect: [x as f32, y as f32, rw as f32, rh as f32],
            color: argb_to_rgba_f32(color),
        });
    }

    pub fn draw_rects(&mut self, rects: &[(i64, i64, i64, i64)], color: u32) {
        let c = argb_to_rgba_f32(color);
        for &(x, y, rw, rh) in rects {
            if rw > 0 && rh > 0 {
                self.pending.push(RectInstance {
                    rect: [x as f32, y as f32, rw as f32, rh as f32],
                    color: c,
                });
            }
        }
    }

    /// Draw a 1px line as a run of 1x1 rect instances along a viewport-clipped Bresenham
    /// walk. Correct for any slope and bounded by the framebuffer (reuses the crate's
    /// unit-tested `clip_line`, so a huge endpoint can't blow up the instance count). M1-simple;
    /// M3 can replace this with a single thick-line quad if the instance volume matters.
    pub fn draw_line(&mut self, x1: i64, y1: i64, x2: i64, y2: i64, color: u32) {
        let (x1, y1, x2, y2) =
            match crate::graphics::clip_line(x1, y1, x2, y2, self.w as usize, self.h as usize) {
                Some(c) => c,
                None => return,
            };
        let c = argb_to_rgba_f32(color);
        let mut x = x1;
        let mut y = y1;
        let dx = (x2 - x1).abs();
        let dy = (y2 - y1).abs();
        let sx = if x1 < x2 { 1 } else { -1 };
        let sy = if y1 < y2 { 1 } else { -1 };
        let mut err = dx - dy;
        loop {
            self.pending.push(RectInstance {
                rect: [x as f32, y as f32, 1.0, 1.0],
                color: c,
            });
            if x == x2 && y == y2 {
                break;
            }
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Ensure `(font_path, ch, px)` is resident in the glyph atlas, rasterizing + uploading it
    /// on first use (shelf packer). Returns the atlas placement, or `None` for whitespace/empty
    /// glyphs (nothing to draw) or if the atlas is full even after a repack.
    fn ensure_glyph(
        &mut self,
        requested_font: &str,
        ch: char,
        px: f32,
    ) -> Option<AtlasGlyph> {
        let g = crate::graphics::rasterize_glyph_for_atlas(requested_font, ch, px).ok()?;
        let key = (g.font_path.clone(), ch as u32, px.to_bits());
        if let Some(existing) = self.atlas_map.get(&key) {
            return Some(*existing);
        }
        if g.width == 0 || g.height == 0 {
            return None; // whitespace / non-inked glyph: advance only, nothing to sample
        }
        let (gw, gh) = (g.width as u32, g.height as u32);
        if gw > ATLAS_SIZE || gh > ATLAS_SIZE {
            return None; // absurdly large glyph; refuse rather than corrupt the atlas
        }
        // Shelf packer with a 1px gutter to stop nearest-sampling from bleeding neighbours.
        const PAD: u32 = 1;
        if self.pack_x + gw + PAD > ATLAS_SIZE {
            // New shelf.
            self.pack_x = 0;
            self.pack_y += self.pack_row_h + PAD;
            self.pack_row_h = 0;
        }
        if self.pack_y + gh + PAD > ATLAS_SIZE {
            // Atlas full: clear and restart. Cached instances from prior frames are already
            // presented, so dropping the map only forces re-upload of still-visible glyphs.
            self.atlas_map.clear();
            self.pack_x = 0;
            self.pack_y = 0;
            self.pack_row_h = 0;
        }
        let (ox, oy) = (self.pack_x, self.pack_y);
        // Upload the R8 coverage rows. write_texture needs bytes_per_row; R8 = width bytes.
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: ox, y: oy, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &g.bitmap,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(gw),
                rows_per_image: Some(gh),
            },
            wgpu::Extent3d {
                width: gw,
                height: gh,
                depth_or_array_layers: 1,
            },
        );
        self.pack_x += gw + PAD;
        if gh > self.pack_row_h {
            self.pack_row_h = gh;
        }
        let inv = 1.0 / ATLAS_SIZE as f32;
        let placed = AtlasGlyph {
            u0: ox as f32 * inv,
            v0: oy as f32 * inv,
            u1: (ox + gw) as f32 * inv,
            v1: (oy + gh) as f32 * inv,
            w: gw,
            h: gh,
            xmin: g.xmin,
            // advance is recomputed by draw_text via a fresh rasterize call cache hit; store
            // nothing extra here to keep AtlasGlyph Copy + small.
        };
        self.atlas_map.insert(key, placed);
        Some(placed)
    }

    /// Queue a run of text at top-left cell origin `(x, y)`, size `px`, in `color` (0xAARRGGBB).
    /// Layout mirrors the CPU `draw_text_aa_into` exactly (same advance + vertical centering),
    /// so GPU and CPU text land on identical pixels. `fixed_advance > 0` gives a monospace cell.
    /// Returns `(glyphs_drawn, width_px, height_px, resolved_font_path)` so callers get the same
    /// layout feedback the CPU path reports (width is used for cursor hit-testing in pbide).
    pub fn draw_text(
        &mut self,
        x: i64,
        y: i64,
        text: &str,
        color: u32,
        px: f32,
        requested_font: &str,
        fixed_advance: f32,
    ) -> (usize, i64, i64, String) {
        if text.is_empty() {
            return (0, 0, 0, String::new());
        }
        let px = px.clamp(6.0, 256.0);
        let rgba = argb_to_rgba_f32(color);
        let mut cursor = x as f32;
        let mut glyphs = 0usize;
        let mut max_right = x;
        let mut max_bottom = y;
        let mut font_path = String::new();
        for ch in text.chars() {
            if ch == '\n' || ch == '\r' {
                continue;
            }
            // Metrics for advance come from the same rasterize path (cache hit after ensure).
            let advance = match crate::graphics::rasterize_glyph_for_atlas(requested_font, ch, px)
            {
                Ok(g) => {
                    if font_path.is_empty() {
                        font_path = g.font_path.clone();
                    }
                    if fixed_advance > 0.0 {
                        fixed_advance
                    } else {
                        g.advance_width.max(px * 0.5)
                    }
                }
                Err(_) => px * 0.5,
            };
            if let Some(gl) = self.ensure_glyph(requested_font, ch, px) {
                let gx = cursor.round() as i64 + gl.xmin as i64;
                let gy = y + (((px as i64) - gl.h as i64) / 2).max(0);
                self.text_pending.push(GlyphInstance {
                    rect: [gx as f32, gy as f32, gl.w as f32, gl.h as f32],
                    uv: [gl.u0, gl.v0, gl.u1, gl.v1],
                    color: rgba,
                });
                max_right = max_right.max(gx + gl.w as i64);
                max_bottom = max_bottom.max(gy + gl.h as i64);
                glyphs += 1;
            }
            cursor += advance;
        }
        ((glyphs), (max_right - x).max(0), (max_bottom - y).max(0), font_path)
    }

    /// Acquire the next swapchain texture, run one render pass (clear + all pending rects),
    /// and present. Rebuilds the instance buffer each frame (fine for M1; M3 will add
    /// persistent buffers + damage/scissor).
    pub fn present(&mut self) -> Result<(), RuntimeError> {
        // Keep the swapchain matched to the live window size.
        let (cw, ch) = self.win.get_size();
        if cw > 0 && ch > 0 {
            self.resize(cw as u32, ch as u32);
        }

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            // Lost/Outdated: reconfigure and skip this frame rather than erroring the program.
            Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            Err(e) => {
                return Err(RuntimeError {
                    message: format!("wgpu acquire: {e}"),
                    span: None,
                    source_name: None,
                    source: None,
                    stack: vec![],
                });
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Upload this frame's rects + glyphs into the persistent instance buffers (M3): reuse
        // the same GPU buffers frame-to-frame, growing only when a frame needs more room.
        let have_rects = !self.pending.is_empty();
        if have_rects {
            let bytes: &[u8] = bytemuck::cast_slice(&self.pending);
            ensure_and_write(
                &self.device,
                &self.queue,
                &mut self.rect_ibuf,
                &mut self.rect_ibuf_cap,
                bytes,
                "pblang-rect-instances",
            );
        }
        let have_text = !self.text_pending.is_empty();
        if have_text {
            let bytes: &[u8] = bytemuck::cast_slice(&self.text_pending);
            ensure_and_write(
                &self.device,
                &self.queue,
                &mut self.glyph_ibuf,
                &mut self.glyph_ibuf_cap,
                bytes,
                "pblang-text-instances",
            );
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("pblang-wgpu-frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pblang-wgpu-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if have_rects {
                if let Some(ref buf) = self.rect_ibuf {
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, &self.screen_bind_group, &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    // 6 vertices (two triangles) per instance, one instance per rect.
                    pass.draw(0..6, 0..self.pending.len() as u32);
                }
            }
            if have_text {
                if let Some(ref buf) = self.glyph_ibuf {
                    pass.set_pipeline(&self.text_pipeline);
                    pass.set_bind_group(0, &self.screen_bind_group, &[]);
                    pass.set_bind_group(1, &self.atlas_bind_group, &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    // 6 vertices per instance, one instance per glyph.
                    pass.draw(0..6, 0..self.text_pending.len() as u32);
                }
            }
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        // Rects + glyphs are consumed by the present; the next frame starts fresh (immediate-mode).
        self.pending.clear();
        self.text_pending.clear();
        Ok(())
    }

    /// Poll OS events off the minifb window and translate them into the same pblang event
    /// maps the other backends emit (so pblang code is portable across backends).
    pub fn poll_events(
        &mut self,
        alloc_array: impl Fn(Vec<Value>) -> Value,
        alloc_map: impl Fn(HashMap<String, Value>) -> Value,
    ) -> Result<Value, RuntimeError> {
        use minifb::{Key, KeyRepeat, MouseButton, MouseMode};
        let mut out: Vec<Value> = Vec::new();

        if !self.win.is_open() {
            self.open = false;
            let mut m = HashMap::new();
            m.insert("type".into(), pblang_ir::alloc_string("close"));
            out.push(alloc_map(m));
            return Ok(alloc_array(out));
        }

        // Resize -> reconfigure swapchain + tell the app so it can relayout.
        let (cw, ch) = self.win.get_size();
        if cw > 0 && ch > 0 && (cw as u32 != self.w || ch as u32 != self.h) {
            self.resize(cw as u32, ch as u32);
            let mut m = HashMap::new();
            m.insert("type".into(), pblang_ir::alloc_string("resize"));
            m.insert("w".into(), Value::Number(cw as f64));
            m.insert("h".into(), Value::Number(ch as f64));
            out.push(alloc_map(m));
        }

        let modifiers = |m: &mut HashMap<String, Value>, win: &minifb::Window| {
            let shift = win.is_key_down(Key::LeftShift) || win.is_key_down(Key::RightShift);
            let ctrl = win.is_key_down(Key::LeftCtrl) || win.is_key_down(Key::RightCtrl);
            let alt = win.is_key_down(Key::LeftAlt) || win.is_key_down(Key::RightAlt);
            let meta = win.is_key_down(Key::LeftSuper) || win.is_key_down(Key::RightSuper);
            m.insert("shift".into(), Value::Bool(shift));
            m.insert("ctrl".into(), Value::Bool(ctrl));
            m.insert("alt".into(), Value::Bool(alt));
            m.insert("meta".into(), Value::Bool(meta));
        };

        for k in self.win.get_keys_pressed(KeyRepeat::Yes) {
            let mut m = HashMap::new();
            m.insert("type".into(), pblang_ir::alloc_string("keydown"));
            m.insert("key".into(), pblang_ir::alloc_string(format!("{k:?}")));
            modifiers(&mut m, &self.win);
            out.push(alloc_map(m));
        }
        for k in self.win.get_keys_released() {
            let mut m = HashMap::new();
            m.insert("type".into(), pblang_ir::alloc_string("keyup"));
            m.insert("key".into(), pblang_ir::alloc_string(format!("{k:?}")));
            modifiers(&mut m, &self.win);
            out.push(alloc_map(m));
        }

        if let Some((mx, my)) = self.win.get_mouse_pos(MouseMode::Discard) {
            let changed = match self.last_mouse {
                Some((px, py)) => px != mx || py != my,
                None => true,
            };
            if changed {
                self.last_mouse = Some((mx, my));
                let mut m = HashMap::new();
                m.insert("type".into(), pblang_ir::alloc_string("mousemove"));
                m.insert("x".into(), Value::Number(mx as f64));
                m.insert("y".into(), Value::Number(my as f64));
                out.push(alloc_map(m));
            }
        }

        let left = self.win.get_mouse_down(MouseButton::Left);
        let right = self.win.get_mouse_down(MouseButton::Right);
        let (bx, by) = self.win.get_mouse_pos(MouseMode::Clamp).unwrap_or((0.0, 0.0));
        let button_event = |ty: &str, button: &str, out: &mut Vec<Value>| {
            let mut m = HashMap::new();
            m.insert("type".into(), pblang_ir::alloc_string(ty));
            m.insert("button".into(), pblang_ir::alloc_string(button));
            m.insert("x".into(), Value::Number(bx as f64));
            m.insert("y".into(), Value::Number(by as f64));
            out.push(alloc_map(m));
        };
        if left && !self.last_left {
            button_event("mousedown", "Left", &mut out);
        }
        if !left && self.last_left {
            button_event("mouseup", "Left", &mut out);
        }
        if right && !self.last_right {
            button_event("mousedown", "Right", &mut out);
        }
        if !right && self.last_right {
            button_event("mouseup", "Right", &mut out);
        }
        self.last_left = left;
        self.last_right = right;

        if let Some((sx, sy)) = self.win.get_scroll_wheel() {
            if sx != 0.0 || sy != 0.0 {
                let mut m = HashMap::new();
                m.insert("type".into(), pblang_ir::alloc_string("scroll"));
                m.insert("dx".into(), Value::Number(sx as f64));
                m.insert("dy".into(), Value::Number(sy as f64));
                out.push(alloc_map(m));
            }
        }

        Ok(alloc_array(out))
    }
}

/// M3 persistent instance buffer: ensure `slot` holds a `VERTEX|COPY_DST` buffer large enough
/// for `data` (growing by doubling, amortized), then upload `data` with `queue.write_buffer`.
/// Reused across frames instead of allocating a fresh buffer each frame. Free function (not a
/// method) so the caller can pass disjoint `&self` fields without a full `&mut self` borrow.
fn ensure_and_write(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    slot: &mut Option<wgpu::Buffer>,
    cap: &mut u64,
    data: &[u8],
    label: &'static str,
) {
    let needed = data.len() as u64;
    if needed == 0 {
        return;
    }
    if slot.is_none() || *cap < needed {
        // Grow to at least `needed`, doubling the old capacity to amortize repeated growth.
        let mut new_cap = (*cap).max(1024);
        while new_cap < needed {
            new_cap = new_cap.saturating_mul(2);
        }
        *slot = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: new_cap,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        *cap = new_cap;
    }
    if let Some(buf) = slot.as_ref() {
        queue.write_buffer(buf, 0, data);
    }
}
