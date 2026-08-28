# pblang IDE Manual

This manual covers the **code-first** browser IDE in `editors/ide/index.html` and its backend API served by `pblang serve` — write, run, check, format, and debug pblang.

> Looking for the **visual drag-and-drop designer** (build desktop GUI, web, and CLI apps by dragging interactive elements — Delphi / Qt Designer / WordPress / Joomla / Android Studio / VS Code style)? That's **pblang Studio**, the visual authoring layer on top of this IDE — see **[`docs/Visual_Designer.md`](Visual_Designer.md)** (design/roadmap).
>
> **Native IDE:** the self-hosted pblang implementation has started. Run `pblang --ide`
> for the browser-free native IDE (`editors/ide/pbide.sapp`) with Code, Terminal,
> Designer, AI, and Browser panels. See **[`docs/PB_IDE.md`](PB_IDE.md)**.
> The native Code panel uses the self-hosted pblang frontend for Run/Check and
> now shows its toolchain status in the side pane; the Rust VM is still the native
> execution kernel and fallback while the self-host path keeps expanding.

## 1. Start the IDE

Start backend:

Create a private token file, then start the backend with protected execution:

```bash
umask 077
openssl rand -hex 32 > /tmp/pblang-ide-token
cargo run -- serve --port 8090 --run-token-file /tmp/pblang-ide-token
```

The token file must be a regular non-symlink file owned by the current user,
with no group/world permissions, and contain 32–256 bytes. Use **🔑 Run access**
in the browser IDE to load its value into the current tab's memory. The token is
not persisted in browser storage, URLs, source files, or run history.

Without `--run-token-file`, `POST /run` returns `404` and code execution is
disabled. An invalid or missing token returns `401`. Authenticated IDE code runs
under a deny-by-default profile: filesystem, network/HTTP, SQLite, FFI, imports,
process creation, environment access, graphics, task spawning, and host process
exit remain disabled. Source, stdin, output, and concurrent-run bounds are also
enforced. This is application-level containment; it is not yet an outer
hostile-code sandbox with process-level hard preemption.

Open:

- `editors/ide/index.html` in your browser.

Health endpoint:

- `GET /health`

## 2. Main workflow

- Write/edit code in the main editor.
- `Run` executes code through backend `/run`.
- `Check` runs parse/type diagnostics through `/check`.
- `Format` uses the pblang formatter behind a parse/lint guard. If formatted code
  would stop parsing or increase diagnostics, the editor leaves the buffer unchanged.
- `Auto-fix` applies deterministic safe fixes, including inline `print(x);` to
  `print x;`, smart punctuation, math-symbol normalization, and common `fs read` /
  `fs write` spacing mistakes.

Keyboard:

- `Ctrl+Enter`: Run
- `Ctrl+Shift+Enter`: Check
- `Ctrl+I`: open the AI assistant
- `Ctrl+D`: open the visual Designer (pblang Studio)
- `Ctrl+B`: open the pblang web Browser
- `Ctrl+P`: open the native IDE command palette
- `Ctrl+N`: create a new untitled buffer from the Code panel
- `Ctrl+Shift+S`: Save As from the Code panel
- `Ctrl+Shift+D`: duplicate the current editor line
- `Ctrl+Shift+K`: delete the current editor line
- `Alt+Up` / `Alt+Down`: move the current editor line up/down
- `Ctrl+/`: toggle `//` line comments on the current line or selected lines
- `Ctrl+1` ... `Ctrl+9`: switch native panels left-to-right; the tab strip shows
  the same numbers and compacts to labels like `1C`, `2T`, `3A` on narrow windows
- `Ctrl+Tab` / `Ctrl+Shift+Tab`: cycle next / previous native panel
- `Ctrl+PageDown` / `Ctrl+PageUp`: cycle next / previous native panel
- `F5` / `F6`: Run / Check from any panel
- `Ctrl+G`: Go To Line modal; type a line number and press Enter
- `Alt+Left` / `Alt+Right`: navigate back / forward through diagnostic, Outline,
  and Go To Line jumps
- Control/Alt/Meta modifiers accept SDL/minifb/platform aliases such as `LCtrl`,
  `LeftCtrl`, `ControlLeft`, and `Control`, and graphics key events now also carry
  direct modifier booleans, so shortcuts work across graphics backends even when a
  standalone modifier keydown is missed.
