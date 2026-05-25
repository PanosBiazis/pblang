//! Quantum-computing builtin implementations.
//!
//! Free functions backing the `quantum_*` builtins registered in
//! `VM::builtins()` (vm.rs):
//!
//!   - `quantum_run(qasm[, backend, shots])` — execute a QASM circuit on
//!     a real backend (qiskit-aer / cirq / braket) via the
//!     `pblang_quantum` crate. Subprocess-based; gated on
//!     `host.allow_exec()`.
//!   - `quantum_statevector(qasm[, backend])` — return the full
//!     statevector. Same gating.
//!   - `quantum_backends()` — JSON array of backend availability flags.
//!   - `quantum_simulate(circuit_map)` — pure-Rust statevector simulator
//!     for circuits expressed as `{ n_qubits, ops: [{gate, qubits, ...}] }`.
//!     Supports I/H/X/Y/Z/S/T, RX/RY/RZ rotations, CX/CZ/SWAP, CCX/CCZ
//!     (Toffoli/Z), and CSWAP (Fredkin). Pure computation — no host caps.
//!
//! Internal helpers `apply_quantum_gate` / `apply_1q_gate` /
//! `apply_controlled` / `apply_pair_1q_inplace` implement the gate
//! application kernel used by `quantum_simulate`.

use std::collections::HashMap;

use pblang_ir::{alloc_string, Obj, RuntimeError, Value, VmHost};

use super::{alloc_array, alloc_map};

/// quantum_run(qasm, backend_str, shots) — Execute a QASM circuit on a quantum backend.
pub(super) fn quantum_run(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_exec() {
        return Err(host.err("exec access denied".into()));
    }
    if args.is_empty() || args.len() > 3 {
        return Err(host.err(format!(
            "quantum_run() expects 1-3 arguments (qasm[, backend, shots]), got {}",
            args.len()
        )));
    }
    let qasm = args[0]
        .as_str()
        .map_err(|m| host.err(format!("quantum_run(): {m}")))?;
    let backend_str = if args.len() >= 2 {
        args[1]
            .as_str()
            .map_err(|m| host.err(format!("quantum_run(): {m}")))?
    } else {
        "qiskit"
    };
    let shots = if args.len() >= 3 {
        match &args[2] {
            Value::Number(n) => *n as u32,
            other => {
                return Err(host.err(format!(
                    "quantum_run(): expected number for shots, got {:?}",
                    other
                )));
            }
        }
    } else {
        1024
    };

    let backend = backend_str
        .parse::<pblang_quantum::QuantumBackend>()
        .map_err(|_| {
            host.err(format!(
                "quantum_run(): unknown backend '{}'. Try: qiskit, cirq, braket",
                backend_str
            ))
        })?;

    match pblang_quantum::run_qasm(qasm, backend, shots) {
        Ok(result) => {
            let json = pblang_quantum::result_to_json(&result);
            Ok(alloc_string(json))
        }
        Err(e) => Err(host.err(format!("quantum_run(): {e}"))),
    }
}

/// quantum_statevector(qasm, backend_str) — Get the statevector of a QASM circuit.
pub(super) fn quantum_statevector(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_exec() {
        return Err(host.err("exec access denied".into()));
    }
    if args.is_empty() || args.len() > 2 {
        return Err(host.err(format!(
            "quantum_statevector() expects 1-2 arguments (qasm[, backend]), got {}",
            args.len()
        )));
    }
    let qasm = args[0]
        .as_str()
        .map_err(|m| host.err(format!("quantum_statevector(): {m}")))?;
    let backend_str = if args.len() >= 2 {
        args[1]
            .as_str()
            .map_err(|m| host.err(format!("quantum_statevector(): {m}")))?
    } else {
        "qiskit"
    };

    let backend = backend_str
        .parse::<pblang_quantum::QuantumBackend>()
        .map_err(|_| {
            host.err(format!(
                "quantum_statevector(): unknown backend '{}'. Try: qiskit, cirq",
                backend_str
            ))
        })?;

    match pblang_quantum::get_statevector(qasm, backend) {
        Ok(sv) => {
            let json = pblang_quantum::statevector_to_json(&sv);
            Ok(alloc_string(json))
        }
        Err(e) => Err(host.err(format!("quantum_statevector(): {e}"))),
    }
}

/// quantum_backends() — List available quantum backends with availability status.
pub(super) fn quantum_backends(
    _host: &mut dyn VmHost,
    _args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let backends = [
        ("qiskit", pblang_quantum::QuantumBackend::QiskitAer),
        ("cirq", pblang_quantum::QuantumBackend::CirqLocal),
        ("braket", pblang_quantum::QuantumBackend::BraketLocal),
    ];
    let mut result = String::from("[");
    for (i, (name, backend)) in backends.iter().enumerate() {
        if i > 0 {
            result.push(',');
        }
        let available = pblang_quantum::is_backend_available(backend.clone());
        result.push_str(&format!(
            "{{\"name\":\"{name}\",\"available\":{available}}}"
        ));
    }
    result.push(']');
    Ok(alloc_string(result))
}

