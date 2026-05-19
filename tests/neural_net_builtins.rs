//! Smoke tests for the native neural-network tensor builtins.

#[allow(dead_code)]
mod common;

use common::run_collect;
use pblang_vm::Value;

fn nums(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a
            .borrow()
            .iter()
            .map(|x| match x {
                Value::Number(n) => *n,
                _ => panic!("expected number, got {:?}", x),
            })
            .collect(),
        _ => panic!("expected array, got {:?}", v),
    }
}

fn tensor_data(v: &Value) -> Vec<f64> {
    match v {
        Value::Map(m) => nums(&m.borrow().get("data").cloned().expect("tensor data")),
        _ => panic!("expected map, got {:?}", v),
    }
}

fn tensor_shape(v: &Value) -> Vec<usize> {
    match v {
        Value::Map(m) => nums(&m.borrow().get("shape").cloned().expect("tensor shape"))
            .into_iter()
            .map(|n| n as usize)
            .collect(),
        _ => panic!("expected map, got {:?}", v),
    }
}

#[test]
fn gelu_known_values() {
    let out = run_collect(
        r#"
        let a = map{ shape: [3], data: [0.0, 1.0, -1.0], dtype: "f32" };
        print tensor_gelu(a);
        "#,
    );
    let d = tensor_data(&out[0]);
    assert!((d[0] - 0.0).abs() < 1e-6, "gelu(0) ≈ 0, got {}", d[0]);
    // GELU(1) ≈ 0.8412
    assert!(
        (d[1] - 0.8412).abs() < 1e-3,
        "gelu(1) ≈ 0.8412, got {}",
        d[1]
    );
    // GELU(-1) ≈ -0.1588
    assert!(
        (d[2] - (-0.1588)).abs() < 1e-3,
        "gelu(-1) ≈ -0.1588, got {}",
        d[2]
    );
}

#[test]
fn leaky_relu_default_slope() {
    let out = run_collect(
        r#"
        let a = map{ shape: [4], data: [-2.0, -1.0, 0.0, 3.0], dtype: "f32" };
        print tensor_leaky_relu(a);
        "#,
    );
    let d = tensor_data(&out[0]);
    assert!((d[0] - (-0.02)).abs() < 1e-9);
    assert!((d[1] - (-0.01)).abs() < 1e-9);
    assert!((d[2] - 0.0).abs() < 1e-9);
    assert!((d[3] - 3.0).abs() < 1e-9);
}

#[test]
fn leaky_relu_custom_slope() {
    let out = run_collect(
        r#"
        let a = map{ shape: [2], data: [-2.0, 4.0], dtype: "f32" };
        print tensor_leaky_relu(a, 0.5);
        "#,
    );
    let d = tensor_data(&out[0]);
    assert!((d[0] - (-1.0)).abs() < 1e-9);
    assert!((d[1] - 4.0).abs() < 1e-9);
}

#[test]
fn log_softmax_sums_correctly() {
    let out = run_collect(
        r#"
        let a = map{ shape: [3], data: [1.0, 2.0, 3.0], dtype: "f32" };
        let r = tensor_log_softmax(a);
        // exp(log_softmax) should sum to 1
        let exp_sum = 0.0;
        let i = 0;
        let data = r["data"];
        while i < len(data) {
            exp_sum = exp_sum + (data[i] + 0.0); // just collect for compare below
            i = i + 1;
        }
        print r;
        "#,
    );
    let d = tensor_data(&out[0]);
    let sum: f64 = d.iter().map(|x| x.exp()).sum();
    assert!(
        (sum - 1.0).abs() < 1e-9,
        "sum(exp(log_softmax)) should be 1, got {}",
        sum
    );
}

#[test]
fn layer_norm_zero_mean_unit_var() {
    let out = run_collect(
        r#"
        let a = map{ shape: [4], data: [1.0, 2.0, 3.0, 4.0], dtype: "f32" };
        print tensor_layer_norm(a);
        "#,
    );
    let d = tensor_data(&out[0]);
    let m: f64 = d.iter().sum::<f64>() / d.len() as f64;
    assert!(m.abs() < 1e-6, "mean should be ~0, got {}", m);
    let var: f64 = d.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / d.len() as f64;
    assert!((var - 1.0).abs() < 1e-3, "var should be ~1, got {}", var);
}

#[test]
fn batch_norm_per_channel_normalization() {
    // Two-batch, two-channel, 1x1 spatial
    let out = run_collect(
        r#"
        let a = map{ shape: [2, 2, 1, 1], data: [1.0, 10.0, 3.0, 20.0], dtype: "f32" };
        print tensor_batch_norm(a);
        "#,
    );
    let d = tensor_data(&out[0]);
    // For channel 0: values [1,3] -> mean=2, var=1, norm = (-1, 1)
    // For channel 1: values [10,20] -> mean=15, var=25, norm = (-1, 1)
    assert!((d[0] - (-1.0)).abs() < 1e-3, "ch0/b0: {}", d[0]);
    assert!((d[1] - (-1.0)).abs() < 1e-3, "ch1/b0: {}", d[1]);
    assert!((d[2] - 1.0).abs() < 1e-3, "ch0/b1: {}", d[2]);
    assert!((d[3] - 1.0).abs() < 1e-3, "ch1/b1: {}", d[3]);
}

