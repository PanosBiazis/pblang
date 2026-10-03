# Dimensions: PBLang form example

Run from the PBLang repository:

```sh
pblang run examples/nd_forms/main.sapp --allow-all
pblang run examples/nd_forms/main.sapp --allow-all -- --serve
```

The first command performs a local smoke check. The second opens a local HTTP
listener at `http://127.0.0.1:8768/`; open that address in a browser. Ctrl+C stops
it. Broad permissions are for this reviewed local example, not untrusted code.
The existing Linux HTTP backend is required. Other local applications can reach
this preview, so the example uses only synthetic data. Immediate reuse of the
same port after stopping remains an existing transport limitation.

The first graph shows sixteen 4D vertices after 4D -> 3D -> 2D affine projections.
The second selects time and response axes from synthetic 4D measurements.
Open **Point values** for the accessible data table; **Explain** runs an explicit
PBLang action. Both graphs declare explicit axis ranges, and point names are repeated in the
SVG tooltips and table. Vertex names preserve the original four coordinates.
These are affine projections, not a VR renderer.

Change matrices in `document.sapp` to select or combine dimensions. Modify the
plot with `ui_form_overlay.apply` without changing the source document. The
example checks that the original title remains unchanged. Imported foreign
handlers are never executed. No third-party scripts, assets or persistence are
required. For API budgets, privacy and limitations see
[ND form projections](../../docs/ND_FORM_PROJECTIONS.md).

## Native CPU preview

`pblang run examples/nd_forms/native.sapp --allow-all` draws the same sixteen
projected vertices through the existing graphics window API and closes after
three seconds. The Web table remains the textual alternative; the native preview
does not implement an OS accessibility bridge. No GPU acceleration is claimed.

For isolated Linux Xvfb tests, an inherited unavailable input-method service can
prevent minifb window creation. A process-local test setting such as
`XMODIFIERS=@im=none` allowed create/present/close in the tested environment.
Do not set this globally: it disables the external input method for that process
and is not appropriate for validating IME/compose input. Requesting SDL2 does not
add support to a binary that was compiled without that backend.

The 2026-10-03 native investigation also observed missing keydown events for
short Tab/Space presses and repeated keyup events in the retained minifb path.
A rendered window or successful pointer test is not proof of keyboard readiness.
Headless GUI semantics and Web keyboard tests are separate from this host-level
issue. No synthetic-keyup activation workaround is installed by these examples.
