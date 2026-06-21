# pblang-native IDE — browser-free, for total security

> **Goal.** Make the IDE completely written in pblang — no HTML, CSS, or JS — while matching the web IDE's workflow as closely as possible.

The native IDE is the self-hosted Studio direction of the project: editor, terminal, designer, AI helper, browser, and run/check history are pblang code running on pblang's own runtime. The web IDE remains useful as a bootstrap/reference UI, but the native IDE is the long-term security-first target.

---

## Why — the security argument

The browser IDE (`editors/ide/index.html`) is convenient, but a web IDE brings browser attack surface:

| Web-stack layer | Attack surface it adds |
|---|---|
| HTML parser | malformed-markup / injection bugs |
| DOM + `innerHTML` | XSS and content-injection bugs |
| CSS engine | layout/parser exploits and data exfiltration tricks |
| JS engine | JIT bugs, prototype pollution, dependency surface |
| `fetch`/iframes | SSRF, content-injection, sandbox escapes |

A pblang-native IDE removes those layers. The whole IDE is a native window, a pixel buffer, input events, and pblang code under pblang's capability model. There is no DOM, no JS runtime, and no HTML renderer to exploit.

---

## Architecture

```text
        ┌─────────────── editors/ide/pbide.sapp ───────────────┐
        │  native window + event loop                           │
        │  code editor | terminal | designer | AI | browser      │
        │  history snapshots + restore                           │
        ├───────────────────────────────────────────────────────┤
        │  stdlib/editor_model.sapp   buffer + cursor + edits   │
        │  stdlib/bitmap_font.sapp    5×7 glyphs → pixels       │
        │  stdlib/browser.sapp        fetch + reader parser     │
        │  stdlib/browser_render.sapp optional host screenshot  │
        │  stdlib/gui.sapp            designer widget model     │
        │  stdlib/design_engine.sapp  responsive 2D/3D layout   │
        │  stdlib/pbai.sapp           local pblang assistant    │
        │  stdlib/graphics.sapp       window / rect / present   │
        └───────────────────────────────────────────────────────┘
```

## Current native panels

- **Code** — editor, syntax highlighting, selection, undo/redo, find, save, run, check, output pane.
- **Terminal** — runs real PC commands through `proc_spawn_wait` when launched with `--allow-exec` or `--allow-all`.
- **Design** — native visual designer with label/button/textbox/checkbox/radio/select/slider/panel palette, 2D chart, 3D cube/ball/scanner placeholders, N-dimensional item placeholders, custom/web component placeholders, and game-engine-style placeholders for camera, light, material, terrain, particles, physics body, prefab, and OpenGL/Vulkan-style render backend targets. It supports responsive layout cycling, bounded drag/move/edit behavior, and parse-checked `->CODE` generation into the editor.
- **AI** — local pbAI assistant with quick actions: explain, bugs, improve, quality, comment. The assistant now exposes structured analysis metadata: intent, score vector, code-present flag, quality score, risk summary, and raw scan metrics.
- **Browser** — pblang-native safe page/reader view using `stdlib/browser.sapp` to enforce an HTTP/HTTPS URL policy, block unsafe schemes/private hosts/credential URLs/CRLF input/non-web service ports plus known ad/tracker/malicious URL patterns by default, fetch pages, decode entities, strip scripts/styles, extract page blocks/images/links/forms, clean noisy reader lines, deduplicate repeated navigation text, and list discovered forms. `Render` is an explicit opt-in host-browser snapshot path through `stdlib/browser_render.sapp`.
- **History** — web-IDE-style run/check snapshot list. Every native Run/Check records status, output summary, and source snapshot; clicking a row restores that code into the editor.
- **Help/Manual** — shortcut overlay and documentation-oriented panel.

---

## Web-IDE parity checklist