- `Tab`: indent selected lines, expand math shortcuts, or insert spaces in editor
- `Shift+Tab`: outdent the current line or selected lines
- `F8`: next diagnostic
- `Shift+F8`: previous diagnostic
- `F11`: toggle Debug / Release mode in the native IDE from any panel
- `F12`: toggle native IDE watcher status from any panel
- `F1`: open/close the in-IDE keyboard manual overlay

Manual modal:

- Toolbar `❓ Manual` opens in-app guide panel.
- Tabs switch between IDE and Project manuals fetched from backend:
  - `GET /manual/ide`
  - `GET /manual/project`

## 2a. Native terminal (`Term` / `F3`)

The native Terminal panel runs real PC commands through pblang's process capability when
the IDE is launched with `--allow-exec` or `--allow-all`.

- The terminal keeps a persistent working directory, so `cd subdir` affects later
  commands.
- Scrollback is bounded so noisy commands cannot grow the IDE session without limit.
- Up/Down recalls a bounded de-duplicated command history. If you start typing a new
  command, press Up to browse history, then Down back past the newest command to restore
  your in-progress draft.
- `Shift+Enter` inserts a newline for multi-line shell commands.
- `clear` clears the terminal output area.
- `Ctrl+P` exposes **Clear Terminal output**, so you can clear Terminal scrollback from
  any panel.
- The status strip above scrollback shows command-history count/selection and the core
  keyboard hints.

## 2b. AI assistant (powered by pbAI — pblang's own model)

The toolbar `🤖 AI` button (or `Ctrl+I`) opens an AI assistant backed by **pbAI**
(`stdlib/pbai.sapp`) — pblang's own model. There is **no external LLM and no network**:
pbAI runs locally, in pblang, and knows pblang's own idioms and gotchas.

- **Connection status** — the panel pings the pbAI gateway (`GET /health`) and shows
  `pbAI online / offline` live.
- **Quick actions** over the current editor code: **Explain code**, **Find bugs**,
  **Improve**, **Add comments**, **Explain diagnostics**. In the native IDE these are
  exposed as `EXPLAIN`, `BUGS`, `IMPROVE`, `QUALITY`, `COMMENT`, and `DIAG`; `INSERT`
  places the last raw pbAI answer into the editor. pbAI classifies the request with a
  small neural net and runs a pblang-grounded skill (e.g. find-bugs checks brace/paren
  balance and flags pblang-specific traps like missing-map-key reads).
- `Ctrl+P` exposes the same AI actions as searchable commands: AI explain, bugs,
  improve, quality, comment, diagnostics, insert last answer, and clear chat.
- **Free-form prompt** with editor-code context. The native panel stores the last raw
  pbAI answer for `INSERT`.
- Up/Down recalls previous free-form prompts from a bounded de-duplicated prompt
  history. When you start typing a draft, browse older prompts, then press Down past
  the newest prompt, the native panel restores your draft.
- `Shift+Enter` inserts a newline for multi-line prompts, and the status strip above
  chat output shows prompt-history count/selection plus the main keyboard hints.
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

## 2c. Visual designer — pblang Studio (`🎨 Designer` / `Ctrl+D`)

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
textbox/checkbox/radio/select/slider/panel palette, an `APP` target selector
(`gui`/`web`/`both`), runnable 3D cube/ball GUI widgets,
2D chart items, N-dimensional item placeholders, custom component placeholders,
web/HTML metadata placeholders, scanner/3D asset placeholders, and game-engine-style
camera/light/material/terrain/particle/physics/prefab/render-backend placeholders. The
palette wraps across rows when needed. It supports drag/move/edit, snapped keyboard
nudging with arrows, snapped keyboard resizing with `Ctrl+arrows` (`Shift` uses a larger step),
selection cycling with `Tab` / `Shift+Tab`, duplication with `Insert`, and layer ordering
with `Ctrl+[` / `Ctrl+]` (`Shift` sends to back/front),
`LAYOUT` cycling through free/responsive/grid/stack positions via `stdlib/design_engine.sapp`, and
parse-checked `->CODE` generation into the editor.
`Ctrl+P` also exposes Designer commands for code generation, clear canvas, layout cycle,
app target cycle, select next/previous, duplicate, delete, layer forward/backward, send
back, and bring front, so the main builder actions are available without memorizing
every shortcut.
The native inspector strip summarizes the current app target, layout mode, active tool,
item count, and selected widget geometry so you can check the design state without
reading the generated code.
Generated desktop GUI code follows the web IDE shape: `let gui = import(...)`,
`app_target()`, `build()`, `preview()`, `run_gui()`, `main()`, and `main();`. IDE Run
executes `preview()` safely without opening a second graphics window; call `run_gui()`
when you want the real GUI window. Generated forms call `gui.fit_form(...)` so placed
items, including 3D previews, fit the form. Foreign-language, HTML/CSS/JS, and scanner
components are stored as safe metadata or runnable placeholders unless an explicit host
renderer/importer is added later.

