# Calibration worksheet and matrix scenarios

These implement two recorded ideas with synthetic, executable numerical examples. They are not instrument calibration certificates or completed graphical workbenches.

## calibration_sheet

`make(points, input_unit, output_unit)` builds a detached model with format `pblang.calibration-sheet.v1`. Exactly 2–64 `{input,output}` reference points are required; inputs must strictly increase. Every number must be finite and between -1e9 and +1e9 inclusive. Unit labels follow `template_fields.valid_name` (1–32 ASCII identifier characters). They are declarations: no unit conversion or dimensional compatibility is inferred. Model keys and point keys are exact; unknown fields are rejected.

`validate(model)` also validates decoded models. `estimate(model, queries)` accepts 1–256 numeric queries and delegates to the existing `sample_series.interpolate`. Reference endpoints remain exact; values outside the range are rejected. Non-monotone reference outputs are allowed. Piecewise linear interpolation is a model assumption, not a physical law or uncertainty estimate.

`review(model, checks)` accepts 0–256 unique `{id,input,expected}` checks with bounded logical IDs and numbers. It returns a fresh row per check with ID, estimate and signed residual **estimate minus expected**, plus maximum absolute residual, mean absolute residual and RMSE in the output unit. Empty checks produce `nil` statistics rather than false zero-error evidence. Validation finishes before an output is returned. Inputs remain unchanged.

The review omits reference points and raw check inputs/expected values, and never claims verified physical calibration. IDs and derived numeric values can still be sensitive; only synthetic inputs are used in examples. Errors do not echo rejected values. There is no file/device/network access, logging, persistence or credential use. JSON decoding is performed by the caller, followed by validation.

Four recipes demonstrate piecewise interpolation, residuals, extrapolation rejection and non-mutating JSON roundtrip/reporting. Tests also cover strict types, NaN/infinities, exact fields, duplicate IDs, 64/256 bounds and finite residuals at the numeric limits.

## Matrix scenarios

`discovery_matrix_scenarios.sapp` calls existing `matrix.solve_many` for three RHS columns, compares each result with `matrix.solve`, and measures the infinity norm of `A*x-b`. The synthetic coefficient matrix requires row pivoting. Expected solutions are `[1,2]`, `[2,4]` and `[1,-1]`. Inputs are checked for preservation; singular, ragged, nonfinite, nonnumeric and oversized RHS inputs are rejected by the existing solver.

Three recipes cover independent-solution agreement, residuals and invalid scenarios. No numerical condition estimate, arbitrary-matrix accuracy guarantee, graphical residual plot, performance gain or external platform qualification is claimed.

```sh
pblang run tests/semantics/calibration_sheet_test.sapp
pblang run examples/discovery_calibration_sheet.sapp
pblang run examples/discovery_matrix_scenarios.sapp
```
