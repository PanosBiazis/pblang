# Measurement worksheets and engineering units — 2026-09-27

This slice extends the existing unit library and PBCircuits and introduces PBMeasure 0.1.0-alpha. It reuses the calculator/circuit directions in `MIGRATION_INVENTORY_2026-08-21.md`, the educational component ideas recorded in `IDEA_INTAKE_ORM_2026-09-15.md`, and the already implemented literal table interchange. Implementation is newly authored PBLang. Existing intake documentation supplied direction; this work did not perform another whole-disk content scan or copy a private dataset or third-party implementation.

## Unit API

Existing helper functions, lowercase binary-byte names, `convert` and `convert_temp` retain their prior behavior. Engineering symbols extend the factor table for voltage, current, resistance, power, frequency, capacitance, pressure, energy, volume, area, and explicit decimal/IEC bytes. Symbols are exact and case-sensitive. No automatic prefix inference or expression parsing occurs. `degC/degF/K` belong to the new checked temperature interface; `F` denotes capacitance. Legacy `convert_temp` continues to accept `C/F/K`.

- `describe_unit(symbol)` returns a detached `{symbol,category,scale,affine}` descriptor. The temperature scale is the multiplicative difference scale; absolute conversion applies the explicit temperature transform internally. This is not a complete affine offset descriptor.
- `catalog(category=nil)` returns sorted detached descriptors, optionally filtered by exact category. Unknown valid category names return an empty list.
- `convert_checked(value,from,to,mode="absolute")` checks numeric bounds, symbols, categories, mode and physical absolute-temperature lower bound. Inputs and outputs must be finite and within +/-1e12. Multiplicative nonzero underflow is rejected. There is no implicit rounding or coercion from strings/booleans.
- `convert_many(values,from,to,mode="absolute")` validates up to 512 values while resolving the common unit pair once. It returns a detached array, including empty-array support; invalid input rejects the operation without returning partial results or mutating its source.

Absolute mode uses offsets for temperature; difference mode uses only the ratio of scales, allowing signed differences. For other dimensions both modes agree. Direct C/F conversions reuse the existing formulas to avoid unnecessary large-offset subtraction; identity returns the original value after validation. Floating-point approximation remains, including for legacy rounded non-SI constants. No exact decimal, uncertainty algebra or general dimensional-expression engine is claimed.

## Shared products

`measure_document.sapp` defines strict 1–31-row worksheets with detached edits and native JSON persistence. Title/label limits are 60 characters and 120 UTF-8 bytes. A computed result is derived from source value/units/mode rather than persisted as a second truth. `to_sheet` creates the supported six-column literal table with one header; the worksheet bounds fit existing row, cell and total-byte budgets. Unknown/duplicate persisted fields reject. `measure_view.sapp` implements page readers with eight rows and portable selection actions, preserving complete admitted Unicode labels.

PBMeasure provides a native worksheet editor, eight applied undo steps, catalogue inspection without draft loss, new-copy persistence, CLI conversion/inspection and CSV/PBOffice exports. Table reports omit the worksheet title and do not reconstruct editable row types. Browser/Android readers are snapshots with local selection; editing and persistence remain native/CLI operations.

`divider_analysis.sapp` computes an ideal unloaded divider's independent resistor-corner bounds at fixed supply, including signed current ranges. It validates through the existing circuit domain and rejects tolerances beyond 0..20% or resistance corners outside 1..1000000 ohms. The voltage ratio is monotone in the two positive resistances, so cross-corners give extrema; negative supply reverses the ordering, handled explicitly. The implementation is checked against the existing nodal solver over 144 combinations and 576 corners. It is not a confidence interval or hardware measurement.

PBCircuits exposes native V/mV and ohm/kohm interpretation selectors and a tolerance field. `tolerance_view` makes the result available in the same portable snapshot model. Existing general-network and nominal CLI results remain unchanged. No additional hardware capabilities are introduced.

## Validation and performance

Tests cover reverse conversions within all catalog categories, scalar/batch equality, absolute versus difference temperatures, absolute zero, case sensitivity, binary/decimal names, invalid and out-of-range input, detached metadata, maximum batch size, retained documents, strict reconstruction, complete Unicode display, Sheet/CSV roundtrips and native product workflows. Old unit and circuit suites remain in the regression set. Lint/build success is separate from installed-platform evidence.

`bench/unit_batch.sapp` compares repeated checked conversions against one checked batch using identical input/output, two warmups and seven samples for 32/128/512 values. This measures the new batching path, not overall PBMeasure application speed. On the measured shared Linux host, repeated versus batch median/p95 milliseconds were:

| Values | Repeated calls | One batch |
|---|---:|---:|
| 32 | 0.299 / 0.472 | 0.104 / 0.157 |
| 128 | 1.143 / 1.801 | 0.368 / 0.634 |
| 512 | 5.027 / 5.960 | 1.457 / 2.331 |

p95 is nearest rank (maximum of seven); timings depend on runtime and host load. Heterogeneous worksheet rows still perform individual conversions because their unit pairs/modes can differ. No product-wide speedup or GPU benefit is inferred.

Linux native runtime/file tests and browser actions are verified independently from generated Android Activity classes/manifests. Physical Windows/macOS/mobile execution, packaged release readiness and full editing parity remain future work.