## 2d. Native run/check history (`Hist` / `F9`)

The pblang-native IDE now mirrors the web IDE's History workflow without HTML, CSS, or
JavaScript. Run (`F5`) and Check (`F6`) each add a bounded newest-first history record
with action, pass/fail status, output summary, and the source snapshot from that moment.

- `F9` opens the native `Hist` tab.
- Clicking a history row restores that record's code snapshot into the editor.
- Type to filter run/check snapshots by action, status, output, code, or timestamp.
  The filter row shows shown/total snapshots plus selected-position status.
- Use Up/Down, Home/End, PageUp/PageDown, or `Ctrl+N`/`Ctrl+P` to select a snapshot,
  then press Enter or click the row to restore its code into the editor.
- Press `Delete` to clear only the filter. Press `C`, click the footer clear action,
  or run **Clear run/check history** from `Ctrl+P` to clear the native session history.
- History is intentionally session-local for now; persistent history remains a browser IDE
  feature until the native file picker/storage layer is completed.

Native diagnostics now match the web IDE's main workflow:

- `F6` Check stores structured diagnostics for the current buffer.
- The code side pane lists current diagnostics with inferred line/column where available.
  Its Diagnostics header summarizes current severity counts, for example
  `diagnostics: 2 error(s), 1 warning(s)`, before the individual rows.
- The Code editor gutter marks diagnostic lines with compact severity letters:
  `E` for error, `W` for warning, `I` for info, and `?` for other diagnostic kinds.
  When several diagnostics share one line, the marker uses the most severe kind.
- The bottom Code status bar mirrors the web IDE's core editor signals: current
  file/dirty state, `Ln`/`Col`, line count, character count, diagnostics count,
  selected-character count, build mode, self-host status, navigation back/forward
  counts, and watcher badge.
  It also includes a compact `health OK/WARN/ERROR/OFF` strip that merges
  Debug/Release mode, watcher freshness, the watcher next action, and
  UI-observer quality into one glanceable signal.
  The `scope ...` segment shows the nearest top-level symbol span for the caret,
  such as `scope fn build L12-L28`, so you can keep orientation without opening
  Outline.
  When the caret is on a diagnostic line, the same status bar adds a compact
  `line error:` / `line warning:` message and a `+N` suffix for additional issues
  on that line.
  Code-line actions include indent/outdent selected lines,
  duplicate-current-line, delete-current-line, and
  move-current-line-up/down, and toggle-line-comment through `Ctrl+Shift+D`,
  `Ctrl+Shift+K`, `Alt+Up`, `Alt+Down`, and `Ctrl+/` or the command palette;
  `Tab` / `Shift+Tab` handle indentation. The command palette also exposes
  **Trim trailing whitespace**, which cleans selected lines when a selection exists
  or the whole buffer otherwise. These actions use normal undo snapshots and update
  the dirty state.
  Clipboard shortcuts also behave like modern code editors: `Ctrl+C` copies the
  current line when there is no selection, and `Ctrl+X` cuts the current line when
  there is no selection.
  Syntax highlighting is refreshed from a separate source hash, so repainting the
  editor does not clear the unsaved marker.
- `Ctrl+G` opens a native Go To Line modal. The requested line is clamped to the
  current buffer's valid line range, then the editor jumps to column 1.