| Area | Native status |
|---|---|
| Top toolbar/actions | Implemented: run, check, format, auto-fix, searchable examples picker, save, open-path prompt, Debug/Release mode toggle, watcher toggle, style cycling, and pinned native panel tabs. |
| Code editor | Implemented: typing, movement, selection, find, undo/redo, syntax coloring, output pane, draggable output splitter, real diagnostics list, click-to-jump diagnostics, watcher side pane, broader digit/punctuation/direct-Unicode key handling, math-symbol normalization through `Fix`, web-style bracket/quote auto-pairing, Ctrl+Enter/Ctrl+Shift+Enter run/check shortcuts, F8/Shift+F8 diagnostic navigation, F11 mode toggle, and F12 watcher toggle. |
| Terminal | Implemented with persistent cwd and scrollback. |
| Designer | Implemented with palette, canvas, drag/move, delete, edit text, wrapped multi-row palette, responsive layout cycling, label/button/textbox/checkbox/radio/select/slider/panel widgets, 2D chart, 3D cube/ball/scanner placeholders, N-dimensional items, custom component placeholders, web/HTML metadata placeholders, engine-style camera/light/material/terrain/particles/physics/prefab/backend placeholders, and parse-checked runnable code generation. |
| AI helper | Implemented with local pbAI; no network/Ollama required. Adds EXPLAIN/BUGS/IMPROVE/QUALITY/COMMENT quick actions plus quality/risk metadata. |
| Browser page/reader | Improved: safe URL policy, entity decode, whitespace collapse, script/style removal, adjacent-link spacing, page block/image extraction, raw-link separation into Links mode, corrupt/noisy block filtering, absolute links, duplicate-line suppression, links view, forms parser, and explicit host-render snapshot mode. |
| Browser forms UI | Basic forms list implemented; editable form cards and submit UI are next. |
| History | Implemented: bounded newest-first Run/Check records with output summary and click-to-restore code snapshots. |
| File picker | Pending; current fallback uses the configured buffer path. |
| Visual polish | Improved: modern brand pill, active-tab underline, pinned always-visible panel tabs, no hidden-counter overlap, Web/Smooth/Contrast/Pixel style cycling, cleaner diagnostics/history cards, lower default bitmap scale, fixed minimum window size, and web-inspired dark palette while staying native and bitmap-font based. |

---

## Screenshot-driven fixes tracked

The screenshots showed three main native IDE problems:

