# Perceptron Logic Gates

This native PBLang example evolves an earlier C perceptron exercise into a bounded and reusable
binary classifier. It uses the new perceptron API in `stdlib/ml.sapp` and has no foreign runtime.

The earlier exercise lost every bias update because bias was passed by value. It also trained a
single perceptron on XOR without identifying that XOR is not linearly separable. This edition keeps
model state explicitly and distinguishes model limitations from successful convergence.

## Run

```sh
pblang run examples/perceptron_logic_gates/main.sapp -- and
pblang run examples/perceptron_logic_gates/main.sapp -- or
pblang run examples/perceptron_logic_gates/main.sapp -- nand
pblang run examples/perceptron_logic_gates/main.sapp -- xor 40
```

AND, OR, and NAND should converge. XOR should remain below perfect accuracy and report that this is
the expected result for a single linear threshold unit.

## Safety and determinism

- training order and initialization are deterministic;
- the caller's input model is copied rather than mutated;
- features, samples, epochs, total training steps, learning rate, and numeric magnitude are bounded;
- malformed dimensions, labels, options, NaN values, and excessive updates fail closed;
- no files, network services, external processes, or personal data are used.