- `Ctrl+F` opens native Find and `Ctrl+H` opens Replace. The status line shows
  live match counts while typing; Replace also shows the active field and
  replacement text before Enter applies replace-all. When a query has matches,
  the status also shows the current match ordinal and source location, such as
  `match 2/5 at L18:C9`. `Ctrl+P` also exposes Find and Replace commands for
  keyboard-discoverable access.
- Diagnostic jumps, Outline jumps, and Go To Line record the previous cursor
  location. Use `Alt+Left` / `Alt+Right` or the command palette to move through
  that jump history. Opening a different file clears stale same-buffer jump
  history, and no-op jumps do not add duplicate navigation entries.
- Click a diagnostic row to jump to that location.
- `F8` jumps to the next diagnostic; `Shift+F8` jumps to the previous diagnostic.
- `Ctrl+P` command palette includes **Problems**, **Next diagnostic**, and
  **Previous diagnostic**. Problems refreshes stale/unchecked diagnostics with Check,
  jumps to the first fresh diagnostic when issues exist, or reports a clean fresh buffer.
- The command palette also exposes Browser commands: Reload/Back, Live/Reader/Links/
  Forms/Source/Inspect/Host views, Safe/Live/Trusted/Tor security profiles, and the
  Tor route toggle.
- The `Prob` panel is a dedicated Problems view. It shows the watcher action row plus
  current diagnostics and an error/warning/info summary; Enter/click runs the action
  row or jumps to the selected diagnostic, and Esc returns to Code. The small padding
  gap between the action row and diagnostics is inert, so a click on the first
  diagnostic cannot accidentally run the watcher action.
  Type in the panel to filter diagnostics by severity, message, source text, or
  location tokens such as `L12` / `C5`; Backspace edits the filter and `Delete`
  clears it. The filter row shows shown/total diagnostics and selected-position
  status.
  Use Home/End, PageUp/PageDown, arrows, or `Ctrl+N` / `Ctrl+P` to move through
  the filtered list.
  A selected-diagnostic detail strip at the bottom shows the diagnostic severity,
  line/column, message, and source line without leaving the Problems panel.
- `F10` opens the native Browser panel because `F8` is reserved for diagnostics parity.

## 2e. Web browser — written in pblang (`🌐 Browser` / `Ctrl+B`)

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
- the native toolbar exposes **Sec:Safe/Live/Trusted/Tor** and **Tor:on/off**. Safe is
  the default, Live enables pblang-owned interactive links/forms without JS/DOM,
  Trusted is the explicit local/private and Host-render opt-in, and Tor is opt-in
  but currently needs the lower-level SOCKS/TLS transport work described by the
  browser engine;
- the Browser shows a visible security summary row for the active profile: links,
  forms, private-host access, Tor route, ad/tracker blocking, and `js/dom:off`.
  This row is informational; only the button row above it changes Browser mode,
  security profile, Tor state, reload, or back navigation;
- native Browser rendering has no DOM, no JS engine, and no embedded iframe.

The native view has:

- an address bar with Back, Reload, Live/Reader/Links/Forms/Source/Inspect/Host mode
  buttons, a `Sec:*` profile button, and a `Tor:on/off` route toggle;
- the Browser header is responsive: on narrow or manually scaled windows it compacts
  labels such as `Reader` -> `Read` and keeps the control row inside the viewport;
- **Live / Reader / Links / Forms / Source / Inspect / Host** modes — *Live* renders
  pblang-extracted page blocks (title/headings/paragraphs/images) as native cards,
  *Reader* shows clean text, *Links* lists the absolutized links, *Forms* lists
  discovered `<form>` fields with editable values and submit previews, *Source* shows numbered HTML source, *Inspect* shows
  F12-style details (URL, policy, status, counts, elements, links, images, forms), and
  *Host* is an explicit external screenshot fallback through `stdlib/browser_render.sapp`;
- form parsing, form value defaults, form encoding, submit preview, and profile-aware
  submit blocking in the pblang engine (`forms`/`form_values`/`form_preview_url`/
  `form_submit_mode`/`urlencode`/`submit`).

Forms mode keys:

