//! Tensor builtin implementations.
//!
//! Free functions backing the `tensor_*` builtins registered in
//! `VM::builtins()` (vm.rs). Tensors are pblang maps of shape:
//!
//! ```text
//! map{ "shape": [d0,d1,...], "data": [...flat row-major numbers], "dtype": "f32" }
//! ```
//!
//! The native paths below operate directly on this shape and return a
//! new tensor map. They mirror the helpers in `stdlib/tensor.sapp` but
//! in Rust, so they are ~10–100× faster for sizable shapes.
//!
//! Covers: element-wise add/sub/mul/scale; matmul / dot; activations
//! (relu/sigmoid/tanh/softmax/log_softmax/gelu/leaky_relu); reductions
//! (sum/mean/argmax); transpose2d; layer_norm / batch_norm;
//! embedding_lookup; cross_entropy; conv2d / maxpool2d / avgpool2d.
//!
//! Pure computation — no host capabilities. Always registered.

use std::collections::HashMap;

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};
use rayon::prelude::*;

use super::{alloc_array, alloc_map};

// ── Native tensor builtins (fast paths for AI workloads) ─────────────
//
// Tensors are pblang maps of shape:
//     map{ "shape": [d0,d1,...], "data": [...flat row-major numbers], "dtype": "f32" }
// The native paths below operate directly on this shape and return a
// new tensor map. They mirror the helpers in stdlib/tensor.sapp but in
// Rust, so they are ~10-100x faster for sizable shapes.
pub(super) fn tensor_get_shape_data(
    host: &mut dyn VmHost,
    v: &Value,
    fname: &str,
) -> Result<(Vec<usize>, Vec<f64>), RuntimeError> {
    let m = match v {
        Value::Map(m) => m.clone(),
        _ => return Err(host.err(format!("{fname}(): expected tensor map, got {}", v))),
    };
    let m_ref = m.borrow();
    let shape_v = m_ref
        .get("shape")
        .ok_or_else(|| host.err(format!("{fname}(): tensor missing 'shape'")))?
        .clone();
    let data_v = m_ref
        .get("data")
        .ok_or_else(|| host.err(format!("{fname}(): tensor missing 'data'")))?
        .clone();
    drop(m_ref);
    let shape_arr = match shape_v {
        Value::Array(a) => a,
        _ => return Err(host.err(format!("{fname}(): 'shape' must be an array"))),
    };
    let data_arr = match data_v {
        Value::Array(a) => a,
        _ => return Err(host.err(format!("{fname}(): 'data' must be an array"))),
    };
    let shape: Vec<usize> = shape_arr
        .borrow()
        .iter()
        .map(|v| match v {
            Value::Number(n) if *n >= 0.0 => Ok(*n as usize),
            _ => Err(host.err(format!(
                "{fname}(): shape entries must be non-negative numbers"
            ))),
        })
        .collect::<Result<_, _>>()?;
    let data: Vec<f64> = data_arr
        .borrow()
        .iter()
        .map(|v| match v {
            Value::Number(n) => Ok(*n),
            _ => Err(host.err(format!("{fname}(): data entries must be numbers"))),
        })
        .collect::<Result<_, _>>()?;
    let expected: usize =
        shape
            .iter()
            .product::<usize>()
            .max(if shape.is_empty() { 0 } else { 1 });
    if data.len() != expected {
        return Err(host.err(format!(
            "{fname}(): data length {} does not match shape size {}",
            data.len(),
            expected
        )));
    }
    Ok((shape, data))
}

pub(super) fn tensor_make(shape: Vec<usize>, data: Vec<f64>) -> Value {
    let shape_v = alloc_array(shape.into_iter().map(|n| Value::Number(n as f64)).collect());
    let data_v = alloc_array(data.into_iter().map(Value::Number).collect());
    let mut m = HashMap::new();
    m.insert("shape".into(), shape_v);
    m.insert("data".into(), data_v);
    m.insert("dtype".into(), alloc_string("f32"));
    alloc_map(m)
}