1. **Greek/web text mojibake or unreadable entity text.** Fixed in `stdlib/browser.sapp` by improving entity decoding, title decoding, whitespace collapse, and reader parsing. The native bitmap font now has compact glyphs for common Greek/math/code symbols, while broader Unicode shaping still belongs to a future text renderer.
2. **Browser panel looked unlike the web reader.** Improved by separating reader text, links, and parsed forms at the engine level. Forms UI is now the next UI task rather than a missing parser task.
3. **Native IDE parity was unclear.** This document now separates implemented, partial, and pending web-parity items.
4. **AI panel felt shallow.** Improved in `stdlib/pbai.sapp` with structured `analyze()` metadata, quality scoring, sensitive-capability risk summaries, more pblang-specific knowledge (typeclasses, logic, reactive streams, security), and a native IDE `QUALITY` quick action.
5. **Code side pane had placeholders.** The diagnostics/history boxes now show actionable check guidance and the latest action instead of "pending" text.
6. **Toolbar parity gaps.** Native Format now uses `stdlib/compiler/formatter.sapp`; Auto-fix applies deterministic pblang fixes; Examples cycles built-in starter programs; Open reloads the current buffer path.
7. **Panel tabs were unreachable in the new toolbar.** Fixed by splitting the toolbar into left-side actions and right-pinned panel tabs (`Code`, `Term`, `AI`, `Designer`, `Browser`, `Hist`), plus keyboard fallbacks (`F2`/`F3`/`F4`/`F7`/`F9`/`F10`). `F8` is reserved for diagnostics parity with the web IDE.
8. **Some embedded examples did not run through F5.** Replaced fragile snippets with self-hosted-runner-compatible examples and added `tests/semantics/pbide_examples_test.sapp`.
9. **Newer screenshots showed `+N` overlap near Open and weak style choice.** Removed the hidden-action counter, shortened core toolbar actions, and added a visible `Style` cycle (`Web` → `Smooth` → `Contrast` → `Pixel`). `PBIDE_SCALE=1` remains the launch-time escape hatch for smaller text.
10. **Open looked broken before the buffer file existed.** `Open` now starts an in-IDE path prompt (`Enter` opens, `Esc` cancels, `Ctrl+V` pastes the IDE clipboard). Save defaults to a repo/session-local `.pbide_buffer.sapp` and falls back there if the previous path fails.
11. **Fractional line numbers appeared after mouse clicks.** Mouse-to-buffer math and `editor_model.set_cursor` now floor fractional coordinates before clamping.
12. **Designer needed a real layout foundation.** Added `stdlib/design_engine.sapp`: backend-neutral 2D/3D item model, snapping, anchors, responsive grid/stack layout, hit-testing, and isometric 3D projection. The native Designer now has `CUBE3D`/`BALL3D` palette items and a `LAYOUT` cycle for free/responsive/grid/stack positioning.
13. **Designer layout crashed on normal GUI widgets.** Fixed `design_engine.sapp` to treat `min_w`/`min_h` as optional, so `gui.sapp` widgets and design-engine 3D items can be reflowed together.
14. **Native IDE lacked the web IDE's History workflow.** Added a `Hist` panel (`F9`) backed by pure `stdlib/ide_features.sapp` history helpers. Run/Check now save bounded snapshots, status, and output summaries; clicking a history row restores its source into the editor.
15. **Native diagnostics were text-only.** F6 now stores normalized diagnostics, the code side pane lists real diagnostic rows, clicking a row jumps to the inferred location, and F8/Shift+F8 navigate next/previous diagnostics like the web IDE.
16. **Native output pane was fixed-width.** Added web-IDE-style splitter dragging in the Code panel. The output pane now resizes during the session and is clamped so the editor and output pane both remain usable.
17. **Native examples only rotated one-by-one.** The `Ex` toolbar action now opens a web-IDE-style searchable picker. Type to filter by title/code, use Up/Down to select, Enter to load, Esc to close, or click a row.
18. **Native browser needed safer protocols.** `stdlib/browser.sapp` now rejects non-HTTP(S) schemes (`file:`, `javascript:`, `data:`, etc.) and blocks loopback/LAN/private hosts by default before fetch or form submit. Local/private browsing must be an explicit opt-in by the caller.
19. **Native browser needed a page-like view without embedding a browser.** Added a `Page` mode backed by pblang page blocks: title/headings/paragraphs/images/links/forms are extracted and rendered natively. This improves visual similarity to the web IDE while keeping the no-DOM/no-JS security boundary.
20. **Large buffers made editing feel laggy.** Code rendering now draws syntax tokens in one visible-pass instead of scanning all tokens once per row, and syntax coloring pauses on very large buffers so editing remains responsive.
21. **Pasted mathematical notation was not pblang source.** Added `stdlib/math_symbols.sapp` and wired native IDE `Fix` to normalize common symbols and shortcuts (`π`, `Π`/`∏`, `τ`, `φ`, `√`, `×`, `÷`, `≤`, `≥`, `≠`, `∧`, `∨`, `¬`) into valid pblang-friendly code. `π` means the pi constant; `Π`/`∏` mean product / repeated multiplication and normalize to `product(...)`.
22. **2026-06-21 screenshot: Browser Page view showed raw Google links and broken image/navigation text.** Page mode now filters corrupt/noisy blocks, stops dumping raw URL links into the page body, keeps Links mode as the URL list, and uses a wider content column.
23. **2026-06-21 screenshot: native UI still felt too pixel-sized.** Default `PBIDE_SCALE` is now `1` with a 1280×760 minimum window, giving a cleaner web-like density. Set `PBIDE_SCALE=2` or higher for larger text.
24. **Typing numbers/symbols varied by backend key names.** `stdlib/keymap.sapp` now accepts direct digits (`"1"`), `Digit1`, `Key1`, `Num1`, and `Numpad1`-style names, plus numpad arithmetic keys.
25. **Exact page rendering needed a real renderer.** Added `stdlib/browser_render.sapp` and a native IDE `Render` mode. It uses an installed Chromium/Chrome-compatible browser in headless screenshot mode, after the same pblang URL policy passes. Page/Reader stay the safe no-DOM/no-JS defaults; Render needs `--allow-exec`.
26. **Browser protocols needed stronger malicious-action blocking.** URL policy now rejects credential-bearing URLs, control/space/CRLF input, and common non-web service ports such as SSH, SMTP, Redis, Postgres, MySQL, Elasticsearch, and MongoDB before fetch/render/submit.
27. **IDE typing needed symbol composition.** The editor can expand math shortcuts like `\pi`, `\Pi`/`\prod`, `\sqrt`, `\le`, `\ge`, `\ne`, `\sum`, and `\int` by pressing Space or Tab after the shortcut. `stdlib/keymap.sapp` also recognizes more punctuation aliases and direct Unicode character keys from capable backends/IMEs.
34. **Code text needed broad character support.** pblang strings now support braced Unicode scalar escapes such as `\u{1F600}` for emoji, the pblang-written tokenizer decodes the same escapes, and the native IDE accepts direct emoji/symbol text input. The native bitmap font includes compact glyphs for common math/Greek symbols plus a few emoji smoke-test glyphs, and unknown symbols render as visible missing-glyph cells instead of collapsing to `?`.
35. **IDE needed real project/build state like normal IDEs.** Added `stdlib/ide_features.sapp` watcher/build-profile helpers and wired the native IDE toolbar/status/side pane to them. Debug mode runs `Check` before `Run` and blocks on diagnostics; Release mode runs directly for speed. Watcher tracks buffer hash, revision, dirty/clean state, line count, symbol count, and diagnostic count. F11 toggles Debug/Release; F12 toggles Watcher.
28. **Designer generated code could be fragile.** Native Designer now emits a web-IDE-style `build()`/`main()` wrapper using `gui.sapp`, validates generated source with `frontend_parse_safe` before replacing the editor buffer, generates runnable placeholders for 3D items, clamps dragged items inside the canvas, refreshes label hit boxes while editing, and supports radio/select/slider widgets.
29. **Designer needed extensible item/component contracts.** `stdlib/visual_engine.sapp` now models custom components, foreign-language metadata components, HTML/CSS/JS web components, scanner-imported components, path/group 2D nodes, sphere/surface 3D nodes, and N-dimensional item nodes. The native Designer exposes safe placeholders for chart/custom/web/scanner/ND items; generated `gui.sapp` code remains runnable and does not execute foreign JS or external code.
30. **Native browser needed ad/tracker/malicious blocking.** `stdlib/browser.sapp` now has deterministic `classify_url_threat(...)` / `threat_policy(...)` checks and applies them from `url_policy(...)`, blocking common tracker/ad/malicious URL patterns before fetch, form submit, or Render.
31. **Some Code panel controls were easy to miss or overlap.** The toolbar hit-test now matches the visible button band more tightly, the Code side pane no longer overlaps Diagnostics and History rows, and the native editor now supports web-style `Ctrl+Enter` Run, `Ctrl+Shift+Enter` Check, `Ctrl+D` Designer, `Ctrl+B` Browser, and `Ctrl+I` AI fallbacks.
32. **Designer needed game-engine-style creation tools.** `stdlib/visual_engine.sapp` now models engine scene concepts inspired by Unreal/Godot/Unity/OpenGL/Vulkan workflows: render backend targets, cameras, lights, materials, shader metadata, physics bodies, terrain, particles, skyboxes, ECS-style entities/components, prefabs, and render pipeline metadata. The native Designer exposes compact `CAM`, `LIGHT`, `MAT`, `TERR`, `PART`, `PHYS`, `PREFAB`, and `VK/GL` tools.
33. **Designer palette grew beyond one row.** The native Designer palette now wraps across rows and the canvas placement, hit-testing, clamping, and responsive layout math all use the real wrapped palette height.

