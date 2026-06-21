# pblang IDE Manual

This manual covers the **code-first** browser IDE in `editors/ide/index.html` and its backend API served by `pblang serve` — write, run, check, format, and debug pblang.

> Looking for the **visual drag-and-drop designer** (build desktop GUI, web, and CLI apps by dragging interactive elements — Delphi / Qt Designer / WordPress / Joomla / Android Studio / VS Code style)? That's **pblang Studio**, the visual authoring layer on top of this IDE — see **[`docs/Visual_Designer.md`](Visual_Designer.md)** (design/roadmap).
>
> **Native IDE:** the self-hosted pblang implementation has started. Run `pblang --ide`
> for the browser-free native IDE (`editors/ide/pbide.sapp`) with Code, Terminal,
> Designer, AI, and Browser panels. See **[`docs/PB_IDE.md`](PB_IDE.md)**.

## 1. Start the IDE

Start backend:

```bash
cargo run -- serve --port 8090
```

Open:

- `editors/ide/index.html` in your browser.

Health endpoint:

- `GET /health`

## 2. Main workflow

- Write/edit code in the main editor.
- `Run` executes code through backend `/run`.
- `Check` runs parse/type diagnostics through `/check`.
- `Format` uses `/format` (falls back to local formatting if backend unavailable).
- `Auto-fix` applies simple known fixes and re-checks.

Keyboard:

- `Ctrl+Enter`: Run
- `Ctrl+Shift+Enter`: Check
- `Ctrl+I`: open the AI assistant
- `Ctrl+D`: open the visual Designer (pblang Studio)
- `Ctrl+B`: open the pblang web Browser
- `Tab`: insert spaces in editor
- `F8`: next diagnostic
- `Shift+F8`: previous diagnostic
- `F11`: toggle Debug / Release mode in the native IDE
- `F12`: toggle native IDE watcher status
- `Ctrl+/`: open in-IDE manual modal

Manual modal:

- Toolbar `❓ Manual` opens in-app guide panel.
- Tabs switch between IDE and Project manuals fetched from backend:
  - `GET /manual/ide`
  - `GET /manual/project`

## 2a. AI assistant (powered by pbAI — pblang's own model)

The toolbar `🤖 AI` button (or `Ctrl+I`) opens an AI assistant backed by **pbAI**
(`stdlib/pbai.sapp`) — pblang's own model. There is **no external LLM and no network**:
pbAI runs locally, in pblang, and knows pblang's own idioms and gotchas.

- **Connection status** — the panel pings the pbAI gateway (`GET /health`) and shows
  `pbAI online / offline` live.
- **Quick actions** over the current editor code: **Explain code**, **Find bugs**,
  **Improve**, **Add comments**, **Explain diagnostics**. pbAI classifies the request with
  a small neural net and runs a pblang-grounded skill (e.g. find-bugs checks brace/paren
  balance and flags pblang-specific traps like missing-map-key reads).
- **Free-form prompt** with an *include editor code* toggle, and **Insert into editor**.
- **Config** (collapsible): the gateway **Endpoint** (default `http://localhost:8080`) and
  a **Skill** override (auto / review / explain). Persists in
  `localStorage['pblang.ide.aimcp.v1']`.

