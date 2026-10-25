# Report customization contract

`stdlib:report_customization.sapp` exports `plan(ui_language, preferences_saved=nil, operation=nil)`.
It creates a detached in-memory report preference plan. No HTML, CSS, SQL, files,
network operations, translation service, locale detection or rendering are involved.

Omitted/nil preference maps mean empty preferences. Both non-nil preference maps allow only `report_language` and `style`. Languages are exactly
`el` or `en`. Missing/nil report language inherits operation → saved → `el`; unsupported
values fail rather than silently falling back. UI language is validated independently
and never selects the report language. The returned `language_source` records the
chosen layer. Even an overridden invalid saved value is rejected.

Style maps allow exactly these optional fields, merged field-by-field in the same
order, without mutating either input:

| Field | Accepted values | Default |
|---|---|---|
| preset | light, dark, ink | light |
| font_size | integral number 9–24, interpreted as points by a future renderer | 12 |
| landscape | boolean | false |
| dense | boolean | false |

The existing `theme` library supplies foreground/background roles and contrast.
Ink selects black on white, with ratio 21. Contrast covers that pair only, not all
elements or complete document accessibility. No arbitrary markup, font name, style
property or color string is accepted. This does not sanitize external content that
a later renderer may consume. `safe_to_log` remains false.

Run `pblang run examples/discovery_report_customization.sapp` for a self-checking
Greek UI/English report example, and `pblang run tests/semantics/report_customization_test.sapp`
for precedence, boundary, rejection and immutability checks.

This is original PBLang implementation using existing primitives. The report model,
message translation/fallback catalogs, number/date formatting, product GUI integration,
HTML/PDF/DOCX/Excel renderers, persistence and print-device behavior remain separate work.
No claim of complete report-engine or product integration is made.
