# PBLang declarative document browser

A working **offline local-document navigator**, authored in PBLang. It reuses
`browser.sapp`, `ui_document`, Modern UI, GUI, HTML, and existing Web/Android
exporters. It is not a replacement for a general HTML/CSS/JavaScript engine.

```sh
pblang run examples/declarative_browser/main.sapp --allow-all
pblang run examples/declarative_browser/main.sapp --allow-all -- --gui
pblang run examples/declarative_browser/main.sapp --allow-all -- --export
pblang run examples/declarative_browser/main.sapp --allow-all -- --web-preview
pblang run examples/declarative_browser/main.sapp --allow-all -- --android
```

The native application offers local links, Back, Forward and Home. The offline
export writes `build/declarative-browser/home.html`, `guide.html`, `about.html`;
open `home.html` and follow the generated relative links using your browser.
It has no JavaScript, server, external assets or network fetches. The separate
Modern UI web preview under `build/declarative-browser-web` uses the existing
bounded portable-action runtime to **inspect** destinations, not navigate.

## Write once, use both representations

```pblang
let D=import("stdlib:browser_document.sapp");
let doc=D.create("My reader",[
    map{tag:"h2",text:"Introduction"},
    map{tag:"p",text:"Written entirely in PBLang."},
    map{tag:"a",text:"Guide",href:"pb://guide"}
]);
let html=D.export_html(doc);
assert(D.import_html(html)==doc,"shared document roundtrip");
```

Or supply the exact inert HTML subset:

```html
<article><h1>My reader</h1><p>Hello</p><a href="pb://guide">Guide</a></article>
```

No nested markup, inter-tag whitespace, extra attributes, script, style, images,
forms, remote links, numeric entities or HTML recovery is accepted. Use `D.create`
for readable multiline application code; the exporter escapes text correctly.
The only entities are `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&#39;`. Encoded literal
markup remains text. Unsupported input throws before session construction.

`create` returns the existing `pblang.ui-document.v1` with a fixed normalized
layout. That canonical subset round-trips exactly; arbitrary imported GUI layouts,
models or handlers are rejected, not silently converted. Source strings/documents
are never rewritten. The Modern UI view uses responsive layout instead of preserving
those fixed coordinates on every display.

## Configure the session

```pblang
let S=import("stdlib:browser_session.sapp");
let session=S.create(pages,map{
    level:"local", home:"pb://home", allow:["pb://home","pb://guide"]
});
let current=session.follow(2); // zero-based block index, which must name a link
session.back(); session.forward(); session.home();
session.close();
```

`pages` maps explicit `pb://name` addresses to source strings. The owner supplies
the grants; page markup cannot enlarge them. Omitting `allow` grants the registered
pages. `locked` admits only home; `local` admits the declared local pages; `review`
additionally allows inert search URL proposals. None enables network, scripts,
Tor, download execution, authentication, cookies or native device permissions.
These are application admission rules, not an OS sandbox or user identity boundary.
To change policy, close the session and construct a new one.

The history is memory-only and capped at 32 entries. Pages, grants and returned
snapshots are detached. A failed navigation preserves the current page/history.
`close` clears the session-owned references and rejects later operations; it cannot
erase copies already returned to callers or files intentionally exported.

## Search without accidental requests

```pblang
let review=S.create(pages,map{level:"review"});
let request=review.search("PBLang Ελληνικά","duckduckgo");
assert(not request.effects and not request.executable);
let custom=review.search("local research","custom","https://search.example.com/?q=");
```

Existing DuckDuckGo, Brave, Startpage, Google and Bing templates are reused as
strings; their availability or privacy properties are not verified by this example.
Custom HTTPS query prefixes are explicit, bounded and lexically checked. Existing
PBLang URL/ad/tracker pattern checks are reused as advisory rejection rules. They
are not a complete threat feed, DNS validation, TLS enforcement or a networking
grant. No query leaves the process; no new search engine is implemented.

## Shared native/Web/Android representation

```pblang
let V=import("stdlib:browser_view.sapp");
let UI=import("stdlib:modern_ui.sapp");
let Android=import("stdlib:modern_ui_android.sapp");
let view=V.create(doc);
let android=V.plan(doc,"android");
let class_bytes=Android.activity_class(android.bundle);
```

The shared action shows a local destination in an output label. Text, Unicode,
ordering and this action are preserved across the desktop/Web/Android representations.
The native example explicitly substitutes session-controlled navigation callbacks.
Those closures are not portable Android/Web actions. `V.plan(doc,"ios").linked`
is false: iOS retains a declarative contract only. Use the explicitly requested
passive HTML export as a fallback; no silent target switch occurs.

`--android` invokes the existing Android builder and writes a signed debug APK to
`build/pblang-document-reader.apk`; it does not install it. The builder can also
package another shared view through
`UI.build_android("PB Reader",view,options)` if its SDK/signing prerequisites are
available. This example does not install a package or claim device execution.
No camera, microphone, location, NFC, filesystem or network authority follows from
a shared UI document. Existing GUI input routes are reused without modification.

## Limits and tests

16 registered pages; 16 KiB UTF-8 per source and 64 KiB total; 8 blocks/page;
80-character titles, 256-character text blocks, 64-character link labels;
32-character local names; 32 history entries; 256-character search queries;
2 KiB proposed URLs. Only literal local `pb://name` navigation is executable.

```sh
pblang run tests/semantics/browser_document_test.sapp --allow-all
pblang run tests/semantics/browser_session_test.sapp --allow-all
pblang run tests/semantics/browser_view_test.sapp --allow-all
xvfb-run -a -s '-screen 0 1000x1000x24 -nolisten tcp' env XMODIFIERS=@im=none python3 tests/tooling/declarative_browser_native.py
```

The X11 driver requires an existing retained runtime, xdotool and Xvfb. It uses
Tab/Enter for local-link → Back → Forward, checks the resulting session and closes
after 6.5 seconds. It refuses non-Xvfb displays. See the capability matrix in
[the browser contract](../../docs/DECLARATIVE_BROWSER.md).


## Reading appearance

The native reader uses one compact Back / Forward / Home / Style toolbar, even at
390px width. **Style** opens a separate reading settings screen; **Done** returns
to the same document. Family, body size, title size, text scale, line spacing and density are
session-local options by default. The optional `--with-profile` mode described below
adds explicit saving; no preferences are transmitted.

Opening **Style** focuses the font choice. Enter changes the focused option while
keeping it selected; Tab and Shift+Tab move between the available controls.
**Done** restores focus to **Style** on the same document. A held activation key
does not activate another option after a redraw. Programmatic appearance updates
also preserve an available, enabled button's focus and any held keyboard modifier.
Back, Forward and Home leave settings and display their destination while retaining
the chosen appearance. Removed, disabled or non-interactive controls do not receive
restored focus.

**Escape** in Style now returns to the same reading position and focuses **Style**,
including when no choice has focus or a compact options page is open. It discards
only unapplied draft choices, exactly like Done; Apply and Save results remain.
Holding Escape cannot turn dismissal into a window close. Once released, Escape
on the document keeps the existing behavior: clear focus first, then close on a
separate press with no focus. Actual native close/quit requests remain effective.
Later events in the same batch target the returned document, not stale settings.

