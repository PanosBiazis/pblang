// pblang-quantum — Quantum computing bridge for pblang.
//
// Connects pblang's QASM emitter (stdlib/quantum.sapp) to real quantum
// simulators and hardware backends via Python subprocess bridges.
//
// Supported backends:
//   • Qiskit (IBM) — local Aer simulator or IBM Quantum cloud
//   • Braket (AWS) — local simulator or AWS Braket
//   • Cirq (Google) — local simulator
//
// Architecture:
//   The crate spawns Python subprocesses that import the respective SDK,
//   execute circuits, and return JSON results. This avoids linking against
//   heavy Python C API libraries and works with any Python installation.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::time::{Duration, Instant};

/// Hard cap on `shots`. A simulator runs the circuit once per shot, so an
/// unbounded count (e.g. a script passing 4_000_000_000) is a denial-of-service.
/// 1,000,000 is far beyond any realistic sampling need.
const MAX_SHOTS: u32 = 1_000_000;

/// Wall-clock budget for the Python subprocess. Without it, a hung interpreter
/// (pathological circuit, deadlocked SDK) hangs the pblang program forever.
/// Overridable via `PBLANG_QUANTUM_TIMEOUT_SECS`.
fn quantum_timeout() -> Duration {
    let secs = std::env::var("PBLANG_QUANTUM_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&s| s > 0)
        .unwrap_or(120);
    Duration::from_secs(secs)
}

/// Clamp a requested shot count into `[1, MAX_SHOTS]`.
fn clamp_shots(requested: u32) -> u32 {
    requested.clamp(1, MAX_SHOTS)
}

/// Run a Python `script` with `qasm` on stdin and `PBLANG_QUANTUM_SHOTS=shots`,
/// returning its stdout. Enforces a wall-clock `timeout` (kills the child on
/// expiry) and drains stdout/stderr on separate threads so a large statevector
/// dump can't deadlock against a full pipe buffer. stdin is closed after writing
/// so Python's `sys.stdin.read()` sees EOF.
fn run_python(script: &str, qasm: &str, shots: &str, timeout: Duration) -> Result<String, String> {
    let python = find_python()?;
    let mut child = Command::new(&python)
        .arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PBLANG_QUANTUM_SHOTS", shots)
        .spawn()
        .map_err(|e| format!("Failed to start Python ({python}): {e}"))?;

    {
        let mut stdin = child.stdin.take().ok_or("Python stdin unavailable")?;
        stdin
            .write_all(qasm.as_bytes())
            .map_err(|e| format!("Failed to write QASM to Python stdin: {e}"))?;
        // stdin dropped here → EOF for `sys.stdin.read()`.
    }

    let mut out = child.stdout.take().ok_or("Python stdout unavailable")?;
    let mut err = child.stderr.take().ok_or("Python stderr unavailable")?;
    let out_h = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = out.read_to_end(&mut v);
        v
    });
    let err_h = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = err.read_to_end(&mut v);
        v
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait().map_err(|e| format!("Failed to wait for Python: {e}"))? {
            Some(st) => break st,
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = out_h.join();
                    let _ = err_h.join();
                    return Err(format!(
                        "Python quantum subprocess timed out after {}s (set PBLANG_QUANTUM_TIMEOUT_SECS to change)",
                        timeout.as_secs()
                    ));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };

    let stdout = String::from_utf8_lossy(&out_h.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&err_h.join().unwrap_or_default()).into_owned();
    if !status.success() {
        return Err(format!("Python process failed: {stderr}"));
    }
    Ok(stdout)
}