pub(super) fn tensor_add_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "tensor_add() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_add")?;
    let (sb, db) = tensor_get_shape_data(host, &args[1], "tensor_add")?;
    if sa != sb {
        return Err(host.err(format!("tensor_add(): shape mismatch {:?} vs {:?}", sa, sb)));
    }
    let out: Vec<f64> = da.iter().zip(db.iter()).map(|(x, y)| x + y).collect();
    Ok(tensor_make(sa, out))
}

pub(super) fn tensor_sub_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "tensor_sub() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_sub")?;
    let (sb, db) = tensor_get_shape_data(host, &args[1], "tensor_sub")?;
    if sa != sb {
        return Err(host.err(format!("tensor_sub(): shape mismatch {:?} vs {:?}", sa, sb)));
    }
    let out: Vec<f64> = da.iter().zip(db.iter()).map(|(x, y)| x - y).collect();
    Ok(tensor_make(sa, out))
}

pub(super) fn tensor_mul_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "tensor_mul() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_mul")?;
    let (sb, db) = tensor_get_shape_data(host, &args[1], "tensor_mul")?;
    if sa != sb {
        return Err(host.err(format!("tensor_mul(): shape mismatch {:?} vs {:?}", sa, sb)));
    }
    let out: Vec<f64> = da.iter().zip(db.iter()).map(|(x, y)| x * y).collect();
    Ok(tensor_make(sa, out))
}

pub(super) fn tensor_scale_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "tensor_scale() expects 2 arguments (tensor, scalar), got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_scale")?;
    let k = match args[1] {
        Value::Number(n) => n,
        _ => return Err(host.err("tensor_scale(): scalar must be a number".into())),
    };
    let out: Vec<f64> = da.iter().map(|x| x * k).collect();
    Ok(tensor_make(sa, out))
}

pub(super) fn tensor_matmul_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "tensor_matmul() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_matmul")?;
    let (sb, db) = tensor_get_shape_data(host, &args[1], "tensor_matmul")?;
    if sa.len() != 2 || sb.len() != 2 {
        return Err(host.err(format!(
            "tensor_matmul(): both arguments must be 2D tensors, got {}D and {}D",
            sa.len(),
            sb.len()
        )));
    }
    let (m, k1) = (sa[0], sa[1]);
    let (k2, n) = (sb[0], sb[1]);
    if k1 != k2 {
        return Err(host.err(format!(
            "tensor_matmul(): inner dim mismatch ({m}x{k1}) * ({k2}x{n})"
        )));
    }
    let k = k1;
    let mut out = vec![0.0_f64; m * n];
    // Each output row is computed independently, in the same ijk order as the sequential
    // path, so results are bit-identical regardless of threading (deterministic). Large
    // products fan out across CPU cores via rayon; small ones stay sequential to avoid
    // thread-pool overhead.
    let compute_row = |i: usize, row: &mut [f64]| {
        for p in 0..k {
            let a_ip = da[i * k + p];
            let b_row = p * n;
            for j in 0..n {
                row[j] += a_ip * db[b_row + j];
            }
        }
    };
    let work = m.saturating_mul(n).saturating_mul(k);
    if work >= 1 << 16 {
        out.par_chunks_mut(n)
            .enumerate()
            .for_each(|(i, row)| compute_row(i, row));
    } else {
        out.chunks_mut(n)
            .enumerate()
            .for_each(|(i, row)| compute_row(i, row));
    }
    Ok(tensor_make(vec![m, n], out))
}

pub(super) fn tensor_dot_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "tensor_dot() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_dot")?;
    let (sb, db) = tensor_get_shape_data(host, &args[1], "tensor_dot")?;
    if sa.len() != 1 || sb.len() != 1 || sa[0] != sb[0] {
        return Err(host.err(format!(
            "tensor_dot(): both arguments must be 1D vectors of equal length (got {:?} and {:?})",
            sa, sb
        )));
    }
    let sum: f64 = da.iter().zip(db.iter()).map(|(x, y)| x * y).sum();
    Ok(Value::Number(sum))
}

pub(super) fn tensor_relu_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "tensor_relu() expects 1 argument, got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_relu")?;
    let out: Vec<f64> = da.iter().map(|x| if *x > 0.0 { *x } else { 0.0 }).collect();
    Ok(tensor_make(sa, out))
}