Developers can call `App.close_preferences(reader)` and then `App.update(reader,[])`
to use the same dismissal. It returns false when already closed or the window has
a pending close request. It does not save/reload, change appearance or erase
Back/Forward positions. See `escape_demo.sapp`, the semantic
`reader_escape_preferences_test.sapp` and the source-hidden real-launcher
`reader_escape_preferences_native.py` journey (Pillow required for image comparison).

```pblang
let App=import("./app.sapp");
let reader=App.create("local","dark",map{
    font_family:"sans",body_size:16,title_size:28,
    caption_size:12,button_size:15,font_scale:1,line_height:1.45,density:"comfortable"
});
App.configure(reader,map{font_family:"serif",body_size:18});
App.refresh(reader);
```

`browser_view.create(document,appearance)` accepts the same options for native,
Web and Android presentation. Unknown keys are rejected. Sizes are integers
12–40, font scale is 0.75–1.5, line height is 1–2.5, density is `compact` or
`comfortable`. Invalid updates leave the existing settings intact. Per-element
choices can be applied to the resulting Modern UI tree using normal option maps.
The reader defaults opt into natural native spacing; existing unrelated GUI
applications retain their defaults. See [typography](../../docs/TYPOGRAPHY.md).

The controller remembers reading positions by default. Developers can call
`App.remember_positions(reader,false)` to retain first-page-only navigation, or
pass `true` to enable restoration again. The boolean-only option returns whether
the policy changed. A change clears remembered positions without moving the current
page, modifying appearance or writing a profile. The policy itself is session-only.
Use the controller's navigation callbacks; direct mutation/replacement of its
session is not a position-history API. Observable external session changes discard
the controller's remembered positions. Positions are not restored across launches.

`position_demo.sapp` is an executable, window-free example of the same callbacks.
`tests/semantics/reader_history_position_test.sapp` covers distinct visits, reflow,
branching, denied links, policy changes and the history cap. The Linux source-hidden
`tests/tooling/reader_history_position_native.py --package /absolute/release
--evidence /new/evidence` exercises the real launcher and records screenshots;
installed mode accepts `--install-root` and `--launcher` instead of `--package`.
This history journey also requires Pillow in the isolated system Python for pixel
comparison; it adds no dependency to the Reader application.

The native example automatically paginates a document when its current view would
exceed the window height, truncate wrapped text, or clip a document link label.
**Previous page / Next page**, **Page Up / Page Down**, **Home / End** navigate
reading pages. The toolbar's **Home** button still navigates to the session home.
Held page keys advance once per press. The original view remains when it fits.

Changing appearance or resizing retains a logical text position (block and offset),
so a changed page count does not move the reader to unrelated text. Local navigation
starts at the destination's first page. Back and Forward restore a separate logical
reading position for each history entry, including repeated visits to the same
document. Positions remain memory-only, bounded by the existing 32-entry session
history; new navigation after Back discards the forward positions. Home deliberately
starts at the beginning. A link's complete label is readable text
followed by **Open link**, which uses the original session-validated block index.

Document text obeys the selected sizes; pagination chrome uses compact 14px buttons
and 12px status text. The planner reuses PBLang's bounded Unicode text flow and
reserves space for navigation. If a text cluster cannot fit the supported width or
a single line cannot fit the available height, it explicitly asks for more space or
a different style rather than silently omitting content. Blank text/button anchors
remain distinct. This does not introduce a general scrolling container or enlarge
the document limits above. The settings screen also adapts to short windows as described below.

Developers can use the pure `pagination.sapp` planner without opening a window:

```pblang
let P=import("./pagination.sapp");let V=import("stdlib:browser_view.sapp");
let layout=P.plan(doc,V.appearance(map{body_size:20,font_scale:1.5}),390,844);
let position=P.anchor(layout,1); // zero-based page; check layout.available first
let resized=P.plan(doc,V.appearance(map{body_size:20,font_scale:1.5}),320,480);
let page=P.locate(resized,position);
App.read_page(reader,reader.reading_page+1); // native controller; clamped to its page count
```

`needs`, `plan`, `anchor`, `locate` and `view` are example-local helpers, not changes
to the document interchange contract or shared renderer API. Native row reservation
remains conservative, and bitmap fallback cannot emulate arbitrary system fonts. Web/Android use platform-owned fonts and may break lines
differently. The inert `--export` HTML continues to use its existing fixed style;
customizable presentation is provided by `--web-preview` and the shared view.

Additional tests: `tests/semantics/readable_typography_test.sapp` and
`tests/tooling/readable_browser_native.py` (isolated Xvfb, optional
`PBLANG_READER_WIDTH=390`). The latter checks navigation plus keyboard-only font
changes and records default/serif screenshots.

`tests/semantics/reader_preferences_focus_test.sapp` checks focus restoration,
held keys, reverse navigation, rejected options and navigation from settings.
The Greek 390×844 keyboard journey can be reproduced with:

```sh
xvfb-run -a -s '-screen 0 1000x1000x24 -nolisten tcp' env XMODIFIERS=@im=none python3 tests/tooling/reader_preferences_native.py
```

It writes synthetic state and screenshots under `build/reader-focus-*` and closes
its own window. These focus rules belong to the native example; the exported
Web/Android presentation does not implement the example's preference callbacks.


Pagination tests cover complete text coverage, canonical link indices, page bounds,
held keys, resize/appearance anchors and explicit unsupported layouts:

```sh
pblang run tests/semantics/reader_pagination_test.sapp --allow-all
xvfb-run -a -s '-screen 0 1000x1000x24 -nolisten tcp' env XMODIFIERS=@im=none python3 tests/tooling/reader_pagination_native.py
```

The native journey uses synthetic Greek/English text, large type at 390×844,
keyboard settings changes, resize to 320×480 and last-page local navigation. It
writes `build/reader-pages-*.png` and its own state file. Web/Android exports retain
the shared view and portable destination-inspection action; they do not gain the
native controller's pagination callbacks. Windows/macOS/iOS/Android device execution
is not established by these Linux/Xvfb tests.


## Preferences in short windows

When the original **Style** screen would overflow or truncate its text, it switches
to a compact layout. All six choices remain available: font family, body
size, title size, text scale, line spacing and density. The layout shows one choice at the
minimum 320×240 viewport, two at 320×288, and up to six as height allows.
**Previous options / Next options**, Page Up/Down and Home/End navigate these option
pages while Style is open; document reading position is unchanged. Changing pages
focuses the first enabled choice on the destination page. A page containing only
an unavailable Save action is skipped in the requested direction, so Apply and
Reset remain reachable even at 320×240. A disabled Save sharing a page with Reset
does not redirect navigation to a different page. The separate recovery behavior
after a failed Save still focuses Reload (or Apply when Reload is unavailable).
Tab/Shift+Tab and Enter/Space retain their
existing control behavior, and held activation keys do not change a newly focused
choice unintentionally. Done remains in the top toolbar and returns to the document.

Resizing keeps the currently focused choice visible. If a page navigation button
becomes disabled or disappears in a larger layout, focus returns to the remembered
choice. Opening Style starts at the font choice. When the original screen fits,
its previous choice layout is preserved. The Style toolbar always uses compact
14px sans text. Compact choice controls use 14px text and fixed spacing
so even extreme document typography cannot hide the options needed to change it.
The legacy controller keeps session-local preferences without storage or network
effects. The native launcher additionally exposes the optional profile workflow
described below; its default remains session-only.