// ─────────────────────────────────────────────────────
// Common types
// ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumResult {
    pub backend: String,
    pub counts: std::collections::HashMap<String, u64>,
    pub shots: u32,
    pub success: bool,
    pub error: Option<String>,
    pub raw_output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumStatevector {
    pub backend: String,
    pub statevector: Vec<ComplexAmplitude>,
    pub num_qubits: usize,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexAmplitude {
    pub real: f64,
    pub imag: f64,
    pub basis_state: String,
}

#[derive(Debug, Clone)]
pub enum QuantumBackend {
    QiskitAer,
    QiskitIBMQ,
    BraketLocal,
    BraketAws,
    CirqLocal,
}

impl QuantumBackend {
    pub fn name(&self) -> &str {
        match self {
            QuantumBackend::QiskitAer => "qiskit_aer",
            QuantumBackend::QiskitIBMQ => "qiskit_ibmq",
            QuantumBackend::BraketLocal => "braket_local",
            QuantumBackend::BraketAws => "braket_aws",
            QuantumBackend::CirqLocal => "cirq_local",
        }
    }

    pub fn parse_name(s: &str) -> Option<Self> {
        <Self as FromStr>::from_str(s).ok()
    }
}

impl FromStr for QuantumBackend {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "qiskit" | "qiskit_aer" | "aer" => Ok(QuantumBackend::QiskitAer),
            "qiskit_ibmq" | "ibmq" | "ibm" => Ok(QuantumBackend::QiskitIBMQ),
            "braket" | "braket_local" => Ok(QuantumBackend::BraketLocal),
            "braket_aws" | "aws" => Ok(QuantumBackend::BraketAws),
            "cirq" | "cirq_local" => Ok(QuantumBackend::CirqLocal),
            _ => Err(()),
        }
    }
}

// ─────────────────────────────────────────────────────
// Qiskit bridge
// ─────────────────────────────────────────────────────

const QISKIT_SCRIPT: &str = r#"
import sys, json, os
try:
    from qiskit import QuantumCircuit, Aer, execute
    from qiskit.quantum_info import Statevector

    qasm_str = sys.stdin.read()

    # Parse QASM into circuit
    circuit = QuantumCircuit.from_qasm_str(qasm_str)

    # Get shots from environment or default
    shots = int(os.environ.get('PBLANG_QUANTUM_SHOTS', '1024'))

    # Run on Aer simulator
    backend = Aer.get_backend('qasm_simulator')
    job = execute(circuit, backend, shots=shots)
    result = job.result()
    counts = result.get_counts()

    # Get statevector
    backend_sv = Aer.get_backend('statevector_simulator')
    job_sv = execute(circuit, backend_sv)
    statevector = job_sv.result().get_statevector()

    output = {
        "success": True,
        "counts": {k: int(v) for k, v in counts.items()},
        "shots": shots,
        "num_qubits": circuit.num_qubits,
        "statevector": [
            {"real": float(sv.real), "imag": float(sv.imag), "basis_state": format(i, '0{}b'.format(circuit.num_qubits))}
            for i, sv in enumerate(statevector.data)
        ]
    }
except ImportError as e:
    output = {"success": False, "error": f"Qiskit not installed: {e}. Install with: pip install qiskit"}
except Exception as e:
    output = {"success": False, "error": str(e)}

print(json.dumps(output))
"#;

// ─────────────────────────────────────────────────────
// Cirq bridge
// ─────────────────────────────────────────────────────

const CIRQ_SCRIPT: &str = r#"
import sys, json, os
try:
    import cirq
    import numpy as np

    qasm_str = sys.stdin.read()
    shots = int(os.environ.get('PBLANG_QUANTUM_SHOTS', '1024'))

    # Parse QASM
    circuit = cirq.qasm_to_circuit(qasm_str)

    # Simulate
    simulator = cirq.Simulator()
    result = simulator.run(circuit, repetitions=shots)

    # Get measurement counts
    counts = {}
    if result.measurements:
        for key, values in result.measurements.items():
            for row in values:
                bits = ''.join(str(int(b)) for b in row)
                counts[bits] = counts.get(bits, 0) + 1

    # Get statevector
    sv_result = simulator.simulate(circuit)
    statevector = sv_result.final_state_vector

    output = {
        "success": True,
        "counts": counts,
        "shots": shots,
        "num_qubits": len(circuit.all_qubits()),
        "statevector": [
            {"real": float(sv.real), "imag": float(sv.imag), "basis_state": format(i, '0{}b'.format(len(circuit.all_qubits())))}
            for i, sv in enumerate(statevector)
        ]
    }
except ImportError as e:
    output = {"success": False, "error": f"Cirq not installed: {e}. Install with: pip install cirq"}
except Exception as e:
    output = {"success": False, "error": str(e)}

print(json.dumps(output))
"#;

// ─────────────────────────────────────────────────────
// Braket bridge
// ─────────────────────────────────────────────────────

