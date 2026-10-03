# Synthetic admin metrics

A complete offline dashboard assembled from existing PBLang libraries: aggregate
cards, a six-row table, a 2D plot, and a 4D → 3D → 2D projection. All records are
fictional. This example has no login, SSO, tenant authorization, service connection,
personal data or telemetry. It is a graphics example, not an admin security layer.

From the repository root:

```sh
pblang run examples/admin_metrics/main.sapp --allow-all
pblang run examples/admin_metrics/main.sapp --allow-all -- --export
```

Open `build/admin-metrics/index.html` in a browser. The single file works offline
with JavaScript disabled. Tab reaches the horizontally scrollable aggregate table
and each **Point values** disclosure; Enter opens the disclosures. The shared web
stylesheet adapts to narrow screens and system light/dark preference.

## Customize the example

In `dashboard.sapp`, edit `points` and matching `names` (the example has six rows).
Each row is `[active_sessions, requests, failed_requests, p95_latency_ms]`.
`ND.compile(matrix).project(points)` chooses and scales axes. `Plot.render` accepts
named `point_labels`, `x_domain`, `y_domain`, `description`, and `table_open` options.
Domains must contain every plotted point. Change the final CSS string for card
colors/spacing; retain the shared stylesheet for responsive plots and focus styling.
When replacing the synthetic records, update the smoke-test totals in `main.sapp`.

The spatial view uses `x=requests/1000`, `y=failures/20`, `z=latency/100`, then
`U=x+0.35z`, `V=y+0.25z`. Active sessions are omitted from this view. Projections
can hide relationships and overlap points; visual distances are not anomaly scores.
Floating-point values in the chart tables may include rounding tails. The first
table preserves the original aggregate integers.

Browser checks cover 1280×1000 and 390×844 Chromium views, disabled JavaScript,
and opening both value tables by Tab/Enter (66 accessible data cells total).
These are browser viewport checks, not native Android/iOS device execution.
See [projection contracts](../../docs/ND_FORM_PROJECTIONS.md).
