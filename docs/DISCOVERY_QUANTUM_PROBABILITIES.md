# Small CPU quantum probability example

Run `pblang run examples/discovery_quantum_probabilities.sapp` in a locked PBLang checkout. The complete example uses the existing `quantum_sim.sapp` implementation; it does not add a simulator or vendor backend.

It constructs a Bell pair with H and CNOT and verifies probabilities approximately `[0.5,0,0,0.5]` in the displayed `00,01,10,11` basis order. A one-qubit Hadamard example verifies approximately `[0.5,0.5]`. The original ground state remains unchanged. The existing Bell helper is also compared with the composed gates. It calculates probabilities deterministically, without random measurement shots.

The local example helpers admit only one or two qubits before allocating a state. Probability inputs must contain exactly two or four complex pairs, each with two finite numeric components in [-1,1], and total probability within 1e-9 of one. Rejected cases include larger/fractional/invalid qubit counts, malformed state shapes, zero or non-normalized vectors, NaN/Infinity and nonnumeric amplitudes. These checks belong to this small example; they are not a claim that every existing quantum API validates all external inputs. No arbitrary user-supplied circuit, gate indices, sampling count or unbounded state size is accepted.

This is **CPU state-vector simulation**, not physical quantum hardware, cloud quantum execution, noise modeling or a GPU-accelerated benchmark. Output includes `real_quantum_hardware:false` and `cloud_execution:false`.

For a future Google Colab notebook, after the user separately prepares a compatible PBLang runtime and this source/dependency set, the same command can be used in a notebook shell cell. This delivery tests it locally on Linux only: no Colab session was created, no cloud job or payment was started, and no private source/data was uploaded. Runtime installation and the AI notebook workflow are coordinated separately. Merely placing this command in Colab does not turn the simulator into a quantum backend.

For more already-existing educational circuits, see `examples/quantum_circuit_lab` (Bell observables, square-root X and two-qubit Grover). This new small example focuses on a self-checking probability/bounds contract convenient for a notebook demonstration.