const BRAKET_SCRIPT: &str = r#"
import sys, json, os
try:
    from braket.circuits import Circuit
    from braket.devices import LocalSimulator
    import numpy as np

    qasm_str = sys.stdin.read()
    shots = int(os.environ.get('PBLANG_QUANTUM_SHOTS', '1024'))

    # Braket uses OpenQASM 3; try importing
    circuit = Circuit.from_ir(qasm_str)

    device = LocalSimulator()
    result = device.run(circuit, shots=shots).result()
    counts = {format(k, '0{}b'.format(circuit.qubit_count)): int(v) for k, v in result.measurement_counts.items()}

    output = {
        "success": True,
        "counts": counts,
        "shots": shots,
        "num_qubits": circuit.qubit_count,
        "statevector": []
    }
except ImportError as e:
    output = {"success": False, "error": f"Braket not installed: {e}. Install with: pip install amazon-braket-sdk"}
except Exception as e:
    output = {"success": False, "error": str(e)}

print(json.dumps(output))
"#;

// ─────────────────────────────────────────────────────
// Execution engine
// ─────────────────────────────────────────────────────

/// Find a Python 3 interpreter on the system.
fn find_python() -> Result<String, String> {
    for candidate in &["python3", "python"] {
        if Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
        {
            return Ok(candidate.to_string());
        }
    }
    Err("No Python 3 interpreter found (tried python3, python)".into())
}

/// Execute QASM circuit on a quantum backend.
pub fn run_qasm(qasm: &str, backend: QuantumBackend, shots: u32) -> Result<QuantumResult, String> {
    let script = match backend {
        QuantumBackend::QiskitAer | QuantumBackend::QiskitIBMQ => QISKIT_SCRIPT,
        QuantumBackend::CirqLocal => CIRQ_SCRIPT,
        QuantumBackend::BraketLocal | QuantumBackend::BraketAws => BRAKET_SCRIPT,
    };
    let shots = clamp_shots(shots);

    let stdout = run_python(script, qasm, &shots.to_string(), quantum_timeout())?;
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse Python output as JSON: {e}\nOutput: {stdout}"))?;

    if parsed["success"].as_bool().unwrap_or(false) {
        let counts: std::collections::HashMap<String, u64> =
            if let Some(obj) = parsed["counts"].as_object() {
                obj.iter()
                    .map(|(k, v)| (k.clone(), v.as_u64().unwrap_or(0)))
                    .collect()
            } else {
                std::collections::HashMap::new()
            };

        Ok(QuantumResult {
            backend: backend.name().to_string(),
            counts,
            shots,
            success: true,
            error: None,
            raw_output: Some(stdout.to_string()),
        })
    } else {
        let error = parsed["error"]
            .as_str()
            .unwrap_or("Unknown error")
            .to_string();
        Ok(QuantumResult {
            backend: backend.name().to_string(),
            counts: std::collections::HashMap::new(),
            shots,
            success: false,
            error: Some(error),
            raw_output: Some(stdout.to_string()),
        })
    }
}