#[test]
fn embedding_lookup_works() {
    let out = run_collect(
        r#"
        // 4 entries, dim 3
        let table = map{
            shape: [4, 3],
            data: [
                1.0, 2.0, 3.0,
                4.0, 5.0, 6.0,
                7.0, 8.0, 9.0,
                10.0, 11.0, 12.0
            ],
            dtype: "f32"
        };
        print tensor_embedding_lookup(table, [2, 0, 3]);
        "#,
    );
    assert_eq!(tensor_shape(&out[0]), vec![3, 3]);
    // [2,0,3] -> rows 2, 0, 3
    let expected = vec![7.0, 8.0, 9.0, 1.0, 2.0, 3.0, 10.0, 11.0, 12.0];
    assert_eq!(tensor_data(&out[0]), expected);
}

#[test]
fn cross_entropy_perfect_prediction() {
    // Confident, correct prediction -> low loss
    let out = run_collect(
        r#"
        // 2-class, 2 samples; targets are class 1 and 0
        let logits = map{
            shape: [2, 2],
            data: [-10.0, 10.0, 10.0, -10.0],
            dtype: "f32"
        };
        print tensor_cross_entropy(logits, [1, 0]);
        "#,
    );
    if let Value::Number(loss) = &out[0] {
        assert!(*loss < 0.01, "expected near-zero loss, got {}", loss);
    } else {
        panic!("expected number");
    }
}

#[test]
fn cross_entropy_worst_prediction() {
    let out = run_collect(
        r#"
        let logits = map{
            shape: [2, 2],
            data: [10.0, -10.0, -10.0, 10.0],
            dtype: "f32"
        };
        // Predicted classes are 0, 1 — labels say 1, 0 (opposite)
        print tensor_cross_entropy(logits, [1, 0]);
        "#,
    );
    if let Value::Number(loss) = &out[0] {
        assert!(*loss > 1.0, "expected large loss, got {}", loss);
    } else {
        panic!("expected number");
    }
}

#[test]
fn conv2d_identity_kernel() {
    // 1x1 image with single channel, identity kernel
    let out = run_collect(
        r#"
        // [N=1, C=1, H=3, W=3], 1 channel
        let x = map{
            shape: [1, 1, 3, 3],
            data: [
                1.0, 2.0, 3.0,
                4.0, 5.0, 6.0,
                7.0, 8.0, 9.0
            ],
            dtype: "f32"
        };
        // 1x1 kernel that is just identity multiply
        let k = map{ shape: [1, 1, 1, 1], data: [1.0], dtype: "f32" };
        print tensor_conv2d(x, k);
        "#,
    );
    assert_eq!(tensor_shape(&out[0]), vec![1, 1, 3, 3]);
    assert_eq!(
        tensor_data(&out[0]),
        vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]
    );
}

#[test]
fn conv2d_3x3_box_filter() {
    // 3x3 input, single 3x3 all-ones kernel, no padding, stride 1
    // Expected output: scalar sum of the input.
    let out = run_collect(
        r#"
        let x = map{
            shape: [1, 1, 3, 3],
            data: [
                1.0, 1.0, 1.0,
                1.0, 1.0, 1.0,
                1.0, 1.0, 1.0
            ],
            dtype: "f32"
        };
        let k = map{
            shape: [1, 1, 3, 3],
            data: [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            dtype: "f32"
        };
        print tensor_conv2d(x, k);
        "#,
    );
    assert_eq!(tensor_shape(&out[0]), vec![1, 1, 1, 1]);
    assert_eq!(tensor_data(&out[0]), vec![9.0]);
}

#[test]
fn maxpool2d_2x2_basic() {
    let out = run_collect(
        r#"
        let x = map{
            shape: [1, 1, 4, 4],
            data: [
                1.0, 2.0, 3.0, 4.0,
                5.0, 6.0, 7.0, 8.0,
                9.0, 10.0, 11.0, 12.0,
                13.0, 14.0, 15.0, 16.0
            ],
            dtype: "f32"
        };
        print tensor_maxpool2d(x, 2);
        "#,
    );
    assert_eq!(tensor_shape(&out[0]), vec![1, 1, 2, 2]);
    // 2x2 maxpool with stride=2 -> max of each 2x2 block.
    assert_eq!(tensor_data(&out[0]), vec![6.0, 8.0, 14.0, 16.0]);
}

#[test]
fn avgpool2d_2x2_basic() {
    let out = run_collect(
        r#"
        let x = map{
            shape: [1, 1, 2, 2],
            data: [1.0, 3.0, 5.0, 7.0],
            dtype: "f32"
        };
        print tensor_avgpool2d(x, 2);
        "#,
    );
    assert_eq!(tensor_shape(&out[0]), vec![1, 1, 1, 1]);
    // Average of [1,3,5,7] = 4
    assert_eq!(tensor_data(&out[0]), vec![4.0]);
}
