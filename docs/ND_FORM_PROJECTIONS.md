# Multidimensional projections and web form plots

`nd_projection.sapp` compiles a detached affine transformation in PBLang.
`ui_scatter_plot.sapp` renders a bounded, passive two-dimensional SVG with a
textual data table. Canonical web forms accept it as a `scatter` widget.
These are functional building blocks, not automatic conversion of arbitrary
Delphi/Pascal applications, a 3D engine, VR/AR interaction, or executable rewriting.

## Projection API

```pblang
let ND=import("stdlib:nd_projection.sapp");
let view=ND.compile([[1,0,0,0.5],[0,1,0,0.25],[0,0,1,0]], [0,0,0]);
let spatial=view.project([[1,2,3,4]]); // [[3,3,3]]
let display=ND.compile([[1,0,0.4],[0,1,0.3]]).project(spatial);
```

Rows define output axes; columns define input axes. Bias is optional (zero by
default). The returned plan exposes `input_dimensions`, `output_dimensions`,
`nonzero_terms`, `evaluated_terms`, `max_points`, and `project(points)`.
Changing public metadata cannot change the private plan. Matrix and bias values
are copied; returned arrays do not alias the plan or the input points.

Rows with fewer than half their coefficients nonzero use sparse entries; other
rows use dense arrays. Both preserve coefficient order and the same numerical
validation. Every input coordinate is checked, including zero-weight axes.
Numbers must be finite and within +/- 1e100; each resulting coordinate must also
fit that range. This is floating-point arithmetic, not exact symbolic geometry.
An empty point batch returns an empty array.

The configurable integer `work_limit` (default 1,000,000, maximum 2^53-1) bounds
matrix coefficients and per-batch work. Before allocating results, `project`
requires point count <= floor(work_limit / (input dimensions + evaluated terms +
output dimensions)). It then checks point shapes and numbers. Raising this budget
allows more dimensions/work; it is not a global dimension limit, heap quota,
concurrency guard, or hard real-time deadline. Callers still own the memory of
inputs they constructed. Compile trusted reusable plans outside request loops.

## Web form integration

```pblang
map{id:"Chart",kind:"scatter",parent:nil,x:0,y:0,w:640,h:360,
    props:map{points:[[1,2],[2,4]],title:"Measurements",x_label:"Time",y_label:"Value"},
    events:map{},bind:nil}
```

Use this widget inside a canonical `ui_document`. `ui_form_overlay.apply` can
replace its properties without modifying the original document or source form.
`ui_form_web.inspect` checks the same renderer contract used by `create`.
This does not add automatic mappings for native Delphi chart components.

Plots accept 1-256 two-dimensional points within +/- 1e12; labels are strings
up to 4096 characters. The ordinary canonical limits (including 4096 total value
nodes and 256 entries per collection) also apply. Output is capped at 256 KiB per
plot. Unknown properties, bindings, and action registration for plots fail closed.
Plots are prepared once from the detached document, not from submitted form data.

Labels are HTML-escaped. SVG contains only generated geometry and text, with no
scripts, remote assets, or input event handlers. A labelled figure and expandable
HTML table expose all point values; keyboard users can open the table. By default each axis
is independently normalized; constant axes are centered. Explicit domains
override autoscaling, including constant axes. Distances and angles
are not preserved on screen. Consult the table for exact source values.

Plot data is deliberately sent to the browser and can be read or copied. Existing
anti-scraping/source-minimization options cannot make visible data secret. Use
synthetic data for the local preview. No tracking, file persistence, new polyglot
block or network fetch is introduced by these modules. Runtime and local HTTP
services still come from the retained host and existing OS/FFI boundary.

## Example and scope

See [the runnable example](../examples/nd_forms/README.md): sixteen 4D vertices
are transformed into 3D, then into a 2D display; a second plot selects two axes
from synthetic four-dimensional measurements. An overlay changes its title and
an explicit PBLang button explains the projection. This covers data projection
and passive spatial views, not a complete multidimensional interaction framework.

Validation includes independent dense-oracle numerical comparisons, malformed and
nonfinite input, budget boundaries, detached data, escaping, forged properties,
request-field rejection, cross-origin rejection, CSRF/revision handling, replay,
and close. Pure projection/plot tests also run with filesystem, network, HTTP,
execution, FFI, spawning, and environment effects denied. The retained Linux host
is the tested execution platform; generated SVG portability does not establish
Windows/macOS/guest runtime compatibility.

