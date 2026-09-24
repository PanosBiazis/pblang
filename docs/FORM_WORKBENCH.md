# Canonical form customization and local web actions

`ui_form_overlay.sapp` and `ui_form_web.sapp` provide a PBLang-only source path
from a **validated canonical document** to an interactive, local web form.
They do not translate arbitrary application code or manipulate native executables.
The retained host and existing HTTP system boundary still supply runtime/OS services.

## Run the example

```sh
pblang run examples/form_workbench/main.sapp --allow-all
pblang run examples/form_workbench/main.sapp --allow-all -- --serve
```

The first command runs a synthetic smoke check. The second serves only
`127.0.0.1:8766`; Ctrl+C stops it. Broad `--allow-all` is for this reviewed local
example, not untrusted code. Bounded HTTP transport currently requires the tested
Linux backend; do not infer Windows/macOS support from portable HTML.

The example imports a text Delphi DFM quotation form, adds labels and changes a
button through an overlay, and combines it with a PBLang review form. Quantity
and unit price are multiplied by an explicit PBLang function. Original Pascal
handlers are **never executed**, even if their names resemble a registered action.
No database, registration, persistence, tracking, foreign scripts or remote API
is added implicitly. Submitted values are reflected into that response only;
refreshing the root page restores configured defaults. Other local programs can
access the preview. Use synthetic data and trusted, short-running actions.

## Developer and agent preflight

```sh
pblang run tools/pbformcheck.sapp --allow-all -- INPUT.dfm
```

The CLI reads one explicit UTF-8 text DFM through the existing secure-file reader
(1 MiB input budget), never executes imported code and never modifies the file.
It rejects symlinks/unsupported secure-file backends instead of falling back to an
unprotected reader. The tested CLI boundary is Linux; its source is PBLang, while
the secure reader still uses existing operating-system/FFI support.

Its versioned `pblang.form-check.v1` JSON reports structural renderability, counts
of controls and fields, unmapped properties/events, external-reference presence
and whether explicit actions are needed. It excludes paths, component/handler
names, labels and field values. Exit 0 means structural preflight passed; exit 1
means the form/target is unsupported; exit 2 means invalid invocation/read failure.
`conversion_complete` and `production_ready` remain false: successful rendering
does not establish equivalent business behavior, event timing or authorization.

For tools that already hold source text, use
`import("stdlib:ui_form_check.sapp").analyze_dfm(text)`. For canonical documents,
use `Web.inspect(document)`. Neither call starts a server or invokes actions.
They share the actual web renderer validation, preventing a separate checker from
silently accepting controls the renderer cannot handle. These APIs are available
for future PBIDE/MCP adapters; this change does not install new PBIDE or MCP tools.

## Detached overlay API

```pblang
let Overlay = import("stdlib:ui_form_overlay.sapp");
let changed = Overlay.apply(document, [
    map{id:"Quantity", set:map{props:map{hint:"Whole units", value:"3"}}},
    map{id:"Calculate", set:map{props:map{text:"Calculate total"}}}
]);
let combined = Overlay.compose([
    map{prefix:"quote", document:changed},
    map{prefix:"review", document:another_document}
], "Workspace");
```

`apply` validates and detaches the source, applies an ordered list atomically, and
revalidates/detaches the result. `props` merges property keys; `events`, `bind`,
`x/y/w/h` replace those fields. Identity, kind and parent changes are rejected.
An error leaves the original document unchanged. It never writes the source file.

`compose` accepts 1–16 uniquely named parts, inserts a panel per part, and prefixes
widget IDs, parent references, model keys and binding keys. Handler strings remain
inert metadata. A button `Calculate` in part `quote` is bound by `quote.Calculate`
in the PBLang action map. Total canonical limits still apply: 512 widgets,
4096 value nodes, 8 levels of property nesting, 256 entries per collection.
Prefixes are ASCII identifiers starting with a letter, at most 32 characters;
combined IDs must also satisfy canonical limits. Canonical data can describe
custom controls, but a target renderer must explicitly support their semantics.