// (real, summing to 1).
//
// Supported gates (case-insensitive): I, H, X, Y, Z, S, T, RX, RY, RZ,
// CX (CNOT), CZ, SWAP, CCX (Toffoli), CCZ, CSWAP (Fredkin).
// Circuit map shape: { "n_qubits": N, "ops": [ { "gate": "H", "qubits": [0] }, ... ] }
pub(super) fn quantum_simulate_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "quantum_simulate() expects 1 argument (circuit), got {}",
            args.len()
        )));
    }
    let circ = match &args[0] {
        Value::Map(m) => m.clone(),
        _ => return Err(host.err("quantum_simulate(): expected a circuit map".into())),
    };
    let (n_qubits, ops_v) = {
        let r = circ.borrow();
        let n = match r.get("n_qubits") {
            Some(Value::Number(n)) if *n >= 0.0 => *n as usize,
            _ => {
                return Err(host
                    .err("quantum_simulate(): circuit missing 'n_qubits' number".into()))
            }
        };
        let ops = r
            .get("ops")
            .cloned()
            .ok_or_else(|| host.err("quantum_simulate(): circuit missing 'ops'".into()))?;
        (n, ops)
    };
    if n_qubits > 20 {
        return Err(host.err(format!(
            "quantum_simulate(): too many qubits ({}); max is 20",
            n_qubits
        )));
    }
    let dim = 1usize << n_qubits;
    // Statevector as (real, imag) pairs.
    let mut state_re = vec![0.0_f64; dim];
    let mut state_im = vec![0.0_f64; dim];
    state_re[0] = 1.0;

    let ops_arr = match ops_v {
        Value::Array(a) => a,
        _ => return Err(host.err("quantum_simulate(): 'ops' must be an array".into())),
    };
    for op_v in ops_arr.borrow().iter() {
        let op_m = match op_v {
            Value::Map(m) => m.clone(),
            _ => return Err(host.err("quantum_simulate(): op entry must be a map".into())),
        };
        let r = op_m.borrow();
        let gate_s = match r.get("gate") {
            Some(Value::Obj(o)) => match o.as_ref() {
                Obj::String(s) => s.to_uppercase(),
            },
            _ => {
                return Err(host.err("quantum_simulate(): op missing 'gate' string".into()))
            }
        };
        let qubits_v = r
            .get("qubits")
            .cloned()
            .ok_or_else(|| host.err("quantum_simulate(): op missing 'qubits'".into()))?;
        let qubits: Vec<usize> = match qubits_v {
            Value::Array(a) => a
                .borrow()
                .iter()
                .map(|v| match v {
                    Value::Number(n) if *n >= 0.0 => Ok(*n as usize),
                    _ => Err(host.err(
                        "quantum_simulate(): qubits must be non-negative integers".into(),
                    )),
                })
                .collect::<Result<_, _>>()?,
            _ => return Err(host.err("quantum_simulate(): qubits must be an array".into())),
        };
        let theta = match r.get("theta") {
            Some(Value::Number(n)) => Some(*n),
            _ => None,
        };
        drop(r);

        for q in &qubits {
            if *q >= n_qubits {
                return Err(host.err(format!(
                    "quantum_simulate(): qubit {} out of range [0,{})",
                    q, n_qubits
                )));
            }
        }

        apply_quantum_gate(
            host,
            &gate_s,
            &qubits,
            theta,
            n_qubits,
            &mut state_re,
            &mut state_im,
        )?;
    }

    // Return probabilities and the raw statevector real/imag parts.
    let probs: Vec<f64> = state_re
        .iter()
        .zip(state_im.iter())
        .map(|(r, i)| r * r + i * i)
        .collect();
    let mut out_map = HashMap::new();
    out_map.insert(
        "probs".into(),
        alloc_array(probs.into_iter().map(Value::Number).collect()),
    );
    out_map.insert(
        "real".into(),
        alloc_array(state_re.into_iter().map(Value::Number).collect()),
    );
    out_map.insert(
        "imag".into(),
        alloc_array(state_im.into_iter().map(Value::Number).collect()),
    );
    out_map.insert("n_qubits".into(), Value::Number(n_qubits as f64));
    Ok(alloc_map(out_map))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_quantum_gate(
    host: &mut dyn VmHost,
    gate: &str,
    qubits: &[usize],
    theta: Option<f64>,
    n: usize,
    re: &mut [f64],
    im: &mut [f64],
) -> Result<(), RuntimeError> {
    let dim = 1usize << n;
    match gate {
        "I" => Ok(()),
        "X" | "Y" | "Z" | "H" | "S" | "T" => {
            if qubits.len() != 1 {
                return Err(host.err(format!("{gate}: expected 1 qubit")));
            }
            let q = qubits[0];
            apply_1q_gate(gate, q, n, re, im, None);
            Ok(())
        }
        "RX" | "RY" | "RZ" => {
            if qubits.len() != 1 {
                return Err(host.err(format!("{gate}: expected 1 qubit")));
            }
            let q = qubits[0];
            let t = theta
                .ok_or_else(|| host.err(format!("{gate}: missing 'theta' parameter")))?;
            apply_1q_gate(gate, q, n, re, im, Some(t));
            Ok(())
        }
        "CX" | "CNOT" => {
            if qubits.len() != 2 {
                return Err(host.err("CX: expected [control, target]".into()));
            }
            let (c, t) = (qubits[0], qubits[1]);
            apply_controlled(c, &[t], "X", n, re, im, None);
            Ok(())
        }
        "CZ" => {
            if qubits.len() != 2 {
                return Err(host.err("CZ: expected [control, target]".into()));
            }
            apply_controlled(qubits[0], &[qubits[1]], "Z", n, re, im, None);
            Ok(())
        }
        "SWAP" => {
            if qubits.len() != 2 {
                return Err(host.err("SWAP: expected 2 qubits".into()));
            }
            let (a, b) = (qubits[0], qubits[1]);
            for i in 0..dim {
                let ba = (i >> a) & 1;
                let bb = (i >> b) & 1;
                if ba == bb {
                    continue;
                }
                let j = i ^ (1 << a) ^ (1 << b);
                if i < j {
                    re.swap(i, j);
                    im.swap(i, j);
                }
            }
            Ok(())
        }
        "CCX" | "TOFFOLI" => {
            if qubits.len() != 3 {
                return Err(host.err("CCX: expected 3 qubits".into()));
            }
            let (c1, c2, t) = (qubits[0], qubits[1], qubits[2]);
            for i in 0..dim {
                let b1 = (i >> c1) & 1;
                let b2 = (i >> c2) & 1;
                if b1 == 1 && b2 == 1 {
                    let j = i ^ (1 << t);
                    if i < j {
                        re.swap(i, j);
                        im.swap(i, j);
                    }
                }
            }
            Ok(())
        }
        "CCZ" => {
            if qubits.len() != 3 {
                return Err(host.err("CCZ: expected 3 qubits".into()));
            }
            let (a, b, c) = (qubits[0], qubits[1], qubits[2]);
            for i in 0..dim {
                if ((i >> a) & 1) == 1 && ((i >> b) & 1) == 1 && ((i >> c) & 1) == 1 {
                    re[i] = -re[i];
                    im[i] = -im[i];
                }
            }
            Ok(())
        }
        "CSWAP" | "FREDKIN" => {
            if qubits.len() != 3 {
                return Err(host.err("CSWAP: expected 3 qubits".into()));
            }
            let (c, a, b) = (qubits[0], qubits[1], qubits[2]);
            for i in 0..dim {
                if ((i >> c) & 1) != 1 {
                    continue;
                }
                let ba = (i >> a) & 1;
                let bb = (i >> b) & 1;
                if ba == bb {
                    continue;
                }
                let j = i ^ (1 << a) ^ (1 << b);
                if i < j {
                    re.swap(i, j);
                    im.swap(i, j);
                }
            }
            Ok(())
        }
        other => Err(host.err(format!("quantum_simulate(): unknown gate '{other}'"))),
    }
}

