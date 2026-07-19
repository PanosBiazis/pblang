# Quantum Provider Slots Design Note

## Status: design contract (Q2-delta)

## Goal

Allow pblang quantum code to target different backends (simulators, cloud QPUs) by swapping a provider map — no VM changes required.

## Provider Contract

A provider is a pblang map with these keys:

| Key       | Type     | Description                                    |
|----------|----------|------------------------------------------------|
| `name`   | string   | Human-readable name (e.g. "qiskit-aer", "ibm-quantum") |
| `submit` | fn(circuit) → job_id | Submits a circuit, returns opaque job ID   |
| `poll`   | fn(job_id) → result_map | Polls for job completion, returns result |

## Built-in Providers (slots)

### Local simulator (default)

Uses `stdlib/quantum_sim.sapp` directly. No network required.

```pblang
let sim_provider = map{
    "name": "local-sim",
    "submit": fn(circ) { return quantum_protocol.submit(circ); },
    "poll":   fn(job_id) { return quantum_protocol.poll(job_id); }
};
```

### Remote HTTP provider

Uses `quantum_protocol.submit_remote(url, circ)` and `quantum_protocol.poll_remote(url, job_id)`.

```pblang
let remote_provider = fn(base_url) {
    return map{
        "name": "remote-http",
        "submit": fn(circ) { return quantum_protocol.submit_remote(base_url, circ); },
        "poll": fn(job_id) { return quantum_protocol.poll_remote(base_url, job_id); }
    };
};
```

## Later provider contracts

- **Qiskit-Aer**: wrap `proc_spawn_wait("python3", ["-c", qiskit_script])`
- **IBM Quantum**: HTTP to https://api.quantum-computing.ibm.com/
- **Braket**: HTTP to AWS Braket endpoint

## Qiskit/Braket/Cirq bridge (Q3-alpha design note)

Two integration paths, no VM changes:

1. **Exec + JSON**: Host or script spawns Python/Node process that runs Qiskit/Braket/Cirq; circuit sent as JSON (same shape as `quantum.sapp` circuit: `{ n, ops }`); process returns JSON `{ measurements: [...] }`. Use `process.spawn_wait` or a small local HTTP bridge that forwards to a Python server.
2. **FFI**: Thin native crate (e.g. `pblang-qiskit`) exposes C ABI: `pblang_qiskit_run(circuit_json_cstr)` → result JSON string. Script does `ffi.load("pblang_qiskit.so")` and `ffi_call("pblang_qiskit_run", circuit_json)`. Requires FfiPolicy allowlist.

Provider slot contract remains: `submit(circuit)` → job_id, `poll(job_id)` → result. Bridge implementation can wrap exec or FFI behind that interface.

## Measurement utilities (Q3-beta, Q3-gamma)

On top of raw measurement arrays (0/1 per qubit, or integer outcomes), `stdlib:quantum_sim.sapp` exposes small helpers:

- `histogram(outcomes_array)` → map from stringified outcomes (e.g. `"0"`, `"1"`, `"2"`) to counts.
- `repetition_encode(bit)` / `repetition_decode(bits)` → simple 3-qubit repetition code helpers for toy error-correction: encode logical bit as `[b,b,b]`, decode via majority vote to correct a single physical bit flip.

Example:

```pblang
let sim = import("stdlib:quantum_sim.sapp");
let results = [0, 0, 1, 1, 1];
let h = sim.histogram(results);
print h["0"];  // 2
print h["1"];  // 3
```

These utilities are intended as building blocks for higher-level analysis (e.g. histograms over many shots from `quantum_protocol.submit` or remote providers) without baking any specific SDK into the VM.

## Non-goals

- No hard-coded provider credentials in VM or stdlib
- No provider SDK bundled with pblang
- No breaking changes to existing `quantum_protocol.submit` / `poll`