## Web contract

```pblang
let Web = import("stdlib:ui_form_web.sapp");
let app = Web.create(combined, map{
    "quote.Calculate": fn(values) { return "Result: " + values["quote.Quantity"]; }
}, 8766);
// app.handle(request) returns a bounded HTTP response; app.close() revokes it.
```

Actions receive a new map keyed by original canonical IDs, regardless of wire-name
minimization. Text fields yield strings, checkboxes booleans, and selects the option
text or nil for no selection. Disabled/hidden/read-only fields are not passed.
Duplicate choice labels are indistinguishable to the callback; use unique labels
when business logic must distinguish options. Return a text string up to 4096
characters; HTML is escaped. Imported event names do not select callbacks.
Caller-provided functions are trusted application code, not sandboxed plugins.

Supported controls: panel, label, textbox, textarea, checkbox, select, button.
Supported property types, parent containers and depth are checked before startup.
Unsupported controls/properties, password fields and unresolved bindings fail
closed. Geometry is deliberately adapted into responsive flow: pixel layout,
styles, tab order, validation/business rules and desktop interaction equivalence
are **not** preserved. Source widget order determines web order.

Actions require exact loopback Host and Origin, valid form encoding, an unguessable
per-preview CSRF token and the current revision. Hidden/disabled/unbound buttons
cannot acquire actions. Duplicate fields/headers, malformed encoding, forged
choices, stale forms and unexpected fields are rejected before callbacks.
A revision is consumed before calling trusted code: an action that changes external
state and then throws cannot replay with the old form. Error details are not
returned. This is not transactional rollback or global exactly-once execution;
a new form/restart can invoke the action again. Use durable idempotency in business
logic where required. Reentrant calls are rejected; parallel threads are unsupported.