Developers may call `App.settings_page(reader,index)` (zero-based, clamped, false
when unavailable or unchanged). Disabled-only pages resolve forward or backward
relative to the current page; paging does not apply, save, reset or reload any
settings. See `disabled_page_demo.sapp` for an executable read-only example.
The example-local `settings_layout.plan(height,
anchor, total=6)` computes capacity and visible indices without opening a window;
anchor is a zero-based choice index, and height uses the existing GUI bounds
240–4320. The optional preferences workflow uses `total=9` for its actions, or `total=10` with explicit reload. Shared GUI
contracts and Web/Android exporters remain unchanged.

```sh
pblang run tests/semantics/reader_compact_settings_test.sapp --allow-all
xvfb-run -a -s '-screen 0 1000x1000x24 -nolisten tcp' env XMODIFIERS=@im=none python3 tests/tooling/reader_compact_settings_native.py
```

The isolated native test changes all six options using real keyboard input at
320×240, enlarges and shrinks the window, checks focus and held keys, then returns
to the same document. Synthetic screenshots are written to `build/reader-settings-*`.
This establishes Linux/Xvfb behavior, not execution on other physical platforms.

## Large text and actual native font widths

The native `--gui` entry point now supplies its window's existing `textMeasure`
function to the reader. If a naturally wide glyph line would exceed its width,
the reader refines the existing text-flow lines at word/Unicode-cluster boundaries
before making pages. This prevents a line from passing the approximate planner and
then being replaced with ellipses by the native renderer. Original whitespace
normalization, canonical source, local-link policy and bitmap fallback budgets are
retained. Each block still has the existing bounded line budget; an unfit cluster
or exhausted budget produces an explicit unavailable layout.

Applications using the controller directly can opt in with the same window:

```pblang
App.set_text_measure(reader,fn(text,size,family) {
    return window.textMeasure(text,size,0,family);
});
App.refresh(reader);
```

The callback is trusted application code and must return the same `{ok,width,height}`
metrics as the owning window for natural spacing. It is never imported from a
document. `App.set_text_measure(reader,nil)` removes it. Without a callback, or when
host measurement is unavailable or too tall for the renderer's line budget, the
existing approximate/bitmap route remains. This does not add universal font shaping
or device guarantees. Use a live owning window, and refresh after replacing the
measurement provider. The pure `pagination.needs` and `pagination.plan` helpers
accept the callback as an optional last argument.

Page status, recovery help, link controls and the local-only footer use compact
line spacing independently of document line spacing. At 320×240 with large document
text, the complete instruction to enlarge the window or change Style stays visible.
Resizing back to a usable layout retains the reading anchor.

```sh
pblang run tests/semantics/reader_text_fit_test.sapp --allow-all
xvfb-run -a -s '-screen 0 1000x1000x24 -nolisten tcp' env XMODIFIERS=@im=none python3 tests/tooling/reader_text_fit_native.py
```

The new Xvfb fixture measures an actually too-wide baseline, then records successful
native text-drawing calls while reading all measured pages. It checks wide Latin
and Greek content, complete help/footer rendering, resize recovery and the final
local link. Synthetic images are under `build/reader-measured-*`. Shared renderers
and Web/Android exports are unchanged; these checks establish Linux behavior only.

## Standalone Linux reader candidate

This example can produce a new portable folder containing the existing reader,
its literal-import library closure, a matching local dependency lock, and an
explicitly selected Linux x86-64 PBLang runtime. It uses the existing PBLang-owned
`stdlib:package_folder.sapp` API (the same route as `tools/pbpackage.sapp`). Its
public `package.sapp` entry and `package_builder.sapp` workflow are PBLang; the
existing PBLang tokenizer discovers literal imports. Python is only used by the
external acceptance harness. No native
`pblang bundle` command is invoked, and the checkout manifest/lock are not edited.

```sh
# Run from the repository. Choose a NEW private output parent.
mkdir -m 700 ../reader-candidates
target/release/pblang run examples/declarative_browser/package.sapp --allow-all -- \
  "$PWD" "$PWD/../reader-candidates/reader-v2" \
  --runtime "$PWD/target/release/pblang"

# These work from a different current directory, using only the candidate.
sh ../reader-candidates/reader-v2/run-reader.sh --check
sh ../reader-candidates/reader-v2/run-reader.sh
```

The default launch opens the native GUI, including actual font measurement and
the existing Style controls. `--check` exercises offline document navigation and
roundtrip without a display. `--help` prints usage, including the opt-in persistence mode; unsupported arguments are rejected.
Default GUI, `--session-only` and `--check` retain `--deny-net`, `--deny-http`, `--deny-exec` and `--deny-spawn`. Explicit `--with-profile` enables process execution for the fixed system `stat` utility used by the existing directory API; it retains the other three flags. Native GUI
still needs local graphics/FFI and filesystem access for libraries/fonts; this is
not an application sandbox. Invoke the launcher with `sh`: the package contract
keeps source files non-executable and only marks the runtime executable.

The output parent must already be private (0700). Both the destination and its
`NEW_OUTPUT.source` staging sibling must be absent and outside the checkout.
Existing outputs/staging are never replaced or removed. The workflow accepts this
example's literal imports only, with at most 128 modules, 1 MiB and 65,536 tokens
per module, and 8 MiB of module bytes; the packager's additional limits still apply.
It captures trusted local sources through the existing no-follow directory API,
not concurrently hostile inputs. Run the public tool from the repository's
supported PBLang environment; its existing bootstrap dependency setup still
applies. The generated application's manifest and local lock are independent of
the repository's dependency paths. The finished reader does not need the checkout.

The `reader-source-sha256.json` inventory records copied modules. The normalized
`pblang.lock` binds the included stdlib tree; `package-receipt.json` describes
publication, not acceptance or runtime authenticity. Select a trusted runtime.
On failure a partial destination may remain: inspect it and use a new destination
for a later attempt. Staging is explicitly reported and retained on success or
failure for inspection; this workflow never recursively deletes it. Preserve an
earlier accepted package until the new candidate passes acceptance.

Accept the candidate before distributing or activating it:

```sh
python3 tests/tooling/reader_package.py \
  --package ../reader-candidates/reader-v2 \
  --evidence ../reader-candidates/reader-v2-acceptance
```

Acceptance requires Linux with Python 3.9+, bubblewrap, Xvfb, xauth, xdotool,
ImageMagick, the native runtime's system libraries and usable system fonts. The
evidence directory must be new. Only OS directories, the read-only candidate and
synthetic test fixtures are mounted into the isolated execution environment; the
original checkout is absent. Checks cover launch from a foreign working directory,
the semantic text-fit suite with effects denied, GUI launch and the full measured
text/resize/navigation journey. Candidate hashes must remain unchanged. The test
does not install a launcher, change a desktop entry or activate an existing product.

This is a Linux folder candidate with readable source, not a source-concealing
single executable, a universal binary or a Windows/macOS/mobile installer. The
host's compatible native libraries/fonts remain requirements. Web and Android
exports remain available through the original source example; this launch wrapper
exposes the native reader only.

`tests/semantics/reader_package_builder_test.sapp` checks the PBLang import
scanner, path admission and v2 digest (including Unicode and empty directories)
without filesystem, network, FFI or process effects. The native runtime and
standard OS libraries are retained dependencies. The small shell launcher is
only an OS handoff to the included runtime, not a packaging implementation.

