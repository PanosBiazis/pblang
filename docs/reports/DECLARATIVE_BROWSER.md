# Declarative browser foundation

This additive slice provides actual local-document rendering, navigation and
roundtrip conversion. Existing `browser.sapp` and application callers are unchanged.
Start with [the runnable example and configuration guide](../examples/declarative_browser/README.md).

## Reuse and scope

- `browser.sapp`: existing search URL templates, percent encoding and advisory
  URL/ad/tracker heuristics are called directly. Its unrestricted reader fetch and
  form-submit functions are not called by the new session.
- `ui_document.sapp`: existing canonical document construction/validation and
  detached bounded values. No second canonical UI format is introduced.
- `html.sapp`, `ui_form_web_document.sapp`: escaping, page assembly and shared
  responsive styling for passive local exports.
- `modern_ui.sapp`, `gui.sapp`, existing Web/Android adapters: responsive text,
  native controls, portable destination-inspection actions, font handling and
  keyboard/pointer input. No Rust or foreign production source is added.
- PB-MCP's separate isolated browser/mediated navigation implementation remains
  a separate tool boundary. This library neither clones it nor claims its process
  containment, HTTPS transport or credentials policy.

A scoped read-only inventory of existing AAP browser/UI modules and local project
metadata preceded implementation. Additional SW/BACKUP discovery supplied reference
findings, not a tested reusable document navigator. No proprietary source was copied;
no claim of exhaustive project discovery is made. Existing `fetch_tor` is a stub,
and external host-browser screenshots are not a PBLang HTML engine or a Tor route.

## Implemented contract

`browser_document.create/import_html/export_html/inspect` map a strict flat
article/title/heading/paragraph/local-link grammar to `pblang.ui-document.v1`.
Version, shape, UTF-8 byte counts, text, local addresses, coordinates, bindings and
handlers are validated. Unknown markup is rejected; literal markup is escaped.
Only the supported normalized layout is reversible. The source is preserved.

`browser_session.create` snapshots owner-supplied source/grants, enforces local
navigation, back/forward/home, bounded memory history, closed state and explicit
locked/local/review levels. `bundle()` returns only granted local pages with fixed
safe relative HTML names. It does not write files; the example's explicit export
operation writes them. Exported pages are readable by recipients and are not an
access-controlled hosting service.

`browser_view.create/plan` project the same content to Modern UI. The portable
button action **inspects a destination**. The native example binds actual local
navigation separately. It never executes imported handlers. Search URL proposals
are non-executable strings and do not alter history. Unsupported permission and
policy options throw, including `level:"tor"` and `javascript:true`.

## Platform and behavior matrix

| Surface | Implemented in this slice | Evidence / remaining boundary |
| --- | --- | --- |
| Pure PBLang core | Bounded HTML↔UI roundtrip, local navigation/grants/history/search proposals | Retained Linux semantic tests; platform-neutral data/functions, no universal host claim |
| Native Linux | GUI document navigation, Back/Forward/Home through existing graphics input | Isolated X11 test; see delivery evidence |
| Native Windows/macOS | Existing GUI adapter and shared core | No live device execution in this slice |
| Web passive bundle | Static local HTML links, text, keyboard accessibility | Generated files and isolated supplied-HTML inspection; no server/auth layer |
| Web Modern UI | Same text and portable destination-inspection action | Existing bounded action runtime; does not perform session navigation |
| Android | Same view/actions; existing JVM Activity and no-permission manifest generation | Class/contract checks and verified debug APK packaging; no installation/emulator/device proof |
| iOS | Shared declarative representation | Adapter explicitly unlinked; HTML export is an explicit fallback, not native iOS execution |
| Other targets/devices | Existing Modern UI target validation | Unsupported targets fail; no implicit native permissions |

No general JavaScript, arbitrary HTML/CSS, remote browsing, DNS/TLS routing, Tor,
antivirus, file downloads, cookies, accounts or SSO is implemented. There is no
claim of superiority to Chromium/Firefox, universal privacy, faster rendering or
all-platform compatibility. Offline operation avoids external resource requests;
it does not make exported content secret or sandbox other application code.

## Verification expectations

Focused tests cover escaped text, Greek Unicode, exact supported roundtrip,
unknown/nested/malformed markup, entities, control characters, budgets, denied
addresses, layout/handler rejection, path traversal, detached grants/documents,
failed-navigation atomicity, history bounds and close. Cross-target checks compare
content and action records, generate real Android class bytes and reject unsupported
permissions. Existing browser and UI suites provide compatibility coverage.

The product source only uses PBLang. The optional X11 test driver is verification
infrastructure and controls only an isolated Xvfb display. Hosted CI and device
results must be reported separately from source/contract checks.

## Recorded verification (2026-10-03)

Eleven focused suites passed, including the new document/session/view tests and
existing browser/native-IDE, canonical UI, Modern UI/Web, toggle and graphics-input
coverage. The two pure core suites also passed with filesystem/network/HTTP/exec/FFI/
spawn/environment/graphics/SQLite effects denied (module import remained enabled).
Nine PBLang files checked with zero errors/warnings; retained build and portable
lock passed. Xvfb physical Tab/Enter navigation performed link → Back → Forward,
ending at `pb://guide` with exactly three navigation actions and Greek title intact.

PB browser supplied-HTML inspection at 1280×900 and 390×844 showed the expected
headings/links and visible Tab focus, with JavaScript disabled. Static relative-link
bundles were generated; this is not remote-network or authenticated-hosting evidence.
The Android exporter produced a 12,715-byte signed debug APK for
`org.pblang.documentreader`, min SDK 21/target SDK 34, with signature, alignment
and package metadata verified by the existing build toolchain. No device was used.
The shared-view manifest declares no requested native permissions.
