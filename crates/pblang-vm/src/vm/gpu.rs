//! Optional GPU compute backend (wgpu). Only compiled with the `gpu` feature.
//!
//! Provides an f32 matmul that runs on a hardware GPU via wgpu (DX12/Vulkan/Metal/GL).
//! pblang tensors are f64, so inputs are downcast to f32 and results widened back —
//! lower precision than the deterministic CPU f64 path. That is why callers gate this
//! behind `PBLANG_GPU=1`: it is opt-in, for throughput on large matmuls, not the default.
//!
//! The device/pipeline are initialized once (lazily) and reused. By default `HighPerformance`
//! power preference is requested so a discrete/integrated GPU is chosen over the software
//! rasterizer fallback where one is reachable. Set `PBLANG_GPU_ADAPTER` to override the
//! choice: an index into `list_adapters()`, a device type (`integrated`/`discrete`/`cpu`),
//! or a name substring — so e.g. an integrated GPU can be selected explicitly. Only ONE
//! adapter is used at a time; pblang does not split a matmul across multiple GPUs.

use std::sync::OnceLock;

use wgpu::util::DeviceExt;

const SHADER: &str = r#"
@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read> b: array<f32>;
@group(0) @binding(2) var<storage, read_write> c: array<f32>;
@group(0) @binding(3) var<uniform> dims: vec4<u32>; // m, k, n, pad

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let m = dims.x; let k = dims.y; let n = dims.z;
    let row = gid.x; let col = gid.y;
    if (row >= m || col >= n) { return; }
    var sum = 0.0;
    for (var p: u32 = 0u; p < k; p = p + 1u) {
        sum = sum + a[row * k + p] * b[p * n + col];
    }
    c[row * n + col] = sum;
}
"#;

struct GpuCtx {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    adapter_name: String,
    adapter_type: String,
}

static CTX: OnceLock<Option<GpuCtx>> = OnceLock::new();

/// Human label for a wgpu device type.
fn type_str(t: wgpu::DeviceType) -> &'static str {
    match t {
        wgpu::DeviceType::IntegratedGpu => "integrated",
        wgpu::DeviceType::DiscreteGpu => "discrete",
        wgpu::DeviceType::VirtualGpu => "virtual",
        wgpu::DeviceType::Cpu => "cpu",
        _ => "other",
    }
}

/// Enumerate every GPU adapter wgpu can see: (name, type, backend). This sees adapters the
/// OS display inventory may not (e.g. both an integrated and a discrete GPU, or a software
/// fallback), so it complements OS-level detection.
pub fn list_adapters() -> Vec<(String, String, String)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    instance
        .enumerate_adapters(wgpu::Backends::all())
        .iter()
        .map(|a| {
            let i = a.get_info();
            (
                i.name,
                type_str(i.device_type).to_string(),
                format!("{:?}", i.backend),
            )
        })
        .collect()
}

/// Pick the adapter to use. Honors `PBLANG_GPU_ADAPTER` (index / type / name substring);
/// otherwise requests a HighPerformance adapter as before.
fn pick_adapter(instance: &wgpu::Instance) -> Option<wgpu::Adapter> {
    if let Ok(sel) = std::env::var("PBLANG_GPU_ADAPTER") {
        let sel = sel.trim().to_string();
        // by index into the enumerated list
        if let Ok(idx) = sel.parse::<usize>() {
            let mut all = instance.enumerate_adapters(wgpu::Backends::all());
            if idx < all.len() {
                return Some(all.swap_remove(idx));
            }
        }
        // by device type or name substring
        let want = sel.to_lowercase();
        for a in instance.enumerate_adapters(wgpu::Backends::all()) {
            let info = a.get_info();
            if type_str(info.device_type) == want || info.name.to_lowercase().contains(&want) {
                return Some(a);
            }
        }
        // fall through to the default if the selector matched nothing
    }
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
}

fn init() -> Option<GpuCtx> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pick_adapter(&instance)?;
    let info = adapter.get_info();
    let adapter_name = info.name.clone();
    let adapter_type = type_str(info.device_type).to_string();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None)).ok()?;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("pblang-matmul"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("pblang-matmul"),
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    Some(GpuCtx { device, queue, pipeline, adapter_name, adapter_type })
}

fn ctx() -> Option<&'static GpuCtx> {
    CTX.get_or_init(init).as_ref()
}

/// Whether a GPU device initialized successfully (cached after the first call).
pub fn available() -> bool {
    ctx().is_some()
}

/// Name of the chosen GPU adapter, if any.
pub fn adapter_name() -> Option<String> {
    ctx().map(|c| c.adapter_name.clone())
}

/// Type ("integrated"/"discrete"/...) of the chosen GPU adapter, if any.
pub fn adapter_type() -> Option<String> {
    ctx().map(|c| c.adapter_type.clone())
}

/// GPU matmul: (m x k) * (k x n) -> (m x n), computed in f32 on the GPU. Returns None if
/// no GPU is available. Results are deterministic for a given device but differ from the
/// CPU f64 path at f32 precision.
pub fn matmul(a: &[f64], b: &[f64], m: usize, k: usize, n: usize) -> Option<Vec<f64>> {
    let g = ctx()?;
    let af: Vec<f32> = a.iter().map(|&x| x as f32).collect();
    let bf: Vec<f32> = b.iter().map(|&x| x as f32).collect();

    let a_buf = g.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("a"),
        contents: bytemuck::cast_slice(&af),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let b_buf = g.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("b"),
        contents: bytemuck::cast_slice(&bf),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let out_size = (m * n) as u64 * 4;
    let c_buf = g.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("c"),
        size: out_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let dims = [m as u32, k as u32, n as u32, 0u32];
    let dim_buf = g.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("dims"),
        contents: bytemuck::cast_slice(&dims),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let staging = g.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("staging"),
        size: out_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let bind_group = g.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &g.pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: a_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: b_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: c_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: dim_buf.as_entire_binding() },
        ],
    });

    let mut encoder = g
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&g.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let gx = ((m + 7) / 8) as u32;
        let gy = ((n + 7) / 8) as u32;
        pass.dispatch_workgroups(gx, gy, 1);
    }
    encoder.copy_buffer_to_buffer(&c_buf, 0, &staging, 0, out_size);
    g.queue.submit(Some(encoder.finish()));

    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = g.device.poll(wgpu::Maintain::Wait);
    let mapped = slice.get_mapped_range();
    let result: Vec<f32> = bytemuck::cast_slice(&mapped).to_vec();
    drop(mapped);
    staging.unmap();
    Some(result.into_iter().map(|x| x as f64).collect())
}