### Dedicated versioned Linux installation

The additive `install.sapp` / `install_manager.sapp` workflow is written in
PBLang and ships in the candidate's import closure. It owns only an explicitly
chosen root and the `pb-document-reader` launcher in an explicitly chosen bin
directory. It does not update global PBLang, desktop entries, other products or
old candidate archives. A typical local contract is:

- `~/.local/share/pb-document-reader/releases/<version>`: pinned complete copies;
- `~/.local/share/pb-document-reader/accepted/<version>.json`: retained acceptance;
- `~/.local/share/pb-document-reader/data`: reserved user data, never modified by
  upgrade/rollback;
- `~/.local/bin/pb-document-reader`: executable containing current and previous
  version IDs and a shell handoff to the selected release.

Both parent directories must already exist; the installer creates a private
0700 root and refuses to adopt an existing unmarked root. Version IDs are 1–64
ASCII letters/digits/dots/underscores/hyphens, start with a letter/digit, and
cannot contain `..`. Existing versions are never overwritten. Directory and
file access uses the existing Linux no-follow, descriptor-bound helpers;
source snapshots are copied with exclusive publication. Inputs are trusted local
packages, not a defense against concurrent hostile processes running as you.

From any working directory, using an already accepted candidate:

```sh
# PIN is sha256(base64(runtime bytes)), with no base64 line wrapping.
# Obtain it independently from the trusted runtime, not from an unknown package.
# This digest format uses the existing streaming Dir.held_base64_sha256 API.
sh /absolute/candidate/run-reader.sh --manage prepare \
  /absolute/candidate /home/USER/.local/share/pb-document-reader VERSION PIN
sh /absolute/candidate/run-reader.sh --manage verify \
  /home/USER/.local/share/pb-document-reader VERSION
```

`prepare` verifies the package/module inventory, exact file set, Linux x86-64
ELF header and the supplied runtime pin. It snapshots all files, copies to a new
release and writes `reader-release.json` last. The release record pins **all**
files, including the runtime, launcher, manifest, dependency lock and inventory.
Its fingerprint also binds the version ID. Maximums are 128 files, 192 directory
entries, depth 8, 1 MiB per source/metadata file, 64 MiB runtime and 72 MiB total.
Installed source files are 0600 and runtime 0700. `verify` recaptures file bytes,
checks pins/permissions and revalidates package admission; it does not execute
the GUI or certify the runtime's origin.

Before activation, run `tests/tooling/reader_package.py` against the **prepared
release** and inspect its native `reader-style.png`, `reader-first.png` and
`reader-last.png`. The harness mounts that release read-only with the original
checkout absent, exercises real launch/Style/pagination plus measured rendering,
and verifies unchanged content. Copy its verified version/fingerprint from
`reader-release.json` into an acceptance JSON file, only after all checks pass:

```json
{
  "format": "pblang.reader.acceptance.v1",
  "version": "VERSION",
  "fingerprint": "64 lowercase hexadecimal characters from the verified record",
  "passed": true,
  "checks": {
    "source_hidden": true,
    "launcher": true,
    "style": true,
    "pagination": true,
    "content_unchanged": true
  }
}
```

This is a **trusted caller attestation**, not a signature, independent auditor or
automatically authenticated test result. The installer enforces its required
fields and exact release binding. Keep actual logs/screenshots separately.

```sh
sh /absolute/candidate/run-reader.sh --manage activate \
  /home/USER/.local/share/pb-document-reader VERSION \
  /home/USER/.local/bin /absolute/acceptance.json
/home/USER/.local/bin/pb-document-reader
/home/USER/.local/bin/pb-document-reader --check
/home/USER/.local/bin/pb-document-reader --manage status \
  /home/USER/.local/share/pb-document-reader /home/USER/.local/bin
/home/USER/.local/bin/pb-document-reader --manage rollback \
  /home/USER/.local/share/pb-document-reader /home/USER/.local/bin
```

Activation validates the target and the current release that becomes the rollback
baseline. The first activation has no previous version: for an initial rollout,
prepare/test/accept a separate recovery snapshot, activate it first, then activate
the separately prepared/tested current version. Two copies of the same candidate
are a recovery snapshot, not evidence of an upgrade from an older product.
Rollback verifies the previous copy and its retained receipt, then swaps current
and previous. It can recover from damaged current files without verifying those
files; a second rollback returns to the other copy if it still verifies. If the
active runtime/manager itself cannot start, invoke a retained good release:

```sh
sh /home/USER/.local/share/pb-document-reader/releases/RECOVERY/run-reader.sh \
  --manage rollback /home/USER/.local/share/pb-document-reader /home/USER/.local/bin
```

A private advisory lock serializes this installer per root; another operation
fails busy rather than waiting. The one authoritative launcher contains both
IDs. A new executable is staged beside it before publication; initial publication
uses no-replace rename and subsequent selection uses atomic rename. A foreign or
modified launcher is refused. Selection is atomic for ordinary readers, but the
whole installation is **not** a transaction or a power-loss durability guarantee.
Partial releases and staged launchers are retained on failure; nothing recursively
deletes them. If publication is reported attempted but unconfirmed, inspect
`status` before any retry. Do not run separate roots against the same launcher.
The normal launch does not hash every file on each start; use `verify`/`status`
after suspected modification. Default GUI preferences remain per session; opt-in saved appearance is described below.

`tests/semantics/reader_install_test.sapp` covers pure identity/launcher/fingerprint
contracts. `tests/tooling/reader_install.py` checks transitions, no-overwrite,
foreign state, locking, tampering, source-hidden management and data preservation
in temporary roots. Its synthetic receipts test the mechanism and **do not**
constitute acceptance of a real installation. Python remains test-only; normal
prepare/verify/activate/status/rollback run through the packaged PBLang manager.
Management enables filesystem/FFI and process support for the existing PBLang
directory helpers (including native no-replace publication); it explicitly denies
network/HTTP. This maintenance route is not an application sandbox. No telemetry,
credential store or remote service is added. Only Linux x86-64 is covered by this
local installer.

After activating or rolling back, test the exact installed launcher with:

```sh
python3 tests/tooling/reader_installed.py \
  --root /home/USER/.local/share/pb-document-reader \
  --bin-dir /home/USER/.local/bin --evidence /absolute/NEW_EVIDENCE
```

This read-only native journey mounts just the selected release, its launcher,
OS dependencies and synthetic test driver; the checkout and user data are absent.
It opens Style, changes font/size/scale/spacing, resizes to 320×480 and goes to the
last reading page. Inspect the resulting screenshots separately; successful key
injection alone is not a visual correctness assertion. The driver checks that
release files and the launcher remain unchanged.

### Optional persistent reading appearance

The native reader keeps session-only behavior by default. Start it with
`--with-profile` to opt in to loading and saving local preferences. The default
and `--session-only` routes skip profile discovery/read/write entirely and show
Save disabled. The UI separates choices from application and persistence:

