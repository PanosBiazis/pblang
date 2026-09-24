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
PBLang action. Each graph scales axes independently and is not a VR renderer.

Change matrices in `document.sapp` to select or combine dimensions. Modify the
plot with `ui_form_overlay.apply` without changing the source document. The
example checks that the original title remains unchanged. Imported foreign
handlers are never executed. No third-party scripts, assets or persistence are
required. For API budgets, privacy and limitations see
[ND form projections](../../docs/ND_FORM_PROJECTIONS.md).