pub(super) fn apply_1q_gate(
    gate: &str,
    q: usize,
    n: usize,
    re: &mut [f64],
    im: &mut [f64],
    theta: Option<f64>,
) {
    let dim = 1usize << n;
    // 2x2 matrix as ((m00r,m00i), (m01r,m01i), (m10r,m10i), (m11r,m11i))
    let inv_sqrt2 = 1.0 / 2f64.sqrt();
    let ((a_r, a_i), (b_r, b_i), (c_r, c_i), (d_r, d_i)) = match gate {
        "X" => ((0.0, 0.0), (1.0, 0.0), (1.0, 0.0), (0.0, 0.0)),
        "Y" => ((0.0, 0.0), (0.0, -1.0), (0.0, 1.0), (0.0, 0.0)),
        "Z" => ((1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (-1.0, 0.0)),
        "H" => (
            (inv_sqrt2, 0.0),
            (inv_sqrt2, 0.0),
            (inv_sqrt2, 0.0),
            (-inv_sqrt2, 0.0),
        ),
        "S" => ((1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 1.0)),
        "T" => ((1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (inv_sqrt2, inv_sqrt2)),
        "RX" => {
            let t = theta.unwrap_or(0.0);
            let cs = (t / 2.0).cos();
            let sn = (t / 2.0).sin();
            ((cs, 0.0), (0.0, -sn), (0.0, -sn), (cs, 0.0))
        }
        "RY" => {
            let t = theta.unwrap_or(0.0);
            let cs = (t / 2.0).cos();
            let sn = (t / 2.0).sin();
            ((cs, 0.0), (-sn, 0.0), (sn, 0.0), (cs, 0.0))
        }
        "RZ" => {
            let t = theta.unwrap_or(0.0);
            let cs = (t / 2.0).cos();
            let sn = (t / 2.0).sin();
            ((cs, -sn), (0.0, 0.0), (0.0, 0.0), (cs, sn))
        }
        _ => return,
    };
    for i in 0..dim {
        if (i >> q) & 1 != 0 {
            continue;
        }
        let j = i | (1 << q);
        // [i, j] = M * [old_i, old_j]
        let (oi_r, oi_i) = (re[i], im[i]);
        let (oj_r, oj_i) = (re[j], im[j]);
        re[i] = a_r * oi_r - a_i * oi_i + b_r * oj_r - b_i * oj_i;
        im[i] = a_r * oi_i + a_i * oi_r + b_r * oj_i + b_i * oj_r;
        re[j] = c_r * oi_r - c_i * oi_i + d_r * oj_r - d_i * oj_i;
        im[j] = c_r * oi_i + c_i * oi_r + d_r * oj_i + d_i * oj_r;
    }
}

pub(super) fn apply_controlled(
    ctrl: usize,
    targets: &[usize],
    sub_gate: &str,
    n: usize,
    re: &mut [f64],
    im: &mut [f64],
    theta: Option<f64>,
) {
    let dim = 1usize << n;
    // Only act when ctrl bit is set.
    for i in 0..dim {
        if (i >> ctrl) & 1 != 1 {
            continue;
        }
        // For 1-qubit sub-gates with a single target.
        if targets.len() == 1 && (i >> targets[0]) & 1 == 0 {
            // Pair: (i, i | 1<<target)
            let j = i | (1 << targets[0]);
            if (j >> ctrl) & 1 != 1 {
                continue;
            }
            // Apply 1q matrix to (i, j).
            apply_pair_1q_inplace(sub_gate, i, j, re, im, theta);
        }
    }
}

pub(super) fn apply_pair_1q_inplace(
    gate: &str,
    i: usize,
    j: usize,
    re: &mut [f64],
    im: &mut [f64],
    theta: Option<f64>,
) {
    let inv_sqrt2 = 1.0 / 2f64.sqrt();
    let ((a_r, a_i), (b_r, b_i), (c_r, c_i), (d_r, d_i)) = match gate {
        "X" => ((0.0, 0.0), (1.0, 0.0), (1.0, 0.0), (0.0, 0.0)),
        "Y" => ((0.0, 0.0), (0.0, -1.0), (0.0, 1.0), (0.0, 0.0)),
        "Z" => ((1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (-1.0, 0.0)),
        "H" => (
            (inv_sqrt2, 0.0),
            (inv_sqrt2, 0.0),
            (inv_sqrt2, 0.0),
            (-inv_sqrt2, 0.0),
        ),
        "RX" => {
            let t = theta.unwrap_or(0.0);
            let cs = (t / 2.0).cos();
            let sn = (t / 2.0).sin();
            ((cs, 0.0), (0.0, -sn), (0.0, -sn), (cs, 0.0))
        }
        "RY" => {
            let t = theta.unwrap_or(0.0);
            let cs = (t / 2.0).cos();
            let sn = (t / 2.0).sin();
            ((cs, 0.0), (-sn, 0.0), (sn, 0.0), (cs, 0.0))
        }
        "RZ" => {
            let t = theta.unwrap_or(0.0);
            let cs = (t / 2.0).cos();
            let sn = (t / 2.0).sin();
            ((cs, -sn), (0.0, 0.0), (0.0, 0.0), (cs, sn))
        }
        _ => return,
    };
    let (oi_r, oi_i) = (re[i], im[i]);
    let (oj_r, oj_i) = (re[j], im[j]);
    re[i] = a_r * oi_r - a_i * oi_i + b_r * oj_r - b_i * oj_i;
    im[i] = a_r * oi_i + a_i * oi_r + b_r * oj_i + b_i * oj_r;
    re[j] = c_r * oi_r - c_i * oi_i + d_r * oj_r - d_i * oj_i;
    im[j] = c_r * oi_i + c_i * oi_r + d_r * oj_i + d_i * oj_r;
}