- Changing a Style control edits a draft. **Done** and **Escape** discard unapplied choices.
  Successful **Home**, **Back** or **Forward** navigation from Style uses the same
  dismissal: the draft is immediately reset to the applied appearance and a real
  discard reports `Unapplied changes discarded` when Style is reopened. Existing
  status messages remain when no draft differs. Navigation never saves or reloads
  preferences, and applied/saved choices and normal reading history are preserved.
  Disabled navigation does not dismiss a draft. See `navigation_draft_demo.sapp`
  for a runnable example using the existing controller and toolbar callbacks.
- **Apply for this session** applies the draft without touching profile storage.
- **Save for next launch** saves and applies the draft explicitly.
- **Reset defaults** saves and applies the original reader defaults. When storage
  is unavailable, the label becomes **Reset this session** and Save is disabled.

Startup only reads. Missing settings use the existing defaults. Invalid, unknown,
noncanonical or oversized profiles cannot silently change the appearance or be
rewritten during startup. The UI reports fallback to defaults or session-only
operation. Save failures keep the chosen draft and the currently applied style;
an uncertain publication disables further Save until an explicit successful reload or a restart checks the disk.

With `--with-profile`, installed releases discover their own marked installation root and use
`data/reader-style/appearance-v1.json`. Releases and recovery snapshots remain
separate from preferences. No global PBLang setting, network or telemetry is used.
The default and `--session-only` launcher routes skip all profile discovery/read/write
and retain the earlier process-execution denial. Explicit `--with-profile` enables
process execution because the existing directory API validates held descriptors through
fixed `/usr/bin/stat` or `/bin/stat` calls. This is a capability grant to trusted
application code, not an OS-enforced command allowlist or sandbox; filesystem/FFI
are also required for native graphics. No arbitrary user command comes from a
profile. The `--deny-net`, `--deny-http` and `--deny-spawn` flags remain in place.

With the opt-in flag, a standalone candidate still remains session-only unless
the developer supplies an
existing private 0700 data directory **outside the application directory**:

```sh
PBLANG_READER_DATA_DIR=/absolute/private/data sh /absolute/candidate/run-reader.sh --with-profile
```

The environment override is process-local; no directory or profile is created
until explicit Save/Reset. The manager creates a dedicated private `reader-style`
child and refuses to adopt an unmarked existing child. Source-hidden installed
acceptance overlays the data path with synthetic test data, leaving real user
files untouched.

`appearance_profile.sapp` reuses `browser_view.appearance` for the eight existing
appearance fields. The exact v1 record has `format`, `version` and `appearance`,
is at most 4096 UTF-8 bytes, and stores all eight validated fields. It accepts
canonical output from `Profile.encode(style)` (surrounding whitespace allowed).
Unknown/missing fields, unknown versions, duplicate keys, noncanonical JSON and
invalid values fall back to defaults. Use the encoder to generate editable
profiles through PBLang; arbitrary pretty-printed JSON is not the storage format.

`appearance_store.sapp` uses existing `linux_dirfs` descriptor-bound reads,
private files, advisory locking and atomic replacement with fsync. Save compares
the revision originally loaded by that window before replacing anything. A stale
window cannot overwrite another window's newer preferences. Previous bounded
bytes—including invalid UTF-8/JSON—are preserved in content-addressed
`previous-<revision>.json` backups before an explicit replacement. Backups are
never automatically deleted: at most 16 distinct previous records are admitted;
further changes fail without replacing the current profile until old backups are
explicitly archived by the owner. The whole folder is capped at 64 entries for
admission. Symlinks, hard links, nonprivate or oversized records are refused and
preserved. A failed backup may remain for inspection; no recursive cleanup or
automatic retry is attempted. This is trusted same-user local storage, not a
hostile-concurrent-process or universal-filesystem guarantee.

Developer integration is additive: `App.create()` and `App.configure()` retain
their original pure/session behavior. Opt in with
`App.enable_preferences(controller, appearance, save_callback, message, reload_callback=nil)`.
The callback receives the complete validated style and returns an `ok` result;
`attempted` signals uncertain publication and `reload` signals a stale revision.
The app owns the draft/Apply/Save/Reset interaction; storage remains injectable.
The standalone `appearance_store` owns local I/O, while `appearance_profile` and
controller tests remain usable with filesystem/network/FFI effects denied.
The new actions also participate in the existing bounded settings pagination.

#### Explicit reload without restart

With `--with-profile`, **Reload saved style** reads the selected profile once. A
successful reload replaces both the draft and the currently applied appearance
with the valid saved style, and renews the revision used by the next Save. The
button deliberately discards unapplied choices; use Apply first only if you want
to keep them temporarily before deciding to reload. Reload never writes a file,
creates a profile, or retries a failed Save. There is no background polling.

After a stale or uncertain Save, Save stays disabled until reload succeeds. If
the profile is missing, invalid, oversized, nonprivate or otherwise unavailable,
reload preserves the current applied style **and** draft, reports the failure,
and leaves Save disabled. Repair/restore the external profile and reload again,
or restart to use the existing startup fallback/explicit first-save behavior.
The original startup defaults and explicit invalid-profile repair remain unchanged.
Another writer can still change the profile immediately after a successful read;
the existing revision check rejects the next stale Save again. Local advisory
locking and trusted same-user assumptions are unchanged.

The optional reload callback takes no arguments and must return
`{ok:true, appearance:complete_validated_style, save:renewed_save_callback}`.
Malformed results and thrown errors preserve both styles and disable Save. The
additive `App.reload_preferences(controller)` performs the same explicit action
and returns a boolean. No callback means no Reload button; previous four-argument
callers and default/session-only launchers retain their earlier behavior. The
native adapter reuses `Store.load` and `Store.save`; it reloads the chosen location
without rediscovering environment overrides or changing launcher capabilities.
All ten controls remain available through the existing compact settings pages.

```sh
pblang run tests/semantics/reader_reload_test.sapp --allow-all --deny-fs --deny-net --deny-http --deny-exec --deny-spawn --deny-ffi
python3 tests/tooling/reader_reload_native.py --package /absolute/candidate --evidence /absolute/NEW_RELOAD_EVIDENCE
python3 tests/tooling/reader_reload_native.py --install-root /home/USER/.local/share/pb-document-reader --launcher /home/USER/.local/bin/pb-document-reader --evidence /absolute/NEW_INSTALLED_RELOAD_EVIDENCE
```

The reload journey runs a second PBLang writer against synthetic data, induces a
GUI conflict, reloads and saves without restarting that GUI process. It checks
repeated invalid/UTF-8/oversized/missing reads without writes, renewed conflicts
after external churn, restart persistence, and default deny-exec/no-write behavior.
It reuses the source-hidden package/installed harness; review its screenshots.

Tests for this extension:

```sh
python3 tests/tooling/reader_appearance_store.py \
  --runtime target/release/pblang --evidence /absolute/NEW_STORE_EVIDENCE
python3 tests/tooling/reader_appearance_native.py \
  --package /absolute/candidate --evidence /absolute/NEW_GUI_EVIDENCE
python3 tests/tooling/reader_appearance_native.py \
  --install-root /home/USER/.local/share/pb-document-reader \
  --launcher /home/USER/.local/bin/pb-document-reader \
  --evidence /absolute/NEW_INSTALLED_EVIDENCE
```

The native journey exercises Save → restart, temporary Apply → restart, Reset →
restart and invalid-profile fallback through the real GUI. Inspect screenshots
as well as machine-readable results; successful key injection is not a visual
assertion. Only Linux x86-64/X11 is covered by this persistence adapter.