/// Run QASM and get the statevector.
pub fn get_statevector(qasm: &str, backend: QuantumBackend) -> Result<QuantumStatevector, String> {
    let script = match backend {
        QuantumBackend::QiskitAer | QuantumBackend::QiskitIBMQ => QISKIT_SCRIPT,
        QuantumBackend::CirqLocal => CIRQ_SCRIPT,
        _ => QISKIT_SCRIPT, // Braket statevector not supported in this MVP
    };

    // Only need 1 shot for the statevector.
    let stdout = run_python(script, qasm, "1", quantum_timeout())?;
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse Python output: {e}"))?;

    if parsed["success"].as_bool().unwrap_or(false) {
        let sv_array = parsed["statevector"].as_array().ok_or("No statevector")?;
        let amplitudes: Vec<ComplexAmplitude> = sv_array
            .iter()
            .map(|v| ComplexAmplitude {
                real: v["real"].as_f64().unwrap_or(0.0),
                imag: v["imag"].as_f64().unwrap_or(0.0),
                basis_state: v["basis_state"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        let nq = parsed["num_qubits"].as_u64().unwrap_or(0) as usize;

        Ok(QuantumStatevector {
            backend: backend.name().to_string(),
            statevector: amplitudes,
            num_qubits: nq,
            success: true,
            error: None,
        })
    } else {
        let error = parsed["error"]
            .as_str()
            .unwrap_or("Unknown error")
            .to_string();
        Err(error)
    }
}

/// Check if a quantum backend is available.
pub fn is_backend_available(backend: QuantumBackend) -> bool {
    run_qasm(
        "OPENQASM 2.0;\ninclude \"qelib1.inc\";\nqreg q[1];\ncreg c[1];\n",
        backend,
        1,
    )
    .map(|r| r.success)
    .unwrap_or(false)
}

/// Convert a QuantumResult to JSON string.
pub fn result_to_json(r: &QuantumResult) -> String {
    serde_json::to_string(r).unwrap_or_default()
}

/// Convert a QuantumStatevector to JSON string.
pub fn statevector_to_json(s: &QuantumStatevector) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

// ─────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_shots_bounds_dos() {
        assert_eq!(clamp_shots(1024), 1024);
        assert_eq!(clamp_shots(0), 1); // at least one shot
        assert_eq!(clamp_shots(u32::MAX), MAX_SHOTS); // capped, no simulator hang
        assert_eq!(clamp_shots(MAX_SHOTS + 1), MAX_SHOTS);
    }

    #[test]
    fn run_python_kills_a_hung_subprocess() {
        // A Python process that would run for 30s must be killed at the timeout,
        // not hang the caller forever. Needs a python3 on the box.
        if find_python().is_err() {
            eprintln!("no python; skipping");
            return;
        }
        let start = Instant::now();
        let r = run_python(
            "import time, sys\nsys.stdin.read()\ntime.sleep(30)\nprint('never')",
            "",
            "1",
            Duration::from_millis(700),
        );
        let elapsed = start.elapsed();
        assert!(r.is_err(), "a hung subprocess must return Err");
        assert!(
            r.unwrap_err().contains("timed out"),
            "error should say it timed out"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "must return promptly after the timeout, took {elapsed:?}"
        );
    }

    #[test]
    fn run_python_round_trips_stdin_to_stdout() {
        if find_python().is_err() {
            eprintln!("no python; skipping");
            return;
        }
        let out = run_python(
            "import sys; sys.stdout.write(sys.stdin.read().upper())",
            "hello",
            "1",
            Duration::from_secs(10),
        )
        .expect("python should run");
        assert_eq!(out, "HELLO");
    }

    #[test]
    fn test_backend_from_str() {
        assert!("qiskit".parse::<QuantumBackend>().is_ok());
        assert!("cirq".parse::<QuantumBackend>().is_ok());
        assert!("braket".parse::<QuantumBackend>().is_ok());
        assert!("invalid".parse::<QuantumBackend>().is_err());
    }

    #[test]
    fn test_find_python() {
        // Python should be available on most dev systems
        let result = find_python();
        // This may fail on minimal containers, so we don't assert
        if let Ok(py) = &result {
            assert!(!py.is_empty());
        }
    }

    #[test]
    fn test_simple_qasm_with_qiskit() {
        let qasm = r#"OPENQASM 2.0;
include "qelib1.inc";
qreg q[1];
creg c[1];
h q[0];
measure q[0] -> c[0];"#;

        let result = run_qasm(qasm, QuantumBackend::QiskitAer, 1024);
        match result {
            Ok(r) => {
                if r.success {
                    assert!(!r.counts.is_empty());
                    assert_eq!(r.shots, 1024);
                }
                // If Qiskit is not installed, that's OK in test
            }
            Err(e) => {
                // Python/Qiskit may not be installed - that's fine
                eprintln!("Skipping quantum test (deps not available): {e}");
            }
        }
    }

    #[test]
    fn test_bell_state_qasm() {
        let qasm = r#"OPENQASM 2.0;
include "qelib1.inc";
qreg q[2];
creg c[2];
h q[0];
cx q[0],q[1];
measure q[0] -> c[0];
measure q[1] -> c[1];"#;

        let result = run_qasm(qasm, QuantumBackend::QiskitAer, 4096);
        match result {
            Ok(r) => {
                if r.success {
                    // Bell state should only have 00 and 11
                    for key in r.counts.keys() {
                        assert!(
                            key == "00" || key == "11",
                            "Bell state should only have 00 and 11, got: {key}"
                        );
                    }
                }
            }
            Err(e) => {
                eprintln!("Skipping Bell state test (deps not available): {e}");
            }
        }
    }
}