## Measurements and remaining work (2026-09-24)

`bench/nd_projection_stages.sapp` uses 256 points, 32 input axes, 3 output axes,
and 125 repeated calls. Set `PB_ND_BENCH=sparse` or `dense`. The comparison forces
`if active*2<inputs` to `if false` in an isolated reference copy of the same module;
all validation, inputs and numerical assertions remain identical. It compares
algorithms for this new capability, not a previous released PBLang feature.
One warmup per combination, seven alternating-order samples; CPU includes process
startup. p95 uses nearest rank (the maximum at this small sample count).

| Workload | Dense reference CPU median / p95 | Adaptive CPU median / p95 | Median peak RSS reference / adaptive |
| --- | --- | --- | --- |
| Sparse axis selection | 1.10 / 1.12 s | 0.56 / 0.57 s | 13,488 / 13,416 KiB |
| Dense transformation | 1.13 / 1.83 s | 1.11 / 1.55 s | 13,432 / 13,364 KiB |

The sparse workload used approximately 49% less CPU. Dense medians are close,
with substantial variation in slower samples. Small RSS differences do not prove
general memory savings. Plot preparation retains generated SVG/table text, trading
memory for avoiding repeated rendering; this tradeoff was not benchmarked here.
There is no measured speedup for accounts, PB-MCP Live View or a whole application.

Existing unresolved work includes the saved-account versus in-memory overhead,
immediate same-port HTTP restart, bcrypt's libxcrypt/FFI dependency, complete
application conversion, interactive multidimensional/VR interfaces, and broader
runtime/platform verification. No claim of a 1.0 stable release is made.
A counted-loop local referenced inside a nested conditional expression produced
`undefined global` on the retained host while preparing this benchmark; the
workload uses ordinary conditional statements. That compiler behavior remains open.

Nine focused suites passed, including form, overlay, document, preflight and PBIDE
checks; the projection suite includes 292 dense-oracle scalar comparisons. The
local browser rendered both plots, expanded the table and successfully submitted
the explicit PBLang action. Lint, build, portable-lock publication and manifest
validation passed in an isolated fixture. Entry-only security analysis reported
no unsuppressed findings; it is not a complete security audit. Existing heuristic
lint warnings remain. The Graphify update exceeded a 20-second bound and was
terminated; no claim of refreshed graph coverage is made.

## Named points and comparable scales

`Plot.render(points, title, x_label, y_label, options)` accepts an optional fifth
argument. Existing four-argument calls and an empty options map retain the
previous HTML byte for byte. Canonical scatter widgets pass the same map in
`props.plot_options`:

```pblang
map{points:[[1,2],[3,4]],title:"Projection",x_label:"X",y_label:"Y",
    plot_options:map{x_domain:[0,4],y_domain:[0,8],
        point_labels:["Reference","Candidate"],
        description:"Both views use the same coordinate ranges.",table_open:true}}
```

- `x_domain` / `y_domain`: optional `[minimum, maximum]`, strictly increasing,
  within the existing coordinate bound. Every point must fit. Out-of-domain
  points are rejected; none are silently hidden, clipped or clamped. Omitted
  axes retain autoscaling. Shared domains provide consistent scale across plots
  only when the same axes/units and plotting dimensions are used.
- `point_labels`: optional array with exactly one nonempty string per point, up
  to 256 characters each and 4096 UTF-8 bytes in total. Names appear in the SVG
  tooltip and the first column of the data table. Duplicate names are allowed;
  rows preserve input order. No color-only encoding is required.
- `description`: optional text up to 4096 characters, visibly rendered below the
  plot. It is descriptive text, never interpreted markup.
- `table_open`: optional boolean, default false; opens the native HTML details
  element initially. Users can still open/close it with keyboard controls.

A nonempty options map also displays point count and exact axis limits as text.
Titles, labels and descriptions remain escaped; unknown options fail closed.
The 256 KiB plot output limit and canonical document budgets remain in force.
Options are prepared from the detached canonical document, and hidden plots do
not expose their labels or descriptions in the page. Names are public plotted
data, not private metadata. This extends passive Web/SVG visualization of 2D
data or projected 3D/ND data; it does not add a native GPU renderer, device
installation or OS accessibility bridge. No rendering speedup is claimed.
