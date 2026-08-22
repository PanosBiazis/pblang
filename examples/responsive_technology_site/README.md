# Responsive Technology Site

A PBLang-only migration of two early multi-page HTML/CSS/JavaScript exercises.
The example keeps the useful learning goals, including navigation, responsive
layout, service cards, an interactive showcase, form controls, and status
feedback, while replacing the duplicated pages and legacy Bootstrap snippets
with one shared Modern UI component tree.

## Improvements over the source exercises

- one content model and one view tree instead of repeated navigation markup;
- portable actions for section and story selection;
- responsive rows and grids for compact and wide screens;
- accessible names and live status feedback for interactive controls;
- local-only contact demonstration that neither sends nor stores form data;
- no jQuery, Bootstrap, CDN, tracker, remote font, remote image, or authored
  browser script; and
- no personal author name, workstation path, placeholder address, phone number,
  or email address.

The original image files are intentionally not copied. They were not required
for the behavior, some pages referenced remote third-party assets, and this
migration does not assume redistribution rights. The source text set is tracked
only by a deterministic SHA-256 digest in `model.sapp`.

## Run

From the PBLang repository root:

```sh
pblang run examples/responsive_technology_site/main.sapp --allow-all
```

Open `examples/responsive_technology_site/build/web/index.html`. The exported
site is self-contained and does not require a server.

## Verify

```sh
pblang run examples/responsive_technology_site/test.sapp --allow-all
pblang lint examples/responsive_technology_site/model.sapp
pblang lint examples/responsive_technology_site/app.sapp
```

The focused test validates the same component tree for compact Web, desktop,
and Android compilation contracts. This slice exports and runs the Web edition;
it does not claim that fresh desktop installers or an Android APK were produced.
