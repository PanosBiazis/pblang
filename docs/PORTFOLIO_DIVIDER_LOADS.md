# Divider loads — portfolio wave 14

The educational passive-circuit direction in PORTFOLIO_MEDIA_CIRCUITS motivates loaded-divider analysis and PBLoadLab. This adds an RL branch to the existing supported DC model rather than a second solver. PBCircuits exposes `loaded` and `load-gui`; PBLoadLab supplies CLI sweeps, numeric CSV and generated portable result pages.

`divider_load.analyze(vin,top,bottom,load)` returns a detached numeric row. `sweep` validates all 1–32 loads before calculating and preserves order/duplicates. `linear_loads` accepts 2–32 samples, inclusive exact endpoints, and permits equal endpoints. Inputs use V/ohm. Each physical resistance must lie in 1..1000000 ohms even when the parallel equivalent is below one ohm. Supply is fixed within +/-1000 V.

The shared native view clears stale results after invalid calculations and requires numeric settings to match the last calculation before CSV export. New files only, no atomic/durability claim. Portable snapshots hold three rows/page with Show/Hide and no live calculation or file management. Rounded V/mA/mW displays are not the full precision of JSON/CSV. Reduction percentage compares transfer ratios, including at zero supply.

Tests compare all 135 combinations of signed supplies and resistance extremes to an independent parallel-resistance formula, current/power balance, zero supply, monotonic load response, ordering, limits, malformed inputs, page bounds and stale-export protection. Linux runtime evidence and generated browser/Android artifacts do not establish physical Windows/macOS/Android verification. This is exact-resistance DC education, not tolerance analysis, real hardware certification or a performance claim.