### Stronger native focus indicator

The native launcher now uses `App.draw(reader, canvas)`. After the unchanged GUI
renderer draws the form, a reader-local three-physical-pixel inset frame identifies
the focused, enabled, visible button. `focus_indicator.sapp` uses existing
`ui_style.button` to resolve the actual fill and `colors.best_text_checked` to
choose black or white against that opaque 8-bit sRGB fill. This also works with
selected buttons and the existing light/dark palettes. The frame remains inside
the widget and its clip/viewport; it does not change hit areas, text layout,
keyboard handling, controller state or saved preferences. Drawing adds at most
four filled rectangles per dirty frame.

Developers can adopt the additive `App.draw(reader, canvas, highlight=true)`
callback or pass `false` to keep the previous visual indicator. Direct `GUI.draw`
callers retain their existing behavior. Missing, hidden, disabled, fully clipped
or tiny custom buttons and unresolved/translucent colors keep the shared fallback.
The indicator is not serialized into appearance profiles. Default session-only,
explicit Save, conflict checks and Reload remain unchanged.

The baseline reader's primary-button focus was one native pixel at approximately
2.02:1 against its fill. The compact native test verifies a three-pixel white
frame at approximately 4.86:1, its removal on Tab and restoration on Shift+Tab,
then exercises large text, page navigation and clearing focus. Pure tests cover
both themes, other button variants, selected states, grayscale colors and clipping.
The W3C [Focus Appearance guidance](https://www.w3.org/WAI/WCAG22/Understanding/focus-appearance)
informs the size/contrast goal. This is measured Linux/X11 behavior in physical
pixels, not a claim of complete WCAG conformance, CSS-pixel equivalence, screen
reader integration, native forced-colors support or verification on other platforms.

```sh
pblang run tests/semantics/reader_focus_indicator_test.sapp --allow-all --deny-fs --deny-net --deny-http --deny-exec --deny-spawn --deny-ffi
python3 tests/tooling/reader_focus_indicator_native.py --package /absolute/candidate --evidence /absolute/NEW_FOCUS_EVIDENCE
python3 tests/tooling/reader_focus_indicator_native.py --install-root /home/USER/.local/share/pb-document-reader --launcher /home/USER/.local/bin/pb-document-reader --evidence /absolute/NEW_INSTALLED_FOCUS_EVIDENCE
```

The native driver reuses the source-hidden synthetic-data harness and additionally
requires Pillow for read-only screenshot pixel inspection. Inspect the screenshots
as well as the machine results; the profile/Reload journey remains a separate test.

### Large imported blocks and recoverable pagination

A valid imported 256-character heading can need more than 64 rows after native
font measurement at maximum magnification. The reader now requests an explicit
256-row refinement budget for each canonical block. `text_fit.lines` retains its
64-row default for existing callers and accepts an optional final `max_rows`
integer from 1 to 256 for measured refinement. Shared `text_flow`, Unicode
clustering, document admission limits and native input remain unchanged. A truly
unfittable text cluster still produces an unavailable layout.

The existing overall limit stays at 512 pages. When a narrow/short viewport and
large text exceed it, the reader presents its existing space/Style recovery view
instead of throwing out of the application. It keeps the canonical document and
logical reading anchor; enlarging the viewport recomputes the complete layout.
No document is truncated or partially presented as complete. No new import UI,
file access or persistence field is introduced: imported documents use the
existing `browser_session`/`browser_document` interfaces.

`reader_large_document_test.sapp` covers native-like wide glyphs, combining groups,
complete title/block/link coverage, budget validation, anchor recovery and the
maximum eight-block document. The native test imports synthetic HTML into the
real reader controller and uses a real window font measurement callback. It
visits every resulting page, verifies exact text draw calls, checks Style and
focus across resize/reflow, and opens the final canonical local link. Its test
fixture writes only to an isolated probe directory; the package is mounted
read-only and the checkout/home are absent. `--baseline` explicitly reproduces
the old failure and does not claim successful reading.

```sh
pblang run tests/semantics/reader_large_document_test.sapp --allow-all --deny-fs --deny-exec --deny-net --deny-http --deny-spawn
python3 tests/tooling/reader_large_document_native.py --package /absolute/reader-release --evidence /absolute/NEW_EVIDENCE
```

This is Linux/X11 evidence for the installed modules and test-owned imported
session. The normal launcher still opens its existing bundled documents; its
separate focus/launch acceptance remains applicable. Other platforms, arbitrary
HTML, unlimited documents and physical screen-reader support are not claimed.

### Open a supported document from the real launcher

```sh
pb-document-reader --open "./My local document.html"
pb-document-reader --with-profile --open "./My local document.html"
pb-document-reader --check --open "./My local document.html"
```

Standalone packages accept the same arguments through `sh /path/run-reader.sh`.
Relative paths are resolved from the caller's directory before the launcher moves
into its application folder. Spaces and Unicode names are supported. Paths are
normalized lexically; symlinks in the remaining parent path or final filename are
refused. Control characters, backslashes and paths over 4096 characters are
unsupported. The reader never evaluates the filename as shell code.

`--open` reads at most 16 KiB through the existing Linux held-descriptor bounded
snapshot API. It rejects invalid UTF-8 and validates the existing inert document
subset before creating a window or loading preferences. Outer whitespace is
accepted; the byte limit is checked before trimming. The existing limits remain:
80 title characters, eight blocks, 256 characters per text/heading block, 64 per
link label, and registered `pb://` navigation only. No scripts, remote loading or
arbitrary HTML engine is introduced. See `import-example.html` for a complete
example; the example is in the repository, not an extra installed package file.

The document replaces `pb://home` only in the new session. Home/Back can return
to it after visiting the built-in guide. Unknown local destinations remain denied.
Each invocation is independent: a rejected file exits unsuccessfully and does not
replace an already open window, write the input, or change saved preferences.
There is no single-instance forwarding, hot reload or automatic file watching.

`--check --open FILE` validates without a display and prints only format, byte
count and block count, without the source text, title or filename. Default
`--check`, ordinary demo startup, `--session-only` and `--manage` remain available.
Import in session-only/check mode retains `--deny-exec`, `--deny-net`,
`--deny-http` and `--deny-spawn`. The existing explicit `--with-profile` option
still permits trusted stat subprocesses for profile operations; the launcher is
not an OS command allowlist or full sandbox. Read-only hard links are permitted.
A bounded snapshot limits allocation but is not a transactional snapshot against
a writer modifying the same file concurrently.

Developer entry points are `document_file.decode(bytes)` / `load(absolute_path)`
and the optional fourth `source` argument of `App.create`. Existing calls retain
the demo. The same session/parser contracts validate programmatic sources.

```sh
pblang run tests/semantics/reader_document_file_test.sapp --allow-all --deny-fs --deny-exec --deny-net --deny-http --deny-spawn
python3 tests/tooling/reader_document_file_cli.py --package /absolute/package --evidence /absolute/NEW_CLI_EVIDENCE
python3 tests/tooling/reader_document_launcher_native.py --package /absolute/package --evidence /absolute/NEW_GUI_EVIDENCE
python3 tests/tooling/reader_document_launcher_native.py --install-root /home/USER/.local/share/pb-document-reader --launcher /home/USER/.local/bin/pb-document-reader --evidence /absolute/NEW_INSTALLED_EVIDENCE
```

The new tests invoke the actual launcher with `--open`; they do not substitute a
controller/session fixture. They use synthetic input/profile data and a read-only
package with the original checkout absent. Inspect GUI screenshots as well as
machine results. File import through this launcher is verified on Linux x86-64;
no Windows/macOS/mobile file-picker or launcher support is claimed.

### Understand and recover from an import error

A rejected file now includes a specific recovery hint rather than one identical
message for every failure. `--check --open FILE` also emits a single JSON object
on stdout with `ok: false`, the existing `pblang.reader.document.v1` format,
`code` and `error`, then exits nonzero. Successful JSON is unchanged. GUI launch
failures print the same human-readable hint on stdout before creating a window or
loading the profile; they do not open an error dialog or replace another reader window.
Expected admission failures exit with status 1 through the existing `os_exit`
primitive, after file handles have closed. They emit no stderr stack trace or
installation path. Tools should inspect the exit status and the documented stdout
result, rather than the old interpreter exception on stderr.

| Code | Recovery |
| --- | --- |
| `empty_document` | Add a supported article and title; whitespace-only input is empty. |
| `too_large` | Reduce the UTF-8 file to at most 16384 bytes, including outer whitespace. |
| `invalid_utf8` | Save the file as UTF-8 and retry. |
| `unsupported_document` | Use the supported markup and existing field limits. |
| `invalid_path` | Use an absolute normalized path without control characters/backslashes; the launcher resolves relative paths. |
| `read_failed` | Choose a readable regular file without symbolic links; the exact OS failure is not exposed. |

`document_file.decode/load` return the same codes in an additive `code` field;
success uses `ok`. `decode` additionally reports `invalid_input` for a non-array
argument. Existing `ok/source/bytes/blocks/error` fields remain. Rejected results
have no source; byte/block values remain zero and are not measurements of the
rejected input. Syntax/argument errors before admission still use the existing
usage/error path and are not promised document JSON. No source, title, input filename,
parser exception or OS error is copied into diagnostics. Unknown read failures
stay `read_failed`; the existing bounded-reader byte-limit result maps to
`too_large` without a second read or changing its allocation limit.

For example, replace a document containing a `<script>` element with this
complete supported document (or start from `import-example.html`):

```html
<article><h1>My document</h1><p>Local readable text.</p><a href="pb://guide">Guide</a></article>
```

Validate with `pb-document-reader --check --open ./document.html`, then open
with `pb-document-reader --with-profile --open ./document.html`. Changing
appearance never repairs unsupported source; correct the file and launch again.
The real-launcher tests also reject malformed/unsupported/oversize/invalid-UTF-8
files while a saved-profile reader remains on its last page, verifying unchanged
profile bytes and screenshots as well as continued navigation. An invalid saved
profile is also tested with a valid imported document: reading remains available,
the Style status explains the fallback and the invalid profile is not rewritten.


## Independent title size in Style

**Title size** cycles through 20, 24, 28, 32, 36 and 40; the default remains 28.
It changes the article's main title independently of **Text size** (paragraphs and
section headings). **Text scale** still multiplies both sizes. The displayed number
is the base size before that scale. At 320×240 the title is the third option page;
at 320×480 all six appearance choices fit together and actions are on the next page.

