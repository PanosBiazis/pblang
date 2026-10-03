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

The default guide fits the tested 800×816 and 390×844 native windows. Very large
custom settings, unusually long documents or shorter windows can exceed the
available height; this example does not add scrolling or a pagination engine.
Native row reservation remains conservative, and bitmap fallback cannot emulate
arbitrary system fonts. Web/Android use platform-owned fonts and may break lines
differently. The inert `--export` HTML continues to use its existing fixed style;
customizable presentation is provided by `--web-preview` and the shared view.

Additional tests: `tests/semantics/readable_typography_test.sapp` and
`tests/tooling/readable_browser_native.py` (isolated Xvfb, optional
`PBLANG_READER_WIDTH=390`). The latter checks navigation plus keyboard-only font
changes and records default/serif screenshots.