- `Tab` / `Shift+Tab`: select next / previous field.
- `Ctrl+N` / `Ctrl+P`: select next / previous form on the page.
- Type text / `Backspace`: edit the selected field value.
- `Space`, `Left`, or `Right`: toggle checkbox/radio fields or cycle select options.
- Mouse click: select a form/field row; checkbox/radio/select rows cycle immediately;
  the submit-preview row submits.
- `Enter`: submit through the current Browser security profile. Safe and Tor block
  submits; Live and Trusted allow them subject to URL policy.

The web IDE may still use browser-native UI features as a reference surface, but the
native IDE Browser intentionally stays a pblang Live/Reader/Source/Inspect renderer by
default. Exact Chrome-like rendering requires a real DOM/CSS/JS engine, so the native
`Host` tab can delegate to an installed Chromium/Chrome/Brave/Edge/Firefox browser in
headless screenshot mode after the pblang URL policy passes. It needs `--allow-exec`;
Live/Reader/Source/Inspect remain the safe no-JS defaults.

The 2026-06-21 Browser screenshots showed why this boundary matters: a full Google page
contains script-driven layout, navigation links, images, and generated parameters that a
reader engine must filter instead of drawing verbatim. Native Live mode now keeps raw URLs
in Links mode, filters noisy/corrupt page blocks, adds Source/Inspect details, and uses a
wider page column so it reads closer to the web IDE while staying browser-free.

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
- `Search/filter`: narrow snapshots by action/status/output/code
- `Clear filter`: remove the current history search
- `Clear`: clear all history

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
- The filter row shows shown/total examples plus selected-position status.
- `Up`/`Down`, Home/End, PageUp/PageDown, or `Ctrl+N`/`Ctrl+P` change selection.
- Larger example catalogs scroll inside the modal and show a right-side scrollbar.
- `Delete` clears the current filter, `Enter` loads the selected example, and `Esc` closes.
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
- The toolbar Mode button shows the full `Debug` / `Release` label, and the Watcher
  button uses a wider label so both are easier to hit and read. The Watcher button
  also shows compact state labels such as `Stale`, `ErrN`, `Dirty`, `Unchk`, or
  `Watch`, and the bottom status bar uses compact badges such as `watch ok` or
  `watch stale` instead of the full watcher summary.
- Native watcher status mirrors normal IDE project watches without a background thread:
  it tracks buffer revision/hash, dirty/clean state, line count, symbol count, and
  diagnostics. It also records line/symbol/diagnostic deltas and a bounded newest-first
  event log. Diagnostics are marked `unchecked`, `fresh`, or `stale`: Check makes the
  current buffer fresh, and later edits mark diagnostics stale until the next Check.
  The Code side pane includes a watcher action row, and the command palette exposes
  **Run watcher suggested action**: stale/unchecked diagnostics run Check, fresh
  diagnostics jump through F8 navigation, and clean dirty buffers route to Save.
  You can also click the watcher action row directly in the Code side pane.
  Unchanged draw refreshes do not overwrite the last meaningful event. The watcher
  appears in the Code side pane and status bar; toggle with Watch or `F12`.
- The native IDE also publishes a structured UI observer snapshot through
  `stdlib/ui_observer.sapp`. Each draw records the visible surface regions, active
  focus, useful signals, recent command/mouse interactions, and layout issues such
  as nonpositive sizes, offscreen regions, or overlapping regions. It also computes
  a bounded `quality_score` so healthy layouts are obvious at a glance. The Code
  side pane shows a clickable compact `UI OBSERVER` block, the watcher state carries
  `ui_snapshot` / `ui_lines`, and `Ctrl+P` exposes **Show UI observer snapshot** to
  print a freshly refreshed report into the output pane.
- Save writes the current buffer path, falling back to `.pbide_buffer.sapp` if needed.
  Save As opens a path prompt, writes the buffer to that path, marks the buffer clean,
  records it in recent files, and clears stale navigation when the current file changes.
  Revert reloads the current path from disk, clears stale diagnostics/navigation/modals,
  and marks the buffer clean again.
  Open / `Ctrl+O` shows a native Quick Open modal. It lists the current directory with
  folders first, prepends matching recent files, filters as you type, supports
  Up/Down/PageUp/PageDown/Home/End and `Ctrl+N`/`Ctrl+P`, shows result counts and
  selected-position details, opens files with Enter or click, enters directories
  with Enter/click, and falls back to the typed path when no row matches.
