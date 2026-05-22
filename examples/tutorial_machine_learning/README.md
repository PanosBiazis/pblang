# tutorial_machine_learning — ML Pipeline Demo

A self-contained machine learning pipeline built from scratch in pblang.
No external ML libraries needed — all algorithms are implemented in pure pblang.

## Features Demonstrated

| Feature              | How it's used                                   |
|----------------------|--------------------------------------------------|
| **data generation**  | Synthetic dataset with noise                     |
| **train/test split** | `ml.train_test_split()` for 80/20 split          |
| **linear regression**| Closed-form least-squares fit from scratch        |
| **prediction**       | Model inference on test data                     |
| **metrics**          | MSE, RMSE, R² score                              |
| **feature scaling**  | Standardization via `ml.standardize()`            |
| **cross-validation** | 3-fold CV with `ml.cross_validate()`              |
| **imports**          | `math.sapp`, `ml.sapp`                            |
| **functions**        | Pure functions for fit, predict, evaluate         |
| **maps & arrays**    | Data structures for features, targets, models     |

## Setup & Run

```bash
# From the pblang project root:
pblang run examples/tutorial_machine_learning/main.sapp
```

## Pipeline Steps

1. **Generate Dataset** — Create 30 data points following `y = 3.5x + 10 + noise`
2. **Train/Test Split** — 80% train, 20% test
3. **Fit Linear Regression** — Closed-form least squares: compute slope and intercept
4. **Predict** — Apply model to test set
5. **Evaluate** — MSE, RMSE, R² metrics
6. **Feature Scaling** — Demonstrate standardization
7. **Cross-Validation** — 3-fold CV on training data

## Expected Output

```
=== Step 3: Fit Linear Regression ===
True relationship:  y = 3.50 * x + 10.00
Learned model:      y = 3.48 * x + 9.97

=== Step 5: Model Evaluation ===
MSE:  2.14
RMSE: 1.46
R²:   0.987

=== Step 7: 3-Fold Cross-Validation ===
Cross-validation scores: [0.98, 0.99, 0.97]
Mean CV score: 0.98
```

## Key Concepts

### Linear Regression (closed form)
Given features X and targets y, the optimal parameters are:
```
w = Σ((x - μ_x)(y - μ_y)) / Σ((x - μ_x)²)
b = μ_y - w * μ_x
```

### R² Score
Measures how well the model explains the variance:
```
R² = 1 - SS_residual / SS_total
```
A score close to 1.0 means the model captures nearly all the variance.