pub(super) fn tensor_sigmoid_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "tensor_sigmoid() expects 1 argument, got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_sigmoid")?;
    let out: Vec<f64> = da.iter().map(|x| 1.0 / (1.0 + (-x).exp())).collect();
    Ok(tensor_make(sa, out))
}

pub(super) fn tensor_tanh_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "tensor_tanh() expects 1 argument, got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_tanh")?;
    let out: Vec<f64> = da.iter().map(|x| x.tanh()).collect();
    Ok(tensor_make(sa, out))
}

pub(super) fn tensor_softmax_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "tensor_softmax() expects 1 argument, got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_softmax")?;
    if sa.len() != 1 {
        return Err(host.err(format!(
            "tensor_softmax(): only 1D vectors supported, got {}D",
            sa.len()
        )));
    }
    let max = da.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exps: Vec<f64> = da.iter().map(|x| (x - max).exp()).collect();
    let sum: f64 = exps.iter().sum();
    let out: Vec<f64> = exps
        .into_iter()
        .map(|x| if sum > 0.0 { x / sum } else { 0.0 })
        .collect();
    Ok(tensor_make(sa, out))
}

pub(super) fn tensor_sum_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "tensor_sum() expects 1 argument, got {}",
            args.len()
        )));
    }
    let (_sa, da) = tensor_get_shape_data(host, &args[0], "tensor_sum")?;
    Ok(Value::Number(da.iter().sum::<f64>()))
}

pub(super) fn tensor_mean_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "tensor_mean() expects 1 argument, got {}",
            args.len()
        )));
    }
    let (_sa, da) = tensor_get_shape_data(host, &args[0], "tensor_mean")?;
    if da.is_empty() {
        return Ok(Value::Number(0.0));
    }
    let sum: f64 = da.iter().sum();
    Ok(Value::Number(sum / da.len() as f64))
}

pub(super) fn tensor_argmax_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "tensor_argmax() expects 1 argument, got {}",
            args.len()
        )));
    }
    let (_sa, da) = tensor_get_shape_data(host, &args[0], "tensor_argmax")?;
    let (mut idx, mut best) = (0usize, f64::NEG_INFINITY);
    for (i, v) in da.iter().enumerate() {
        if *v > best {
            best = *v;
            idx = i;
        }
    }
    Ok(Value::Number(idx as f64))
}

pub(super) fn tensor_transpose2d_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "tensor_transpose2d() expects 1 argument, got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_transpose2d")?;
    if sa.len() != 2 {
        return Err(host.err(format!(
            "tensor_transpose2d(): only 2D tensors supported, got {}D",
            sa.len()
        )));
    }
    let (m, n) = (sa[0], sa[1]);
    let mut out = vec![0.0_f64; m * n];
    for i in 0..m {
        for j in 0..n {
            out[j * m + i] = da[i * n + j];
        }
    }
    Ok(tensor_make(vec![n, m], out))
}

// ── Native neural-network builtins ────────────────────────────────────
//
// These mirror the most common neural-net primitives so pblang code
// can build CNN / transformer training loops at near-native speed.
//
// Shape conventions:
//   * 2D conv inputs:  [N, C, H, W]  (batch, channels, height, width)
//   * 2D conv kernels: [Co, Ci, KH, KW]
//   * Pooling inputs:  [N, C, H, W]
//   * Layer/batch norm: last dim is the normalization axis for layer_norm,
//     first non-batch dim for batch_norm.
//   * Embedding table: [V, D]; indices: [N] (long-style integers).

pub(super) fn tensor_gelu_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "tensor_gelu() expects 1 argument, got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_gelu")?;
    // GELU(x) = 0.5 * x * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))
    let c = (2.0_f64 / std::f64::consts::PI).sqrt();
    let out: Vec<f64> = da
        .iter()
        .map(|&x| 0.5 * x * (1.0 + (c * (x + 0.044715 * x * x * x)).tanh()))
        .collect();
    Ok(tensor_make(sa, out))
}

