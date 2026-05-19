//! Smoke tests for the native quantum statevector simulator builtin.

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

fn map_get(v: &Value, key: &str) -> Value {
    match v {
        Value::Map(m) => m
            .borrow()
            .get(key)
            .cloned()
            .unwrap_or_else(|| panic!("expected key {}", key)),
        _ => panic!("expected map, got {:?}", v),
    }
}

#[test]
fn identity_circuit_stays_in_zero() {
    let out = run_collect(
        r#"
        let circuit = map{
            n_qubits: 2,
            ops: []
        };
        print quantum_simulate(circuit);
        "#,
    );
    let probs = nums(&map_get(&out[0], "probs"));
    // |00> should be 1, all others 0
    assert_eq!(probs.len(), 4);
    assert!((probs[0] - 1.0).abs() < 1e-9);
    for &p in probs.iter().skip(1) {
        assert!(p.abs() < 1e-9);
    }
}

#[test]
fn hadamard_creates_superposition() {
    let out = run_collect(
        r#"
        let circuit = map{
            n_qubits: 1,
            ops: [
                map{ gate: "H", qubits: [0] }
            ]
        };
        print quantum_simulate(circuit);
        "#,
    );
    let probs = nums(&map_get(&out[0], "probs"));
    assert_eq!(probs.len(), 2);
    assert!((probs[0] - 0.5).abs() < 1e-9);
    assert!((probs[1] - 0.5).abs() < 1e-9);
}

#[test]
fn bell_state_is_correlated() {
    let out = run_collect(
        r#"
        // |00> -> H on qubit 0 -> CNOT(0,1) -> Bell state
        let circuit = map{
            n_qubits: 2,
            ops: [
                map{ gate: "H", qubits: [0] },
                map{ gate: "CX", qubits: [0, 1] }
            ]
        };
        print quantum_simulate(circuit);
        "#,
    );
    let probs = nums(&map_get(&out[0], "probs"));
    // Bell state: 50% |00>, 50% |11>, 0% |01> or |10>
    assert!((probs[0] - 0.5).abs() < 1e-9, "P(00)={}", probs[0]);
    assert!(probs[1].abs() < 1e-9, "P(01)={}", probs[1]);
    assert!(probs[2].abs() < 1e-9, "P(10)={}", probs[2]);
    assert!((probs[3] - 0.5).abs() < 1e-9, "P(11)={}", probs[3]);
}

#[test]
fn x_gate_flips_qubit() {
    let out = run_collect(
        r#"
        let circuit = map{
            n_qubits: 1,
            ops: [
                map{ gate: "X", qubits: [0] }
            ]
        };
        print quantum_simulate(circuit);
        "#,
    );
    let probs = nums(&map_get(&out[0], "probs"));
    // |0> -> |1>: probs = [0, 1]
    assert!(probs[0].abs() < 1e-9);
    assert!((probs[1] - 1.0).abs() < 1e-9);
}

#[test]
fn rx_pi_equals_x_up_to_phase() {
    // RX(pi) and X both flip |0> -> |1>, just different phase. The probabilities
    // must be identical.
    let out = run_collect(
        r#"
        let circuit = map{
            n_qubits: 1,
            ops: [
                map{ gate: "RX", qubits: [0], theta: 3.141592653589793 }
            ]
        };
        print quantum_simulate(circuit);
        "#,
    );
    let probs = nums(&map_get(&out[0], "probs"));
    assert!(probs[0] < 1e-6);
    assert!((probs[1] - 1.0).abs() < 1e-6);
}

#[test]
fn toffoli_gate_works() {
    let out = run_collect(
        r#"
        // |110> -> CCX(0,1,2) -> |111>
        let circuit = map{
            n_qubits: 3,
            ops: [
                map{ gate: "X", qubits: [0] },
                map{ gate: "X", qubits: [1] },
                map{ gate: "CCX", qubits: [0, 1, 2] }
            ]
        };
        print quantum_simulate(circuit);
        "#,
    );
    let probs = nums(&map_get(&out[0], "probs"));
    // We expect |111> (index 7) = 1, all else 0.
    for (i, &p) in probs.iter().enumerate() {
        if i == 7 {
            assert!((p - 1.0).abs() < 1e-9, "P(|111>) should be 1, got {}", p);
        } else {
            assert!(p.abs() < 1e-9, "P(|{:03b}>) should be 0, got {}", i, p);
        }
    }
}

#[test]
fn probabilities_sum_to_one() {
    let out = run_collect(
        r#"
        let circuit = map{
            n_qubits: 3,
            ops: [
                map{ gate: "H", qubits: [0] },
                map{ gate: "H", qubits: [1] },
                map{ gate: "H", qubits: [2] }
            ]
        };
        print quantum_simulate(circuit);
        "#,
    );
    let probs = nums(&map_get(&out[0], "probs"));
    assert_eq!(probs.len(), 8);
    let s: f64 = probs.iter().sum();
    assert!((s - 1.0).abs() < 1e-9, "sum of probs = {}", s);
    // Should all be 1/8
    for &p in &probs {
        assert!((p - 0.125).abs() < 1e-9, "p={}", p);
    }
}
