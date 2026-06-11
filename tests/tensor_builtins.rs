//! Smoke tests for native tensor builtins added in 0.5.

#[allow(dead_code)]
mod common;

use common::{run_collect, run_err};
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
        Value::Map(m) => {
            let data = m
                .borrow()
                .get("data")
                .cloned()
                .expect("tensor map must have data");
            nums(&data)
        }
        _ => panic!("expected map, got {:?}", v),
    }
}

#[test]
fn tensor_add_native() {
    let out = run_collect(
        r#"
        let a = map{ shape: [3], data: [1.0, 2.0, 3.0], dtype: "f32" };
        let b = map{ shape: [3], data: [10.0, 20.0, 30.0], dtype: "f32" };
        print tensor_add(a, b);
        "#,
    );
    assert_eq!(out.len(), 1);
    assert_eq!(tensor_data(&out[0]), vec![11.0, 22.0, 33.0]);
}

#[test]
fn tensor_dot_native() {
    let out = run_collect(
        r#"
        let a = map{ shape: [3], data: [1.0, 2.0, 3.0], dtype: "f32" };
        let b = map{ shape: [3], data: [4.0, 5.0, 6.0], dtype: "f32" };
        print tensor_dot(a, b);
        "#,
    );
    assert_eq!(out.len(), 1);
    if let Value::Number(n) = &out[0] {
        assert!((n - 32.0).abs() < 1e-9, "expected 32, got {n}");
    } else {
        panic!("expected number, got {:?}", out[0]);
    }
}

#[test]
fn tensor_matmul_native() {
    let out = run_collect(
        r#"
        let a = map{ shape: [2, 3], data: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0], dtype: "f32" };
        let b = map{ shape: [3, 2], data: [7.0, 8.0, 9.0, 10.0, 11.0, 12.0], dtype: "f32" };
        print tensor_matmul(a, b);
        "#,
    );
    assert_eq!(out.len(), 1);
    // [[1*7+2*9+3*11, 1*8+2*10+3*12], [4*7+5*9+6*11, 4*8+5*10+6*12]]
    // = [[58, 64], [139, 154]]
    assert_eq!(tensor_data(&out[0]), vec![58.0, 64.0, 139.0, 154.0]);
}

#[test]
fn tensor_relu_native() {
    let out = run_collect(
        r#"
        let a = map{ shape: [4], data: [-1.0, 0.0, 2.0, -3.0], dtype: "f32" };
        print tensor_relu(a);
        "#,
    );
    assert_eq!(tensor_data(&out[0]), vec![0.0, 0.0, 2.0, 0.0]);
}

#[test]
fn tensor_softmax_sums_to_one() {
    let out = run_collect(
        r#"
        let a = map{ shape: [3], data: [1.0, 2.0, 3.0], dtype: "f32" };
        let s = tensor_softmax(a);
        print tensor_sum(s);
        "#,
    );
    assert_eq!(out.len(), 1);
    if let Value::Number(n) = &out[0] {
        assert!((n - 1.0).abs() < 1e-9, "softmax should sum to 1, got {n}");
    } else {
        panic!("expected number, got {:?}", out[0]);
    }
}

#[test]
fn tensor_argmax_works() {
    let out = run_collect(
        r#"
        let a = map{ shape: [4], data: [3.0, 1.0, 4.0, 2.0], dtype: "f32" };
        print tensor_argmax(a);
        "#,
    );
    if let Value::Number(n) = &out[0] {
        assert_eq!(*n as i64, 2);
    } else {
        panic!("expected number");
    }
}

#[test]
fn tensor_transpose2d_native() {
    let out = run_collect(
        r#"
        let a = map{ shape: [2, 3], data: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0], dtype: "f32" };
        print tensor_transpose2d(a);
        "#,
    );
    // [[1,2,3],[4,5,6]] -> [[1,4],[2,5],[3,6]]
    assert_eq!(tensor_data(&out[0]), vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);
}

#[test]
fn tensor_scale_native() {
    let out = run_collect(
        r#"
        let a = map{ shape: [3], data: [1.0, 2.0, 3.0], dtype: "f32" };
        print tensor_scale(a, 2.5);
        "#,
    );
    assert_eq!(tensor_data(&out[0]), vec![2.5, 5.0, 7.5]);
}

#[test]
fn tensor_rejects_huge_shape_before_allocation() {
    let err = run_err(
        r#"
        let a = map{ shape: [1000000000000, 1000000000000], data: [], dtype: "f32" };
        print tensor_transpose2d(a);
        "#,
    );
    assert!(
        err.message.contains("overflows") || err.message.contains("maximum"),
        "unexpected error: {}",
        err.message
    );
}