pub(super) fn tensor_leaky_relu_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.is_empty() || args.len() > 2 {
        return Err(host.err(format!(
            "tensor_leaky_relu() expects 1 or 2 arguments (tensor, slope?), got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_leaky_relu")?;
    let slope = if args.len() == 2 {
        match &args[1] {
            Value::Number(n) => *n,
            _ => return Err(host.err("tensor_leaky_relu(): slope must be a number".into())),
        }
    } else {
        0.01
    };
    let out: Vec<f64> = da
        .iter()
        .map(|&x| if x > 0.0 { x } else { slope * x })
        .collect();
    Ok(tensor_make(sa, out))
}

pub(super) fn tensor_log_softmax_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "tensor_log_softmax() expects 1 argument, got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_log_softmax")?;
    // log-softmax over the last dim
    let last_dim = *sa.last().unwrap_or(&0);
    if last_dim == 0 {
        return Err(host.err("tensor_log_softmax(): empty last dimension".into()));
    }
    let outer: usize = sa.iter().product::<usize>() / last_dim;
    let mut out = vec![0.0_f64; da.len()];
    for o in 0..outer {
        let off = o * last_dim;
        let m = da[off..off + last_dim]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        let mut sum = 0.0_f64;
        for i in 0..last_dim {
            sum += (da[off + i] - m).exp();
        }
        let log_sum = m + sum.ln();
        for i in 0..last_dim {
            out[off + i] = da[off + i] - log_sum;
        }
    }
    Ok(tensor_make(sa, out))
}

