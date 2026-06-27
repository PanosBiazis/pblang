# pbAI Goal: The Ideal NN (train toward an imaginary optimum)

> **Status:** north-star goal + first computable contract (`stdlib/pbai_ideal_nn.sapp`).
> Self-contained; not yet wired into the pbAI-Max training loop. Owned in the
> Claude lane; integration into distillation/acceptance is a future *joint* change
> coordinated on the agent bus with `codex-pbai-pblang-main`.

## Relationship to pbAI-Max

This module is the **computable mechanism** behind the Ideal-NN research target. The
goal declaration and pbAI-Max research-target metadata live in
[`pbAI_Max_Strategy.md`](pbAI_Max_Strategy.md) and `stdlib/pbai_max.sapp`.
`pbai_max.status().ideal_neural_target.math_contract` now exposes a tiny verified
join point: one `step_toward_ideal(...)` reduces `ideality_distance(...)`, while
`training_loop_wired` remains `false`.

The future training join point is the distillation loss:
`loss = task_loss + lambda · ideality_regularizer(model, ideal)` — wiring it into the
Max loop is a future change that must extend validation, benchmark, and safety gates.

## The idea

Most training optimizes a loss against data. This goal adds a second, complementary
compass: an **Ideal NN** — an idealized, partly *imaginary* network defined as a
**mathematical object**, not a checkpoint. We measure a computable **ideality
distance** `D(current, ideal)` and train the current network to *move toward the
ideal* by reducing `D` (as a regularizer alongside the task loss, or as an extra
acceptance-gate signal).

The Ideal NN is "imaginary" in two senses: it is not a realizable trained artifact
(it is a limit/target), and one of its axes literally lives in the **complex plane**.

```text
current NN ──(reduce ideality distance D)──▶ Ideal NN (mathematical target)
            spectral + probabilistic + geometric + complex
```

## Four axes of the ideal (the math)

| Axis | Field | The ideal is… | Computable proxy |
|------|-------|---------------|------------------|
| **Spectral** | linear algebra | weight matrices are **orthogonal** (dynamical isometry: every singular value = 1, gradients neither vanish nor explode) | orthogonality defect `‖WᵀW − I‖_F`; the **Newton–Schulz** iteration `W ← 1.5W − 0.5·W(WᵀW)` drives a matrix toward orthogonality |
| **Distributional** | probability | output matches a **target distribution** (well-calibrated, max-entropy subject to constraints) | `KL(p‖q)` and the **Hellinger** distance `H(p,q)=¹⁄√2·‖√p−√q‖₂` |
| **Geometric** | geometry + probability | a point on the **statistical manifold**; we move along geodesics, not straight lines | the **Fisher–Rao geodesic** on the probability simplex: `d(p,q) = 2·arccos(Σᵢ √(pᵢqᵢ))` |
| **Complex / imaginary** | complex numbers | weights, viewed as complex numbers `re + i·im`, sit at the ideal point `1 + 0i` (unit magnitude, zero phase) | magnitude defect `|‖w‖−1|` and phase defect `|arg w|`, aggregated as the RMS distance to `1+0i` |

The composite **ideality distance** is a weighted sum of the four axis distances. The
weights are the *research dial*: emphasizing the spectral axis pursues trainability,
the geometric axis pursues calibrated probability, the complex axis is the
exploratory/"imaginary" frontier.

## The contract (`stdlib/pbai_ideal_nn.sapp`)

- `ideal_spec(...)` — declare the target (target distribution, complex target, weights).
- Spectral: `orthogonality_defect(W)`, `newton_schulz_step(W)`.
- Probability: `kl_divergence(p,q)`, `hellinger(p,q)`.
- Geometry: `bhattacharyya(p,q)`, `fisher_rao(p,q)`.
- Complex: `complex_magnitude_defect(w)`, `complex_phase_defect(w)`, `complex_ideality(ws)`.
- Composite: `ideality_distance(model, ideal)` → `{ total, spectral, distributional, geometric, complex }`.
- Train-toward-ideal: `step_toward_ideal(model, lr)` → a nudged model whose
  `ideality_distance` is provably lower (locked by a test).
- Opt-in hook: `ideality_regularizer(model, ideal)` — the scalar to add to a task
  loss: `loss = task_loss + lambda · ideality_regularizer(model, ideal)`.

### Numerical note
`pbai_ideal_nn` computes its own `acos(x) = atan2(√(1−x²), x)` so the Fisher–Rao
geodesic is exactly 0 for identical distributions, keeping the module self-contained.
The underlying `stdlib:math.sapp` trig has since been fixed and exported too: `asin` is
now the exact `atan2(x, √(1−x²))` (the old 2-term Taylor gave `acos(1) ≈ 0.33`), and a
latent bug where `math.sapp`'s `range` shadowed the `for-in-to` loop desugaring — which
silently broke `sin`/`cos` — was fixed by renaming it (`stats.range` API preserved).
Full pblang `test tests/semantics` stays 191/0.

## Roadmap

1. **Now:** standalone module + lean tests (this commit).
2. **Next (joint):** offer `ideality_regularizer` as an opt-in term in the pbAI-Max
   distillation loss and an extra signal in `distillation_acceptance_gate`.
3. **Later:** per-layer target spectra, learned ideal distributions (NN-as-GP limit),
   natural-gradient flow using the Fisher metric, fully complex-valued layers.
