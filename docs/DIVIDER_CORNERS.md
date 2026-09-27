# Loaded-divider tolerance corners

This composition extends the existing `divider_analysis.sapp` unloaded-tolerance and `divider_load.sapp` loaded-divider recipes identified in the discovery documentation. It supplies PBCircuits's new corners workflow and the focused PBCorners educational workbench.

`stdlib:divider_corners.sapp` exports `analyze(vin, top, bottom, load, top_percent=0, bottom_percent=0, load_percent=0)` and `from_tokens(tokens)`. `stdlib:divider_corners_view.sapp` exports `view`, `build`, `calculate`, `run`. PBCircuits and PBCorners reuse both modules; no duplicate solver or new dependency is introduced.

The supply is fixed, with R1 between supply/output and R2 plus RL between output/ground. All three resistance intervals are independent. Supply: -1000..1000 V. Each tolerance: 0..20 percent. Every low and high endpoint must remain within 1..1000000 ohms; invalid endpoints reject rather than clamp. Zero tolerance retains all eight rows, including duplicates. Rows are ordered R1-low/high, R2-low/high, RL-low/high; indices 0..7 identify the first minimum/maximum in ties.

The existing nodal solver evaluates the nominal network and all eight corners. For positive supply, output increases with R2 and RL and decreases with R1; negative supply reverses these directions. Thus the corner voltage extrema bound this ideal model's whole independent interval box. This is a floating-point deterministic range, not a statistical confidence interval. Current/power fields in each row are point results: no global power extrema or component rating guarantees are claimed. No supply tolerance, correlated components, temperature, reactive elements, measurements or hardware access.

CLI JSON retains runtime numeric precision. Display rounds to six decimals and can hide tiny differences. Numeric text uses the existing strict 64-character decimal/exponent parser; whitespace, suffixes, nonfinite values and nonzero underflow reject. GUI fields have a 24-character limit. Seven values are required: V, R1, R2, RL, R1-percent, R2-percent, RL-percent.

## Editions and checks

Native desktop supports editable calculation and clears stale results on invalid input. Web/Android exports are read-only snapshots with two accessible page actions covering all eight corners. Exported HTML includes caller-selected numeric values; no automatic scanning, telemetry, network request or hardware read is added. The caller retains/deletes generated files.

Run `pblang run tests/semantics/divider_corners_test.sapp` and `pblang run tests/semantics/divider_corners_view_test.sapp`. The numerical suite covers 405 cases, 3240 corner oracles using an independent parallel-resistance formula and 10935 interior points, plus rejection, tie ordering and input preservation. Existing loaded/unloaded analysis remains compatible.

Linux is the local execution target. Windows/macOS physical execution and installed Android are not established by compiling shared views. These are alpha source editions, not installers or certified circuit design tools.