pub(super) fn tensor_layer_norm_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    // tensor_layer_norm(x, eps?) — normalize over the last dim.
    if args.is_empty() || args.len() > 2 {
        return Err(host.err(format!(
            "tensor_layer_norm() expects 1 or 2 arguments (tensor, eps?), got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_layer_norm")?;
    let eps = if args.len() == 2 {
        match &args[1] {
            Value::Number(n) => *n,
            _ => return Err(host.err("tensor_layer_norm(): eps must be a number".into())),
        }
    } else {
        1e-5
    };
    let last = *sa.last().unwrap_or(&0);
    if last == 0 {
        return Err(host.err("tensor_layer_norm(): empty last dim".into()));
    }
    let outer: usize = sa.iter().product::<usize>() / last;
    let mut out = vec![0.0_f64; da.len()];
    for o in 0..outer {
        let off = o * last;
        let mean = da[off..off + last].iter().sum::<f64>() / last as f64;
        let var = da[off..off + last]
            .iter()
            .map(|v| (v - mean) * (v - mean))
            .sum::<f64>()
            / last as f64;
        let inv = 1.0 / (var + eps).sqrt();
        for i in 0..last {
            out[off + i] = (da[off + i] - mean) * inv;
        }
    }
    Ok(tensor_make(sa, out))
}

pub(super) fn tensor_batch_norm_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    // tensor_batch_norm(x, eps?) — normalize over batch dim, per-channel.
    // x shape: [N, C, ...] — the first dim is N, second is C; everything
    // after is flattened.
    if args.is_empty() || args.len() > 2 {
        return Err(host.err(format!(
            "tensor_batch_norm() expects 1 or 2 arguments, got {}",
            args.len()
        )));
    }
    let (sa, da) = tensor_get_shape_data(host, &args[0], "tensor_batch_norm")?;
    if sa.len() < 2 {
        return Err(host.err(
            "tensor_batch_norm(): tensor must have at least 2 dims [N, C, ...]".into(),
        ));
    }
    let eps = if args.len() == 2 {
        match &args[1] {
            Value::Number(n) => *n,
            _ => return Err(host.err("tensor_batch_norm(): eps must be a number".into())),
        }
    } else {
        1e-5
    };
    let n = sa[0];
    let c = sa[1];
    let spatial: usize = sa[2..].iter().product::<usize>().max(1);
    // For each channel c, gather all N*spatial values and normalize.
    let mut out = vec![0.0_f64; da.len()];
    for ch in 0..c {
        let total = n * spatial;
        let mut mean = 0.0_f64;
        for batch in 0..n {
            let base = batch * c * spatial + ch * spatial;
            for k in 0..spatial {
                mean += da[base + k];
            }
        }
        mean /= total as f64;
        let mut var = 0.0_f64;
        for batch in 0..n {
            let base = batch * c * spatial + ch * spatial;
            for k in 0..spatial {
                let d = da[base + k] - mean;
                var += d * d;
            }
        }
        var /= total as f64;
        let inv = 1.0 / (var + eps).sqrt();
        for batch in 0..n {
            let base = batch * c * spatial + ch * spatial;
            for k in 0..spatial {
                out[base + k] = (da[base + k] - mean) * inv;
            }
        }
    }
    Ok(tensor_make(sa, out))
}

pub(super) fn tensor_embedding_lookup_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    // tensor_embedding_lookup(table, indices) -> tensor
    // table shape: [V, D]; indices shape: [N] -> output shape [N, D]
    if args.len() != 2 {
        return Err(host.err(format!(
            "tensor_embedding_lookup() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let (st, dt) = tensor_get_shape_data(host, &args[0], "tensor_embedding_lookup")?;
    if st.len() != 2 {
        return Err(host.err("tensor_embedding_lookup(): table must be 2D [V, D]".into()));
    }
    let v = st[0];
    let d = st[1];
    // Indices: accept either a flat array or a tensor map with shape/data.
    let idx_vals: Vec<i64> = match &args[1] {
        Value::Array(a) => {
            a.borrow()
                .iter()
                .map(|x| match x {
                    Value::Number(n) => Ok(*n as i64),
                    _ => Err(host
                        .err("tensor_embedding_lookup(): indices must be numbers".into())),
                })
                .collect::<Result<_, _>>()?
        }
        Value::Map(_) => {
            let (_si, di) =
                tensor_get_shape_data(host, &args[1], "tensor_embedding_lookup")?;
            di.iter().map(|n| *n as i64).collect()
        }
        _ => {
            return Err(host.err(
                "tensor_embedding_lookup(): indices must be an array or tensor".into(),
            ))
        }
    };
    let n = idx_vals.len();
    let mut out = vec![0.0_f64; n * d];
    for (i, &ix) in idx_vals.iter().enumerate() {
        if ix < 0 || (ix as usize) >= v {
            return Err(host.err(format!(
                "tensor_embedding_lookup(): index {} out of range [0, {})",
                ix, v
            )));
        }
        let src = (ix as usize) * d;
        let dst = i * d;
        out[dst..dst + d].copy_from_slice(&dt[src..src + d]);
    }
    Ok(tensor_make(vec![n, d], out))
}

pub(super) fn tensor_cross_entropy_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    // tensor_cross_entropy(logits, targets) -> scalar (mean loss).
    // logits shape: [N, C]; targets: [N] integer labels (or one-hot [N, C]).
    if args.len() != 2 {
        return Err(host.err(format!(
            "tensor_cross_entropy() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let (sl, dl) = tensor_get_shape_data(host, &args[0], "tensor_cross_entropy")?;
    if sl.len() != 2 {
        return Err(host.err("tensor_cross_entropy(): logits must be 2D [N, C]".into()));
    }
    let n = sl[0];
    let c = sl[1];

    // Read targets — either int labels (Array of Numbers) or one-hot tensor.
    let target_labels: Vec<usize> = match &args[1] {
        Value::Array(a) => a
            .borrow()
            .iter()
            .map(|x| match x {
                Value::Number(num) => Ok(*num as usize),
                _ => Err(host
                    .err("tensor_cross_entropy(): label entries must be numbers".into())),
            })
            .collect::<Result<_, _>>()?,
        Value::Map(_) => {
            let (st, dt) = tensor_get_shape_data(host, &args[1], "tensor_cross_entropy")?;
            if st == vec![n] {
                dt.iter().map(|n| *n as usize).collect()
            } else if st == vec![n, c] {
                // one-hot: take argmax along last dim
                let mut labels = Vec::with_capacity(n);
                for row in 0..n {
                    let off = row * c;
                    let (mut best, mut idx) = (f64::NEG_INFINITY, 0);
                    for j in 0..c {
                        if dt[off + j] > best {
                            best = dt[off + j];
                            idx = j;
                        }
                    }
                    labels.push(idx);
                }
                labels
            } else {
                return Err(host.err(format!(
                    "tensor_cross_entropy(): targets shape {:?} doesn't match logits [N={},C={}]",
                    st, n, c
                )));
            }
        }
        _ => {
            return Err(
                host.err("tensor_cross_entropy(): targets must be array or tensor".into())
            )
        }
    };
    if target_labels.len() != n {
        return Err(host.err(format!(
            "tensor_cross_entropy(): targets length {} != N {}",
            target_labels.len(),
            n
        )));
    }
    // Compute mean(-log_softmax(logits)[label])
    let mut loss = 0.0_f64;
    for (row, &label) in target_labels.iter().enumerate() {
        let off = row * c;
        let m = dl[off..off + c]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        let mut sum_exp = 0.0_f64;
        for j in 0..c {
            sum_exp += (dl[off + j] - m).exp();
        }
        let log_z = m + sum_exp.ln();
        if label >= c {
            return Err(host.err(format!(
                "tensor_cross_entropy(): label {} out of range [0,{})",
                label, c
            )));
        }
        loss += log_z - dl[off + label];
    }
    Ok(Value::Number(loss / n as f64))
}

pub(super) fn tensor_conv2d_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    // tensor_conv2d(x, kernel, stride?, padding?) -> tensor
    // x: [N, Ci, H, W]; kernel: [Co, Ci, KH, KW]
    // stride and padding default to 1 and 0; both are scalar (square).
    if args.len() < 2 || args.len() > 4 {
        return Err(host.err(format!(
            "tensor_conv2d() expects 2-4 arguments (x, kernel, stride?, padding?), got {}",
            args.len()
        )));
    }
    let (sx, dx) = tensor_get_shape_data(host, &args[0], "tensor_conv2d")?;
    let (sk, dk) = tensor_get_shape_data(host, &args[1], "tensor_conv2d")?;
    if sx.len() != 4 || sk.len() != 4 {
        return Err(host.err(
            "tensor_conv2d(): expected [N,Ci,H,W] input and [Co,Ci,KH,KW] kernel".into(),
        ));
    }
    let stride = if args.len() >= 3 {
        match &args[2] {
            Value::Number(n) => (*n as usize).max(1),
            _ => {
                return Err(
                    host.err("tensor_conv2d(): stride must be a positive integer".into())
                )
            }
        }
    } else {
        1
    };
    let padding = if args.len() >= 4 {
        match &args[3] {
            Value::Number(n) => *n as usize,
            _ => {
                return Err(host
                    .err("tensor_conv2d(): padding must be a non-negative integer".into()))
            }
        }
    } else {
        0
    };
    let (n, ci, h, w) = (sx[0], sx[1], sx[2], sx[3]);
    let (co, ci2, kh, kw) = (sk[0], sk[1], sk[2], sk[3]);
    if ci != ci2 {
        return Err(host.err(format!(
            "tensor_conv2d(): channel mismatch (Ci={} vs kernel Ci={})",
            ci, ci2
        )));
    }
    // Output dims:
    let h_padded = h + 2 * padding;
    let w_padded = w + 2 * padding;
    if h_padded < kh || w_padded < kw {
        return Err(host.err("tensor_conv2d(): kernel larger than padded input".into()));
    }
    let oh = (h_padded - kh) / stride + 1;
    let ow = (w_padded - kw) / stride + 1;
    let mut out = vec![0.0_f64; n * co * oh * ow];
    // Naive direct conv. Adequate for small/medium tensors; users wanting
    // im2col can call tensor_matmul on a reshaped buffer.
    for batch in 0..n {
        for oc in 0..co {
            for oy in 0..oh {
                for ox in 0..ow {
                    let mut acc = 0.0_f64;
                    for ic in 0..ci {
                        for ky in 0..kh {
                            for kx in 0..kw {
                                let iy = oy * stride + ky;
                                let ix = ox * stride + kx;
                                if iy < padding || ix < padding {
                                    continue;
                                }
                                let iy_in = iy - padding;
                                let ix_in = ix - padding;
                                if iy_in >= h || ix_in >= w {
                                    continue;
                                }
                                let x_idx =
                                    batch * ci * h * w + ic * h * w + iy_in * w + ix_in;
                                let k_idx = oc * ci * kh * kw + ic * kh * kw + ky * kw + kx;
                                acc += dx[x_idx] * dk[k_idx];
                            }
                        }
                    }
                    let o_idx = batch * co * oh * ow + oc * oh * ow + oy * ow + ox;
                    out[o_idx] = acc;
                }
            }
        }
    }
    Ok(tensor_make(vec![n, co, oh, ow], out))
}

pub(super) fn tensor_maxpool2d_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    // tensor_maxpool2d(x, kernel_size, stride?) -> tensor
    if args.len() < 2 || args.len() > 3 {
        return Err(host.err(format!(
            "tensor_maxpool2d() expects 2 or 3 arguments (x, k, stride?), got {}",
            args.len()
        )));
    }
    let (sx, dx) = tensor_get_shape_data(host, &args[0], "tensor_maxpool2d")?;
    if sx.len() != 4 {
        return Err(host.err("tensor_maxpool2d(): expected [N,C,H,W] tensor".into()));
    }
    let k = match &args[1] {
        Value::Number(n) if *n as usize > 0 => *n as usize,
        _ => {
            return Err(
                host.err("tensor_maxpool2d(): kernel size must be positive integer".into())
            )
        }
    };
    let stride = if args.len() >= 3 {
        match &args[2] {
            Value::Number(n) => (*n as usize).max(1),
            _ => {
                return Err(
                    host.err("tensor_maxpool2d(): stride must be positive integer".into())
                )
            }
        }
    } else {
        k
    };
    let (n, c, h, w) = (sx[0], sx[1], sx[2], sx[3]);
    if h < k || w < k {
        return Err(host.err("tensor_maxpool2d(): kernel larger than input".into()));
    }
    let oh = (h - k) / stride + 1;
    let ow = (w - k) / stride + 1;
    let mut out = vec![0.0_f64; n * c * oh * ow];
    for batch in 0..n {
        for ch in 0..c {
            for oy in 0..oh {
                for ox in 0..ow {
                    let mut best = f64::NEG_INFINITY;
                    for ky in 0..k {
                        for kx in 0..k {
                            let iy = oy * stride + ky;
                            let ix = ox * stride + kx;
                            let idx = batch * c * h * w + ch * h * w + iy * w + ix;
                            if dx[idx] > best {
                                best = dx[idx];
                            }
                        }
                    }
                    let o_idx = batch * c * oh * ow + ch * oh * ow + oy * ow + ox;
                    out[o_idx] = best;
                }
            }
        }
    }
    Ok(tensor_make(vec![n, c, oh, ow], out))
}

pub(super) fn tensor_avgpool2d_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() < 2 || args.len() > 3 {
        return Err(host.err(format!(
            "tensor_avgpool2d() expects 2 or 3 arguments, got {}",
            args.len()
        )));
    }
    let (sx, dx) = tensor_get_shape_data(host, &args[0], "tensor_avgpool2d")?;
    if sx.len() != 4 {
        return Err(host.err("tensor_avgpool2d(): expected [N,C,H,W] tensor".into()));
    }
    let k = match &args[1] {
        Value::Number(n) if *n as usize > 0 => *n as usize,
        _ => {
            return Err(
                host.err("tensor_avgpool2d(): kernel size must be positive integer".into())
            )
        }
    };
    let stride = if args.len() >= 3 {
        match &args[2] {
            Value::Number(n) => (*n as usize).max(1),
            _ => {
                return Err(
                    host.err("tensor_avgpool2d(): stride must be positive integer".into())
                )
            }
        }
    } else {
        k
    };
    let (n, c, h, w) = (sx[0], sx[1], sx[2], sx[3]);
    if h < k || w < k {
        return Err(host.err("tensor_avgpool2d(): kernel larger than input".into()));
    }
    let oh = (h - k) / stride + 1;
    let ow = (w - k) / stride + 1;
    let mut out = vec![0.0_f64; n * c * oh * ow];
    let denom = (k * k) as f64;
    for batch in 0..n {
        for ch in 0..c {
            for oy in 0..oh {
                for ox in 0..ow {
                    let mut acc = 0.0_f64;
                    for ky in 0..k {
                        for kx in 0..k {
                            let iy = oy * stride + ky;
                            let ix = ox * stride + kx;
                            let idx = batch * c * h * w + ch * h * w + iy * w + ix;
                            acc += dx[idx];
                        }
                    }
                    let o_idx = batch * c * oh * ow + ch * oh * ow + oy * ow + ox;
                    out[o_idx] = acc / denom;
                }
            }
        }
    }
    Ok(tensor_make(vec![n, c, oh, ow], out))
}