Regression coverage: `tests/semantics/browser_native_ide_test.sapp` locks URL-policy blocking/opt-in, reader parsing, title/entity decode, page block/image extraction, page-mode noise filtering, raw-link separation, link resolution, form extraction, and UTF-8 form URL encoding. `tests/semantics/browser_render_test.sapp` locks host-render command planning and policy-first blocking without requiring Chrome to be installed. `tests/semantics/pbide_ui_test.sapp` covers keymap variants and syntax-highlighting colors. `tests/semantics/pbide_designer_codegen_test.sapp` locks parse-safe Designer-generated GUI code and gui_parse round-tripping. `tests/semantics/pbide_examples_test.sapp` runs the embedded IDE examples through the same self-hosted frontend used by F5. `tests/semantics/design_engine_test.sapp` covers responsive columns, grid layout, anchors, snapping, hit-testing, and 3D projection. `tests/semantics/ide_features_test.sapp` covers outline, find/replace, status text, diagnostic normalization/summaries, bounded history records, Debug/Release build profiles, and watcher records used by the native IDE.

---

## Honest gaps

- **Full Unicode rendering** is not done. The current native font has compact glyphs for common math/Greek symbols and falls back visibly for unknown characters; full shaping, combining marks, bidi text, and CJK/font fallback need a real text renderer.
- **Arbitrary emoji identifiers/operators** are not enabled. Unicode is supported for strings, comments, editor text, and tooling helpers; the parser grammar still keeps identifiers/operators conservative for stability.
- **Browser layout** is a native page/reader model by default. The optional Render tab delegates exact DOM/CSS/JS layout to a host Chromium-compatible renderer when installed and allowed.
- **Engine integration** is a scene/component contract today. pblang can describe cameras/lights/materials/physics/prefabs/render targets, but it does not embed Unreal, Unity, Godot, OpenGL, or Vulkan runtimes directly yet.
- **Interactive verification** still needs human testing across display backends, keyboard layouts, DPI settings, and focus behavior.
- **Performance** is improved for large buffers, but can still improve further with a host `gfx_draw_text` primitive or row-run batching.

## Migration plan

1. Keep the web IDE as the reference workflow.
2. Bring native panels to web-parity feature by feature.
3. Add regression fixtures for every screenshot-level bug.
4. Replace remaining pending actions: full file picker dialog, editable browser form cards.
5. Retire `index.html` only when the native IDE reaches real workflow parity.
