# pblang IDE Manual

This manual covers the **code-first** browser IDE in `editors/ide/index.html` and its backend API served by `pblang serve` — write, run, check, format, and debug pblang.

> Looking for the **visual drag-and-drop designer** (build desktop GUI, web, and CLI apps by dragging interactive elements — Delphi / Qt Designer / WordPress / Joomla / Android Studio / VS Code style)? That's **pblang Studio**, the visual authoring layer on top of this IDE — see **[`docs/Visual_Designer.md`](Visual_Designer.md)** (design/roadmap).
>
> **Goal:** the IDE itself is to be **written in pblang** (a self-hosted IDE on pblang's own runtime). The HTML/JS `index.html` below is the bootstrap toward that target.

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

## 2c. Web browser — written in pblang (`🌐 Browser` / `Ctrl+B`)

A web browser whose **engine is pblang**: `stdlib/browser.sapp` fetches a page
(`http_get`) and parses its title, reader text (script/style stripped), links (both `"`
and `'` quoting), and **resolves relative links to absolute URLs** (`resolve(base, href)`
— root-relative, protocol-relative, and directory-relative). The view has:

- an address bar with **◀ Back / ▶ Forward / ⟳ Reload / ⌂ Home** (a real history stack)
  and a loading indicator;
- **Page / Reader / Links / Forms** modes — *Page* renders the fetched HTML in a
  sandboxed iframe, *Reader* shows the pblang-extracted text, *Links* lists the
  absolutized links, *Forms* lists the page's `<form>`s with editable fields;
- **clickable link-following**: clicking a link in *Links* navigates to it (pushing
  history), all fetched + parsed by pblang;
- **form submission**: *Forms* mode renders each form's controls; **Submit** sends them
  via the pblang engine — GET appends an encoded query string, POST sends an
  `x-www-form-urlencoded` body (`browser.sapp` `forms`/`urlencode`/`submit`, with a
  pblang UTF-8 percent-encoder) — and shows the resulting page.

It calls `POST /browse {url}` (fetch) and `POST /submit {action, method, fields}` (form)
on the pblang gateway (`editors/ide/tools/ai/gateway.sapp`, the same one the AI panel
uses) — so browsing works end-to-end, down to pblang doing the fetch, parse, URL
resolution, form encoding, and submission.

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