Apply, Save, Reload and Reset include the title using the existing v1 profile.
Apply preserves the reading anchor and does not write the profile; reflow can change
the page number. Done discards an unapplied draft. Saved styles work with `--open`
on a new document, and local navigation retains the applied style. Reset restores
28 along with all other defaults. Default/session-only launches still ignore saved
profiles. No source document is rewritten.

Developers can continue using `App.configure(reader,map{title_size:27})` or supply
`title_size` in the appearance passed to `App.create`. All previously accepted
integer sizes 12–40 remain valid and are preserved without rounding to presets.
Explicitly activating Title size from a non-preset value starts the sequence at 20.
The profile schema, shared typography, renderers and packaging contracts are unchanged.

`reader_title_size_test.sapp` covers draft/apply/save/reload/reset, imported content,
navigation, a new document, custom values and paginated reflow.
`reader_title_size_native.py` exercises the real packaged/installed launcher with
synthetic imported files and isolated profile data, including 320×480 screenshots.
This is a native reader control; it does not add a settings UI to exported Web/Android
previews or establish device execution on other platforms.

## Admission before presentation loading

The executable validates `--open` before loading the controller, graphics or
preference store. A successful `--check --open FILE` returns its existing JSON and
exits normally without loading the presentation layer. A rejected file uses the
same codes, text, output stream and exit status as before; graphical `--open`
rejections also finish before any window or profile is opened.

This avoids parsing unused presentation modules during headless checks and failed
imports. It does not cache input, skip UTF-8/parser/path checks, change byte limits,
or relax launcher permissions. Valid GUI imports still load the existing controller
and saved style. The ordinary `--check` navigation demo, source export routes and
all title-size choices retain their behavior. The public builder still captures
literal imports inside branches, so GUI/export dependencies remain in the package.

`tests/tooling/reader_admission_imports.py` traces the actual packaged or installed
launcher with `strace` on Linux in the existing source-hidden harness. Successful
checks and failed imports must not open the reader controller or its settings
module, while the ordinary demo must still load them. Shared stdlib files can still
be read to verify the dependency tree hash; these reads do not prove module
evaluation and are preserved. The trace checks successful opens rather than failed
lookups and uses no timing threshold. A separate integrity test overlays a changed
shared GUI file in an isolated mount and requires rejection without modifying the
real package. Timing comparisons are separate evidence;
this optimization does not establish GUI frame-rate or other-platform improvements.

### Reader history keyboard shortcuts

In the native reader, **Alt+Left** goes Back and **Alt+Right** goes Forward,
returning to the same remembered reading positions as the toolbar. A held arrow
moves once; release and press again for the next history entry. With no available
entry it preserves the current page and focus. Plain arrows and the existing
PageUp/PageDown/Home/End reading controls are unchanged. Additional Ctrl, Meta or
Shift modifiers do not trigger these shortcuts (including Ctrl+Alt/AltGr).

While Style is open, these shortcuts leave the draft, focus and history alone.
Escape still returns to reading. Navigation never saves a style or reading history.
Embedding applications can reserve the shortcut keys independently:

```sapp
App.history_shortcuts(reader, false); // Disabled; toolbar still works.
App.history_shortcuts(reader);        // Enable again (default for new readers).
```

The method requires a boolean and returns whether the policy changed; it does not
alter `remember_positions`, rebuild the view or reinterpret an already held key.
The controller accepts explicit boolean modifier flags on key events and falls
back to the shared `keymap.sapp` aliases for older event producers. A native
modifier snapshot takes precedence over tracked key edges.

Run `shortcuts_demo.sapp` for a small embedding example. The semantic regression is
`tests/semantics/reader_history_shortcuts_test.sapp`; the source-hidden actual
launcher journey is `tests/tooling/reader_history_shortcuts_native.py --package
<prepared-release> --evidence <new-directory>`. It compares returned reading areas,
a held key across three history entries and the unchanged Style draft with real
X11 keyboard input. This establishes Linux native behavior; Web/Android exports
and other desktop/mobile native targets have no new shortcut guarantee.

