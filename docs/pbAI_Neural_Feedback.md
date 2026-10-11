# Bounded neural reconstruction and feedback

`stdlib/pbai_feedback.sapp` adds a pure PBLang inference path. In standalone
pbAI use `import("stdlib/pbai_feedback.sapp")`; with a PBLang core dependency
containing this module use `import("stdlib:pbai_feedback.sapp")`:

```text
input -> encoder hidden layers -> output -> decoder hidden layers -> reconstructed input
   ^                                                                         |
   +----------------------- bounded feedback --------------------------------+
```

This complements existing `nn.sapp` trainable MLPs and cyclic `nn_graph.sapp`
graphs. It uses **actual dense-network evaluations**, not descriptive topology
metadata. Every dense layer uses the existing sigmoid activation. Each direction
accepts 1–8 independently parameterized layers, with widths 1–32; encoder output
and decoder input dimensions must match, and decoder output must match the input.
Use `stack([net.l1, net.l2])` to import an existing `nn.mlp_new` model after explicit
training. `stack` copies parameters: training the original later will not update it.

## API

- `stack(layers)` validates and deep-copies dense layers (`n_in`, `n_out`, `w`, `b`).
  Weight layout is `w[output][input]`. Returns format `pbai.feedback.stack.v1`.
- `forward(model, input)` validates a model and performs a feed-forward pass.
- `cycle(encoder, decoder, input, options=nil)` evaluates a bounded recurrence.
  Models, options and input are never mutated. Returned vectors are detached.

Options are optional, but unknown keys and out-of-range values are rejected:

| Option | Default | Allowed |
| --- | --- | --- |
| `max_iterations` | 8 | integer 1–32 |
| `tolerance` | 0.000001 | finite 1e-12–1 |
| `damping` | 1 | finite 1e-6–1 |
| `anchor` | 0 | finite 0–0.999999 |

At each step, `y = encoder(x)`, `r = decoder(y)`, and
`target = anchor * original_input + (1-anchor) * r`.
The next state is `(1-damping)*x + damping*target`.
`anchor=0` recirculates the reconstruction. A positive anchor keeps part of the
original observation. Smaller damping relaxes changes; it is not a guaranteed
improvement. Swapping compatible encoder/decoder models provides an output-origin
cycle. Networks can have different hidden widths/depths in either direction.

Convergence uses the maximum absolute `target-x` **before damping**: a tiny update
must not be mistaken for a fixed point. `residuals` records one scalar per loop
iteration; final `residual` is recomputed at the returned state. A final diagnostic
encoder/decoder evaluation keeps `input`, `output`, and `reconstruction` consistent.
`pair_evaluations = iterations + 1` includes it. `stop_reason` is `converged` or
`iteration_limit`; bounded oscillations are reported as the latter.

**Convergence is not reconstruction accuracy.** `reconstruction_error` measures
maximum absolute deviation from the original input. A constant decoder may
converge while reconstructing the wrong observation. No accuracy target or learned
inverse is implied by reaching the numerical tolerance.

## Bounds and effects

Models are checked before inference. Input magnitudes are at most 1,000,000;
weights/bias magnitudes are at most 10,000. NaN, infinity, ragged matrices,
boolean/non-number values and inconsistent dimensions are rejected with a fixed
message that does not echo model data. The cycle accepts at most 16,384 combined
weights/biases. With the final diagnostic evaluation it makes at most 33 paired
passes (at most 540,672 parameter visits in those passes). Copying and validation
also consume CPU; this is an operation bound, not a wall-clock latency guarantee.
The sigmoid outputs are validated as finite [0,1] values after each layer.

The module performs no file/network/device access, logging, automatic training,
checkpoint promotion or global mutation. The caller chooses what to persist.
A per-call snapshot copies only supported numeric parameters; arbitrary callbacks,
custom activations and executable model providers are not accepted.

## Example and scope

Run `demo/pbai_feedback.sapp` with the project's compatible PBLang dependency.
It uses seeded demonstration networks; they are **not trained reconstructions**.
Tests cover forward parity with `nn.predict`, multiple hidden layers, anchored
and damped recurrence, oscillation, final-output consistency, detached ownership,
finite/shape/budget validation and the difference between convergence and accuracy.

An encoder/decoder can be trained separately with existing neural facilities before
calling this module. Joint autoencoder training, denoising objectives, learned
cycle consistency, predictive coding, deep-equilibrium implicit differentiation,
attention-based feedback and hardware-accelerated feedback remain future work.
This bounded inference loop does not implement those training methods or claim
consciousness, universal convergence, optimality or superiority over other models.

Background: [autoencoders](https://www.cs.toronto.edu/~hinton/absps/cvq.html),
[Deep Equilibrium Models](https://arxiv.org/abs/1909.01377).