- The native `Files` panel is the persistent project browser. It uses the same
  pblang-owned directory listing as Quick Open, keeps its current directory visible,
  filters as you type, shows result counts and selected-position details, toggles
  flat/tree/recent mode with `Tab`, supports Up/Down/PageUp/PageDown/Home/End and
  `Ctrl+N`/`Ctrl+P`, opens files with Enter/click, enters directories with
  Enter/click, goes up with Backspace on an empty filter, and refreshes with
  `Ctrl+R`. It also tracks workspace roots: `Ctrl+Left`/`Ctrl+Right` or
  `Ctrl+H`/`Ctrl+L` cycles roots, and `Ctrl+B` bookmarks the current directory as
  a root. `Ctrl+Plus` / `Ctrl+Minus` adjusts tree depth, clamped to `1..8`.
  Opening files records a bounded newest-first recent list, so recent mode can
  quickly return to active work without walking the full project tree.
  `Ctrl+P` also exposes Files commands for Flat, Tree, Recent, and Reveal current
  file; Reveal switches to the active file's directory and filters by its name.
  In recent mode, `Delete` removes the selected recent row, `Ctrl+Delete` clears
  the whole recent list, and `Ctrl+P` exposes **Clear recent files**.
  Tree mode is bounded by depth and entry count and skips generated-heavy folders
  so project browsing stays responsive.
- `Ctrl+P` opens the native command palette. Type to filter commands, use Up/Down,
  PageUp/PageDown, Home/End, or `Ctrl+N`/`Ctrl+P` to select, Enter to run, and Esc
  to close. It exposes Run, Check, New buffer, Save, Save As, Open, Revert, Clear output, Restore starter, Format,
  Fix, Indent/Outdent lines, Duplicate line, Delete line, Move line up/down, Toggle line comment, Trim trailing whitespace, Find, Replace, Examples, Debug/Release, Watcher, Theme, UI snapshot, panel switches, Files
  view/reveal commands, and Help from one modal.
  Shortcuts are searchable, multi-word queries can match across fields such as
  `panel ctrl+6`, and the palette shows shown/total commands plus selected-command
  position/detail. `Delete` clears the current command filter. Shortcuts render in
  their own right-side column, so hidden
  toolbar actions stay discoverable on compact windows. Clicks outside native
  modals are absorbed by the backdrop, so Quick Open, Examples, and Command
  Palette cannot accidentally trigger controls behind the overlay. Backdrop
  clicks also close Help, Go To Line, Quick Open, Examples, and Command Palette,
  matching normal IDE modal behavior.
  The native Theme toggle now affects the main editor, Browser, Files, Outline,
  Problems, History, Terminal/AI input bars, and core modals such as Go To Line,
  Examples, Command Palette, Quick Open, Help, and the Designer palette/canvas/
  inspector/widget preview surface. The top toolbar action buttons and panel tabs
  also use the same Light/Dark-aware color system, including Browser body output
  rows, security-mode chips, Files/Outline semantic rows, Problems diagnostics,
  watcher status, UI observer issue lines, AI chat user lines, code side-pane
  headers, Terminal output, History OK/FAIL rows, modal empty-result text,
  Designer component badges, Watch toolbar stale/error states, and Code syntax
  highlighting. The toolbar/browser/history buttons also use density-aware hit
  padding, so compact layouts keep small visual buttons without making the
  clickable target too small. Repeated text labels are shaped through a bounded
  pblang cache before drawing, which keeps the smoother text path responsive.
  Browser controls, Designer palette buttons, and History actions share the
  same raised native button chrome as the main toolbar, and editor/list/browser
  panels use thicker rounded scrollbars so compact layouts are easier to scan.
- The native `Out` panel is the source outline. It lists top-level `fn`, `let`,
  `const`, and `export` symbols with `Lx-Ly` spans and line counts. The filter
  row shows shown/total symbols plus the selected row position; type to filter,
  use Up/Down, Home/End, PageUp/PageDown, or `Ctrl+N`/`Ctrl+P` to select, `Delete`
  to clear the filter, Enter or click to jump to the source line, and Esc to
  return to Code.
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