### Keyboard recovery when Save becomes unavailable

If a profile conflict, uncertain save or failed reload disables the focused Save
button, the native Reader now focuses **Reload saved style** when a reloader is
available, or **Apply for this session** otherwise. The relevant settings page is
shown automatically, including at 320×240 where only one option fits per page.
The error message and unapplied draft remain intact. Focus movement does not read,
apply or save anything; release and press Enter again to choose the recovery.
A held Enter cannot accidentally invoke the newly focused action.

A retryable save error keeps Save focused and available. Other focused controls
are retained; document navigation and Escape are unchanged. Embedders use the
existing `App.enable_preferences` save/reload callbacks without an API change.
See `recovery_focus_demo.sapp` for the fallback without a reload callback,
`tests/semantics/reader_recovery_focus_test.sapp` for the controller contract, and
`tests/tooling/reader_recovery_focus_native.py` for source-hidden actual-launcher
keyboard recovery with a second PBLang profile writer at wide and compact sizes.
This is Linux X11 evidence, not a new Web/mobile/native-platform support claim.

### Ordered input batches

`App.update(reader, events)` now settles pending presentation changes before the
next event. Page Down followed by Enter in one poll therefore edits the newly
visible option. Resize followed by Tab uses the new layout's focus order; opening
Style followed by a separate Enter edits its first choice. An explicit second
press after a blocked Save can reach its newly focused recovery action.

Held keys remain held across these rebuilds. Idle polls, releases and ordinary
pointer movement do not rebuild an unchanged view. Close/quit is terminal for the
batch: later callbacks are ignored and a pending refresh cannot discard the close
request. The controller preserves input order and does not sort backend events or
change the shared input pump, renderer or builder.

`event_batch_demo.sapp` demonstrates the ordered controller API. The deterministic
regression is `tests/semantics/reader_event_batch_test.sapp`. The real-launcher
`tests/tooling/reader_event_batch_native.py` queues an X11 resize and Tab while
pausing only its isolated test child, then compares editing, explicit Save and
restart with a sequential UI reference. That native stress flow also passes on
0.8.3: it does not establish that the backend delivered a single controller batch.
The defect reproduction is the explicit ordered `App.update` batch. Native checks
verify Linux behavior remains usable; no additional platform guarantee is implied.

### Precise text and title sizes

Focus the **Text** or **Title** size choice in Style and use Left/Right to decrease or
increase by one, including values between the existing presets (for example 17
and 29). Both remain within the shared 12–40 appearance limits; the endpoints do
not wrap or rebuild the unchanged view. The hint stays on the choice in compact
layouts. Enter/Space and mouse activation keep their existing preset cycles.

Only unmodified arrows on an enabled, visible size choice adjust it. Held arrows
change once until released, including across rebuilds. Other controls, reading
navigation and Alt+history retain their behavior. Profile-enabled editing remains
a draft until explicit Apply or Save; Escape/Done discards an unapplied draft.
The original session-only mode applies adjustments immediately.

Embedders can call `App.adjust_text_size(reader, "body_size", 1)` or use
`"title_size"` and `-1`. The helper validates the property and unit step, returns
whether the value changed, and acts only while Style is open and not closing.
It marks presentation pending; call `App.update(reader, [])` when no input follows.
`App.configure` continues to accept exact validated appearance maps.

See `fine_size_demo.sapp`, `tests/semantics/reader_fine_size_test.sapp` and
`tests/tooling/reader_fine_size_native.py`. Acceptance is real Linux X11 execution;
no new native mobile, Windows or macOS support is implied by these controls.

### Precise scale and line spacing

In Style, focus **Scale** or **Spacing** and use unmodified Left/Right to move to
the adjacent 0.05 value. Scale uses the existing 0.75–1.50 range; line spacing uses
1.00–2.50. For example, scale 1 becomes 1.05 and spacing 1.45 becomes 1.50. Values
between grid points move to the next point in the requested direction (1.123 to
1.15 or 1.10). Tiny floating-point noise near a grid point is normalized before
stepping. Integer ticks prevent accumulated drift; limits do not wrap or rebuild
an unchanged view. The controls retain short ASCII keyboard hints in compact UI.

Enter/Space/click keep the original presets. Only a fresh unmodified arrow on an
enabled visible numeric choice changes it. Held arrows remain consumed across
rebuilds. Profile mode edits drafts until explicit Apply/Save; Escape/Done discards
unapplied ratios. Session-only mode applies immediately, as before.

`App.adjust_text_ratio(reader, "font_scale", 1)` also accepts `"line_height"` and
step `-1`. It returns whether the value changed, validates the property/unit step,
and acts only while Style is open and not closing. Call `App.update(reader, [])`
to settle a programmatic change when no input follows. `App.configure` and the
profile schema still accept their existing validated values; they are not globally
quantized by the new keyboard helper.

See `text_ratio_demo.sapp`, `tests/semantics/reader_text_ratio_test.sapp` and
`tests/tooling/reader_text_ratio_native.py`. Native acceptance is Linux X11 only;
these controls do not establish new mobile, Windows or macOS execution coverage.

Long custom ratio labels remain readable in a 320px-wide Style panel. Values whose
numeric string exceeds six characters display rounded to three decimal places,
prefixed with `~` (for example `Scale: ~1.235 (Left/Right)`). Short existing labels
remain unchanged. This is presentation only: configuration, draft values, Apply,
Save and canonical profile reload retain the original precise number. Arrow keys
still use that original number and the existing 0.05 stepping rules.

The Scale/Spacing widgets retain the full numeric string in `accessibility_label`
metadata for embedders. This does not add an operating-system screen-reader bridge.
Run `ratio_label_demo.sapp` for a small customization example. Regression coverage
is in `tests/semantics/reader_ratio_label_test.sapp` and
`tests/tooling/reader_ratio_label_native.py`; the latter uses the real Linux launcher,
canonical profile Save/restart, isolated synthetic data and native font measurements.

When a document link has keyboard focus, native resize events keep its action on
the visible reading page. The Reader uses the existing typed link anchor when the
viewport changes, so Enter can still open the selected destination at 320×240.
Resizing alone never follows the link. Ordinary reading positions, toolbar focus,
Style focus and explicit Page Up/Down/Home/End navigation keep their existing rules.
Disabled/hidden controls and an externally changed document do not acquire a link
anchor from stale focus.

Embedders should feed resize events through `App.update`, as demonstrated by
`resize_focus_demo.sapp`. The existing "needs more space" fallback remains for
unsupported text/viewport combinations. Direct manual form reconstruction is not a resize
event. Regression tests are `tests/semantics/reader_resize_focus_test.sapp` and
`tests/tooling/reader_resize_focus_native.py`; native evidence is isolated Linux X11.

If a resize temporarily makes a large-text layout unavailable, the Reader retains
one validated link-focus target without focusing an invisible control. When the
layout becomes available again, the same link regains focus. A new keydown, mouse
press or scroll cancels that deferred target; passive mouse movement, key release
and ticks do not. Style, explicit focus/page changes, document navigation and close
also discard it. This transient state is controller-local and is never saved in
an appearance profile. See `focus_recovery_demo.sapp`,
`tests/semantics/reader_focus_recovery_test.sapp` and
`tests/tooling/reader_focus_recovery_native.py` for recovery and cancellation flows.