No inline or imported scripts/styles, event attributes, URLs, external fonts,
analytics or model values are emitted. All visible strings are escaped. CSP
restricts resources and form destinations. Responses are no-store and nosniff.
`Referrer-Policy: same-origin` avoids sending referrers to other origins while
allowing normal POST Origin headers. `no-referrer` turns that header into `null`
for these navigations, as specified by the
[Fetch Standard](https://fetch.spec.whatwg.org/#append-a-request-origin-header).
`Origin: null` is still rejected. CSRF/Origin guards are not an account system.

The listener binds loopback only and limits headers (8192 bytes), request body
(65536 bytes), response (1 MiB), concurrent sockets (4) and I/O waits. The renderer
limits parent depth to 16 and input text to 4096 characters. These are preview
adapter budgets, not language-wide limits. Trusted callback CPU is not preempted;
callbacks must enforce their own resource/effect budgets. Sensitive inputs require
an authenticated application-specific adapter. Do not publish this preview through
a reverse proxy or expose it as a production application.

## Two independent options, ON by default

The most recent requested defaults are both **ON**:

```pblang
let policy = map{discourage_scraping:true, hide_source_names:true};
let app = Web.create(document, actions, 8766, policy);
// Opt out separately by setting either boolean to false.
// Web.serve(document, actions, 8766, nil, policy) uses the same settings.
```

- `discourage_scraping`: emits X-Robots-Tag restrictions and a disallow-all
  robots.txt, plus a per-preview global 120-request/minute fixed-window budget.
  Excess requests receive 429/Retry-After. Clock rollback closes the preview.
  This is voluntary crawler guidance and request throttling, **not bot detection
  or guaranteed scraping prevention**. A hostile local client can exhaust the
  shared budget. No IP/user-agent fingerprinting or tracking is used.
- `hide_source_names`: replaces HTML field/action identifiers with ordinal wire
  names and generic fallback labels. Original canonical IDs stay on the server
  and still key callback values. Explicit visible labels remain public. This
  reduces internal-name disclosure; it **cannot prevent reverse engineering**
  of delivered HTML/CSS or observable behavior. PBLang callback source is never
  sent to the browser in either mode.

Both options may be turned off independently. Turning them off does not disable
CSRF, Origin/Host checks, escaping, CSP, revision checks or input validation.
Unknown/nonboolean options, including a generic `unsafe` bypass, are rejected.
These defaults apply to **this adapter**, not automatically to every existing
PBLang web application or foreign/polyglot framework.

## Interoperability boundary and remaining adapters

| Source/target | Current verified scope |
| --- | --- |
| Text Delphi DFM | Existing structural importer plus synthetic web action example |
| PBLang canonical forms | Detached customization, composition and seven web controls |
| Binary DFM | Rejected; requires a bounded decoder or trusted designer text export |
| Lazarus | Separate local preview work exists; not a dependency of this change |
| Qt, Android/iOS XML, Blazor, HTML/JS frameworks | Need format adapters and explicit action/data contracts |
| Unreal, Godot, Blender | Need scene/component semantics and engine-specific adapters |
| Word/Excel/PowerPoint/PDF templates | Need document/schema extraction, field mapping and effect-free import |
| Running programs/executables on other OSes | Need authorized platform/accessibility APIs; no binary rewriting/injection |

A canonical document is an interchange boundary, not an automatic Pascal/C#/JS
compiler. Database rules, transactions, authorization, calculations, file/device
access and lifecycle hooks must be mapped and tested explicitly. Blazor components
cannot be embedded safely merely by relabeling controls. Native and polyglot
implementations can share explicit data/action schemas, but those bridges remain
separate work. Never execute macros, scripts or unknown source while importing.

## Quality gates, not a claimed Lighthouse score

The example serves a small standalone stylesheet and no JavaScript or third-party
assets. The response policy also sets COOP same-origin, CORP same-origin, COEP
require-corp, X-Frame-Options DENY and restricted Permissions-Policy. CSP disallows
inline styles/scripts and requests Trusted Types enforcement where supported.
These settings are appropriate to this self-contained preview, not automatically
compatible with every embedded/polyglot framework.

Automated checks cover headers, no tracking cookie/CORS grant, escaped fields,
labels, native controls and request rejection. Browser checks cover a calculation,
escaped review text, keyboard focus and a 390px layout without horizontal overflow.
Generated controls have labels; semantic headings/main/fieldset/legend, visible
focus styles and an output live region support keyboard/assistive use. A screen
reader assessment, broader zoom/contrast/RTL/language testing and full WCAG
conformance remain separate work. Correct focus after full-page navigation must
also be evaluated with the intended assistive technology.

No Lighthouse/PageSpeed score, Core Web Vitals field data, calibrated FCP/LCP/CLS,
SEO rank or WebMCP conformance is claimed. Default noindex intentionally conflicts
with public discoverability. A public SEO edition must deliberately opt out and
supply appropriate metadata/content. No WebMCP endpoint or tool discovery is
implemented here. The callback registry is server-internal and grants no AI client
permission. Any future agent interface needs authentication, scopes and effect
approval contracts.

HSTS, HTTPS redirects, certificates and Secure cookies require a real HTTPS
application/deployment boundary; adding those headers to an HTTP loopback demo
would not establish TLS. HSTS preload/includeSubDomains must be chosen only after
checking ownership and HTTPS readiness of the whole intended domain tree. No
production/Specisoft host was contacted or changed for these checks.

## Full application and multidimensional scope

The requested long-term scope includes Delphi/Lazarus code and all form families,
including custom controls; both spatial 3D/VR/AR interfaces and multidimensional
data/parametric projections. Neither is represented as complete by this 2D adapter.

Further stages require a Pascal/component semantic model, binary DFM/LFM support,
explicit database/transaction and OS-effect mapping, lifecycle/event-order tests,
and parity fixtures from authorized projects. Spatial targets need a separate
versioned scene/coordinate model (axes, units, transforms, projection, interaction,
accessibility alternatives and resource ownership), not silent flattening of n-D
coordinates into x/y. Device-specific rendering and bounded CPU fallbacks then
need real measurements. Unrecognized semantics must remain reported/rejected,
never inferred as equivalent execution. Original files stay unchanged; overlays
and adapter code remain separate and reversible.