**Run the pbAI gateway** (`editors/ide/tools/ai/gateway.sapp`, written in pblang — it
serves the panel's contract and answers via `stdlib/pbai.sapp`):

```bash
# from the repo root — no model server needed; pbAI is in-process
./target/debug/pblang run editors/ide/tools/ai/gateway.sapp --allow-all
# serves GET /health, OPTIONS *, POST /mcp/tool on 127.0.0.1:8080 (PB_AI_GATEWAY_BIND to change)
```

Leave the panel **Endpoint** at `http://localhost:8080`. The gateway sets CORS headers so
the browser IDE can call it; if it's down the panel degrades gracefully with this hint.

Request contract — the panel `POST`s `{endpoint}/mcp/tool` with
`{ "tool": "<name>", "arguments": { "prompt": "...", "language": "pblang" } }` and accepts
`{result}` / `{text}` / `{content:[{text}]}` / raw-string responses.

> pbAI is the language's own intelligence (see `docs/pbAI.md`); the assistant skills here
> are useful today, and the generative scale-up toward a full pblang language model is the
> roadmap. The pb-mcp-server remains pbAI's tool layer for agentic actions.

## 2b. Visual designer — pblang Studio (`🎨 Designer` / `Ctrl+D`)

A drag/click visual builder (the start of `docs/Visual_Designer.md`). Three panes —
**Palette · Canvas · Inspector** — and a *kind* selector. The design is the single source
of truth: it renders live and **generates pblang** (`⟶ To pblang code` writes a runnable
program into the editor).

- **🌐 Web** — click components (Heading, Text, Link, Button, Input, Image, Section,
  Card, **2D Canvas**, **3D Scene**, **Custom element**) onto a live HTML canvas; nest
  into a selected container; edit properties in the inspector. Codegen targets
  `stdlib/html.sapp`.
- **🔌 Circuit** — digital logic: add Inputs, AND/OR/NOT/XOR/NAND gates, and Outputs;
  wire inputs in the inspector; **▶ Simulate** shows the truth table; codegen emits pblang
  boolean functions.
- **⚛ Quantum** — place H/X/Z/CNOT/Measure gates on a qubit grid; **▶ Simulate** runs a
  browser state-vector simulation (basis-state probabilities); codegen targets
  `stdlib/quantum.sapp` (`circuit`/`h`/`cx`/`measure`/`run`).

Native Designer (`pblang --ide`) now has a parallel pblang-only surface: label/button/
textbox/checkbox/radio/select/slider/panel palette, 3D cube/ball design items,
2D chart items, N-dimensional item placeholders, custom component placeholders,
web/HTML metadata placeholders, scanner/3D asset placeholders, and game-engine-style
camera/light/material/terrain/particle/physics/prefab/render-backend placeholders. The
palette wraps across rows when needed. It supports drag/move/edit, `LAYOUT` cycling
through free/responsive/grid/stack positions via `stdlib/design_engine.sapp`, and
parse-checked `->CODE` generation into the editor.
Generated desktop GUI code follows the web IDE shape: `let gui = import(...)`,
`build()`, `main()`, and `main();`, so the result is easier to run and round-trip.
Foreign-language, HTML/CSS/JS, and scanner components are stored as safe metadata or
runnable placeholders unless an explicit host renderer/importer is added later.

## 2c. Native run/check history (`Hist` / `F9`)

The pblang-native IDE now mirrors the web IDE's History workflow without HTML, CSS, or
JavaScript. Run (`F5`) and Check (`F6`) each add a bounded newest-first history record
with action, pass/fail status, output summary, and the source snapshot from that moment.

- `F9` opens the native `Hist` tab.
- Clicking a history row restores that record's code snapshot into the editor.
- History is intentionally session-local for now; persistent history remains a browser IDE
  feature until the native file picker/storage layer is completed.

Native diagnostics now match the web IDE's main workflow:

- `F6` Check stores structured diagnostics for the current buffer.
- The code side pane lists current diagnostics with inferred line/column where available.
- Click a diagnostic row to jump to that location.
- `F8` jumps to the next diagnostic; `Shift+F8` jumps to the previous diagnostic.
- `F10` opens the native Browser panel because `F8` is reserved for diagnostics parity.

## 2d. Web browser — written in pblang (`🌐 Browser` / `Ctrl+B`)

A native browser whose **engine is pblang**: `stdlib/browser.sapp` enforces a URL
policy, fetches a page (`http_get`), parses title, reader text (script/style stripped),
headings, images, links, and forms, and **resolves relative links to absolute URLs**
(`resolve(base, href)` — root-relative, protocol-relative, and directory-relative).

Native safety rules:

- only explicit `http://` and `https://` URLs are fetched;
- `file:`, `javascript:`, `data:`, and similar non-web schemes are blocked;
- credential URLs (`https://user:pass@host`), CR/LF/control characters, and common
  non-web service ports are blocked before fetch, form submit, or host rendering;
- common ad/tracker/malicious URL patterns are blocked before network access;
- loopback, LAN, and private hosts are blocked by default unless the caller explicitly
  opts in with `allow_private=true`;
- native Browser rendering has no DOM, no JS engine, and no embedded iframe.

The native view has:

- an address bar with Reload and Page/Reader/Links/Forms/Render mode buttons;
- **Page / Reader / Links / Forms / Render** modes — *Page* renders pblang-extracted page blocks
  (title/headings/paragraphs/images), *Reader* shows clean text, *Links* lists the
  absolutized links, *Forms* lists discovered `<form>` fields, and *Render* is an
  explicit host-browser screenshot path through `stdlib/browser_render.sapp`;
- form parsing and form encoding in the pblang engine (`forms`/`urlencode`/`submit`).

The web IDE may still use browser-native UI features as a reference surface, but the
native IDE Browser intentionally stays a pblang reader/page renderer by default. Exact
Chrome-like rendering requires a real DOM/CSS/JS engine, so the native `Render` tab
delegates to an installed Chromium/Chrome-compatible browser in headless screenshot mode
after the pblang URL policy passes. It needs `--allow-exec`; Page/Reader remain the
safe no-JS default.

The 2026-06-21 Browser screenshots showed why this boundary matters: a full Google page
contains script-driven layout, navigation links, images, and generated parameters that a
reader engine must filter instead of drawing verbatim. Native Page mode now keeps raw URLs
in Links mode, filters noisy/corrupt page blocks, and uses a wider page column so it reads
closer to the web IDE while staying browser-free.

## 3. Diagnostics panel

Diagnostics include stage, message, location, and optionally source line text.

Features:

- click a diagnostic to jump to line/column
- prev/next navigation buttons and keyboard shortcuts
- active diagnostic highlighting
- quick-fix buttons on matching errors

## 4. History panel

History records latest run/check events (up to 30 entries), including:

- action (`run` or `check`)
- success/failure status
- timestamp
- output summary
- diagnostics snapshot
- code snapshot

Actions:

- `Show`: replay that entry output + diagnostics
- `Load code`: restore snapshot into editor
- `Clear`: clear history

All history is stored in browser local storage.

## 5. Examples

Examples are fetched dynamically from backend:

- `GET /examples`
- `GET /examples/<id>`

IDE behavior:

- loads fallback examples if backend example catalog is unavailable
- supports filter/search input by id/title

Native IDE parity:

- In `pblang --ide`, click `Ex` to open the searchable examples picker.
- Type to filter examples by title or source code.
- `Up` / `Down` changes selection, `Enter` loads the selected example, and `Esc` closes.
- Clicking a row loads that example into the editor.

## 6. Persistence

IDE persists:

- editor autosave content
- theme preference
- output pane width
- example filter text
- run/check history

Storage keys are local browser keys under `localStorage`.

## 7. Layout controls

- drag splitter between editor and output pane to resize output width
- width is clamped and persisted

Native IDE parity:

- In `pblang --ide`, drag the vertical splitter between the editor and right output pane
  to resize the output pane.
- Native splitter width is clamped during the session. Browser localStorage persistence is
  still web-IDE-only until native storage preferences are added.
- Default native scale is now smaller and cleaner (`PBIDE_SCALE=1`) with a fixed minimum
  window size. Use `PBIDE_SCALE=2` or higher for larger text.
- Key input accepts common graphics/browser backend names for digits and symbols:
  direct digits (`1`), `Digit1`, `Key1`, `Num1`, `Numpad1`, and numpad arithmetic keys.
- The native code editor also accepts direct Unicode character keys from capable
  graphics backends and IMEs, so pasted or typed symbols like `π`, `≤`, and `→` stay
  visible when the compact native font has a glyph. Emoji and other unsupported glyphs
  stay in the buffer and render as a visible missing-glyph cell instead of disappearing.
- In the native code editor, type math compose shortcuts such as `\pi`, `\Pi`/`\prod`,
  `\sqrt`, `\le`, `\ge`, `\ne`, `\sum`, or `\int`, then press Space or Tab to insert the
  symbol. `π` is the pi constant; `Π`/`∏` is product/repeated multiplication and
  normalizes to `product(...)`. `Fix` can still normalize these symbols back into
  parser-friendly pblang source.
- Source strings support direct Unicode and scalar escapes such as `"\u{1F600}"` for
  emoji. Use `stdlib:unicode.sapp` for codepoint inspection, escaping, and basic
  classification.
- Code typing auto-pairs `()`, `[]`, `{}`, and quotes, and skips over an existing
  closing pair when typed at the cursor.
- Native build modes mirror normal IDE behavior: **Debug** runs Check before Run and
  blocks execution when diagnostics exist; **Release** runs directly for a faster loop.
  Use the toolbar Mode button or `F11`.
- Native watcher status mirrors normal IDE project watches without a background thread:
  it tracks buffer revision/hash, dirty/clean state, line count, symbol count, and
  diagnostics. It appears in the Code side pane and status bar; toggle with Watch or
  `F12`.
- Native Code panel shortcut parity: `Ctrl+Enter` runs, `Ctrl+Shift+Enter` checks,
  `Ctrl+D` opens Designer, `Ctrl+B` opens Browser, and `Ctrl+I` opens AI.

## 7b. Visual/media engine

`stdlib/visual_engine.sapp` is the high-level scene contract for graphics and media. It
does not replace `graphics.sapp`; it sits above it and describes what a renderer should
draw or play:

- 2D nodes: rectangles, lines, circles, polygons, meshes, paths, groups, text, images,
  charts, custom UI components, and imported-designer metadata;
- 3D nodes: points, meshes, cubes, spheres, surfaces, 3D scatter/chart data;
- engine-scene nodes: cameras, lights, materials, shader metadata, physics bodies,
  terrains, particle systems, skyboxes, ECS-style entities/components, prefabs, and
  render backend targets for native/web/OpenGL/Vulkan/Godot/Unity/Unreal-style bridges;
- N-dimensional nodes: points, clouds, tensors, and generic N-dimensional items with a
  projection hint;
- media nodes: image/audio/video asset references, timelines, and keyframes;
- web/foreign/scanner components: HTML/CSS/JS, other-language snippets, and scanner
  assets are represented as metadata-only safe nodes until a trusted renderer/importer
  is explicitly used;
- safe network asset policy using the same `browser.sapp` URL guard.

Backends can compile this scene plan to the native IDE, web canvas, future GPU renderer,
or network display without changing the pblang scene source.

## 7c. Science library

`stdlib/science.sapp` adds a first portable science layer: constants, unit conversion,
quantities, vector math, physics formulas, chemistry helpers, geometry helpers, signal
processing, and experiment tables. It is meant to support simulations, scientific plots,
AI data preparation, and future visual-science panels in the IDE.

## 8. Backend API contract

Used endpoints:

- `POST /run` body: `{ code }`
- `POST /check` body: `{ code }`
- `POST /format` body: `{ code, max_line_length? }`
- `POST /normalize` body: `{ code }`
- `GET /health`
- `GET /examples`
- `GET /examples/<id>`
- `GET /manual/<id>`
- `GET /ws` — WebSocket upgrade. Sends one JSON object per text frame:
  - `{"op": "ping"}` → `{"op": "pong", "ts_ms": <number>}`
  - `{"op": "echo", "msg": "..."}` → `{"op": "echoed", "msg": "..."}`
  - `{"op": "version"}` → `{"op": "version", "version": "...", "service": "pblang-ide-backend"}`
  Useful for IDE liveness checks and bring-up without per-request TCP
  handshake. `run` / `check` / `format` are intentionally **not** exposed
  over WS yet — those are CPU-bound and stay on the POST endpoints. A
  streaming wrapper that re-uses the same compile/run pipeline is on the
  roadmap.

Responses are JSON and include CORS support for browser access.

## 9. Troubleshooting

- **`pblang --ide` cannot find `pbide.sapp`**
  - update to a recent `pblang` binary; old installed copies only looked under the current working directory
  - set `PBLANG_IDE=/absolute/path/to/editors/ide/pbide.sapp` for custom layouts
  - package installs can also place the repo payload under `/usr/local/share/pblang` or `/usr/share/pblang`
- **Cannot connect to server**
  - start backend with `pblang serve --port 8090`
  - verify port and browser access
- **Examples missing**
  - backend may be offline; fallback examples should still appear
- **Diagnostics stale**
  - run `Check` explicitly
- **Formatting differences**
  - backend formatter may differ from local fallback when offline

## 10. Recommended usage pattern

1. Write/modify code.
2. Run `Check`.
3. Navigate diagnostics with `F8`.
4. Apply quick fixes as needed.
5. Run full execution with `Run`.
6. Use History to compare outputs/regressions quickly.
