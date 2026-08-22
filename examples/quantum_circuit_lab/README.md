# Quantum Circuit Lab

This PBLang-only example consolidates several fragmented Qiskit, Cirq, and Q#
experiments into a deterministic local state-vector laboratory. It includes:

- a Bell pair with six Pauli-observable expectations;
- a theoretical Hadamard random-bit distribution;
- a square-root-of-X circuit expressed as `H-S-H`; and
- a complete two-qubit Grover iteration that amplifies the `|11>` state.

Run it with:

```text
pblang run examples/quantum_circuit_lab/main.sapp
```

The report deliberately distinguishes local simulation from physical quantum
hardware. It uses no cloud account, API token, package installer, network
request, subprocess, or foreign-language runtime.

The implemented scope is bounded to deterministic one- and two-qubit circuits.
It does not emulate a vendor processor calibration, execute against cloud or
physical quantum hardware, provide a general Grover compiler, or claim that a
small state-vector simulation models hardware noise.

## Migration boundary

The reviewed folder also contains copied third-party tutorial/notebook material.
That content is not copied into this example and is not represented as original
PBLang work. Only general quantum-computing behavior from the small experiments
is reimplemented through the existing PBLang quantum libraries. Source snapshot
digests preserve provenance without retaining workstation paths.
