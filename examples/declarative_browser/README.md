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
to the same document. Family, body size, text scale, line spacing and density are
session-local options. No preferences are persisted or transmitted.

Opening **Style** focuses the font choice. Enter changes the focused option while
keeping it selected; Tab and Shift+Tab move between the available controls.
**Done** restores focus to **Style** on the same document. A held activation key
does not activate another option after a redraw. Programmatic appearance updates
also preserve an available, enabled button's focus and any held keyboard modifier.
Back, Forward and Home leave settings and display their destination while retaining
the chosen appearance. Removed, disabled or non-interactive controls do not receive
restored focus.

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

The native example automatically paginates a document when its current view would
exceed the window height, truncate wrapped text, or clip a document link label.
**Previous page / Next page**, **Page Up / Page Down**, **Home / End** navigate
reading pages. The toolbar's **Home** button still navigates to the session home.
Held page keys advance once per press. The original view remains when it fits.

Changing appearance or resizing retains a logical text position (block and offset),
so a changed page count does not move the reader to unrelated text. Local navigation
starts at the destination's first page. Reading position is memory-only and is not
stored separately for each history entry. A link's complete label is readable text
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
to a compact layout. All five existing choices remain available: font family, body
size, text scale, line spacing and density. The layout shows one choice at the
minimum 320×240 viewport, two at 320×288, and up to five as height allows.
**Previous options / Next options**, Page Up/Down and Home/End navigate these option
pages while Style is open; document reading position is unchanged. Changing pages
focuses the first displayed choice. Tab/Shift+Tab and Enter/Space retain their
existing control behavior, and held activation keys do not change a newly focused
choice unintentionally. Done remains in the top toolbar and returns to the document.

Resizing keeps the currently focused choice visible. If a page navigation button
becomes disabled or disappears in a larger layout, focus returns to the remembered
choice. Opening Style starts at the font choice. When the original screen fits,
its previous choice layout is preserved. The Style toolbar always uses compact
14px sans text. Compact choice controls use 14px text and fixed spacing
so even extreme document typography cannot hide the options needed to change it.
Preferences are still session-local and have no new storage or network effects.

Developers may call `App.settings_page(reader,index)` (zero-based, clamped, false
when unavailable or unchanged). The example-local `settings_layout.plan(height,
anchor)` computes capacity and visible indices without opening a window; anchor is
the choice index 0–4, and height uses the existing GUI bounds 240–4320. Shared GUI
contracts and Web/Android exporters remain unchanged.

```sh
pblang run tests/semantics/reader_compact_settings_test.sapp --allow-all
xvfb-run -a -s '-screen 0 1000x1000x24 -nolisten tcp' env XMODIFIERS=@im=none python3 tests/tooling/reader_compact_settings_native.py
```

The isolated native test changes all five options using real keyboard input at
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
