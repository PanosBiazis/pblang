# pblang Studio — the visual drag-and-drop designer

> **Status: VISION / DESIGN (2026-05-30).** Today the pblang IDE
> (`editors/ide/index.html` + `pblang serve`) is a **code-first** editor: write,
> run, check, format, debug (see `docs/IDE_MANUAL.md`). The **visual designer**
> described here — build apps by dragging interactive elements onto a canvas —
> is the roadmap. It is grounded in runtime pblang **already has** (`graphics.sapp`
> / `pblang-graphics`, `web_framework.sapp` / `html.sapp` / `http_server.sapp`,
> and the self-hosted parser/codegen for round-tripping). Items are marked
> **(today)** or **(planned)**.
>
> **The IDE is to be written in pblang.** A core project goal: pblang Studio — both
> the editor and the designer — is itself a **pblang application** (using pblang's own
> web/GUI/CLI stack), not an HTML/JS app that merely talks to pblang. The current
> `editors/ide/index.html` is the bootstrap; the target is a **self-hosted IDE** that
> dogfoods Studio (the IDE is built with the IDE) and runs on pblang's own runtime —
> the same "all the way down" property as self-hosting the compiler.

---

## 1. What it is

**pblang Studio** is a visual app builder layered on top of the pblang IDE. The goal:
a programmer who *wants* a visual workflow can **drag and drop interactive
elements** to assemble an app — instead of writing every widget by hand — while a
programmer who prefers code keeps full control. You choose; the two stay in sync.

It is meant to feel like the tools people already know, unified over one language:

- **Delphi / Qt Designer** — for **desktop GUI** apps (forms, widgets, the object
  inspector, event handlers).
- **WordPress / Joomla** — for **websites / web apps** (page/section/block
  composition, themes, content, plugins).
- **Android Studio / VS Code** — for the **code side** (a real editor, diagnostics,
  build/run, debugging) that sits right next to the canvas.

And, distinctively, pblang Studio also targets:

- **CLI / TUI apps** — assemble a command-line app visually too: arguments, prompts,
  menus, tables, and progress — the parts that are usually hand-coded.

The output is always **pblang code** you own — there is no opaque binary project format
you can't read or hand-edit.

---

## 2. Three project kinds (one IDE)

When you start a project you pick a target; the palette and canvas adapt, but the
designer, inspector, event model, and code round-trip are the same everywhere.

| Target | Feels like | Renders on (today) | You drag… |
|--------|-----------|--------------------|-----------|
| **Desktop GUI** | Delphi / Qt Designer | `graphics.sapp` → `pblang-graphics` (SDL2 / minifb); `window` + draw loop **(today)** | windows, panels, buttons, labels, text inputs, lists, menus, canvases |
| **Web app / site** | WordPress / Joomla / VS Code web | `web_framework.sapp` (routes, middleware) + `html.sapp` (tags, forms, tables) + `http_server.sapp` **(today)** | pages, sections, nav bars, blocks, forms, tables, cards, buttons |
| **CLI / TUI** | a form-builder for terminals | pblang stdout / terminal **(today)** | arguments & flags, prompts, select menus, tables, spinners, progress bars |

> The *runtime* for all three already exists in the stdlib; pblang Studio adds the
> **visual authoring layer** and the **code generator** on top of it.

---

## 3. Core concepts

The designer is built from a small set of ideas shared across all three targets:

- **Palette** — a categorized tray of **components/items** (widgets, blocks, CLI
  controls). Drag one onto the canvas to instantiate it.
- **Canvas** — the live, what-you-see surface. Drop, move, resize, nest, and align
  elements directly. Snap-to-grid and alignment guides.
- **Tree / outline** — the component hierarchy (parent → children), like a DOM/VCL
  tree; select, reparent, and reorder here when the canvas is crowded.
- **Property inspector** — edit the selected element's properties (text, size, color,
  layout, data source, validation) in a form. No code needed for the common cases.
- **Event wiring** — make it **interactive**: pick an event (`on_click`,
  `on_change`, `on_submit`, `on_select`, `on_timer`…) and bind it to a pblang
  handler — generated as a stub you fill in, or pointed at an existing function.
- **Layout** — constraint/flex/grid layout so designs survive resize (rather than
  absolute pixels only).
- **Data binding** — bind a widget to a value/model field so the UI updates when data
  changes and vice-versa.

---

## 4. The killer feature — two-way code ↔ design sync

pblang Studio is **not** a one-way scaffold generator. Design and code are two views of
the same program:

```
   ┌────────────┐   serialize    ┌────────────┐   codegen     ┌──────────────┐
   │   Canvas   │ ─────────────▶ │  .pbui      │ ────────────▶ │  app.sapp    │
   │ (drag/drop)│ ◀───────────── │  design doc │ ◀──────────── │ (pblang code)│
   └────────────┘   reflect      └────────────┘   parse        └──────────────┘
```

- Drag a button → the designer writes the corresponding pblang (e.g. a
  `graphics`/`html`/CLI element) into `app.sapp`.
- Edit `app.sapp` by hand → the designer **re-parses** it and reflects your change on
  the canvas.

This round-trip is exactly the kind of work pblang's **self-hosted compiler** is built
for (`stdlib/compiler/`: it tokenizes, parses, and can run pblang — see
`docs/CURRENT_STATUS.md`). The designer reads the AST to render the canvas and emits
pblang through codegen, so hand-written and drag-authored code are first-class equals.
A small **`.pbui`** design document holds layout/inspector metadata that has no natural
home in code (canvas positions, grouping), kept beside the generated `.sapp`.

---

## 5. Component palettes (planned, grounded in current stdlib)

### Desktop GUI (Delphi / Qt Designer style)
`Window`, `Panel`, `GroupBox`, `Label`, `Button`, `TextInput`, `TextArea`, `Checkbox`,
`Radio`, `ComboBox`, `ListBox`, `Slider`, `ProgressBar`, `Menu`/`MenuItem`, `Toolbar`,
`TabControl`, `Canvas`/`Image`, `Timer`. → emits `graphics.sapp` `window` + draw/event
loop on `pblang-graphics`.

### Web (WordPress / Joomla style)
`Page`, `Header`/`Footer`, `NavBar`, `Section`, `Hero`, `Card`, `Grid`/`Columns`,
`Form` + `Field`s, `Table`, `Button`, `Link`, `Image`, `List`, `Menu`, `Content
block`, reusable `Component`/`Partial`, `Theme`, `Plugin slot`. → emits `html.sapp`
markup + `web_framework.sapp` routes/middleware, served by `http_server.sapp`.

### CLI / TUI
`Command`, `Argument`, `Flag`/`Option`, `Prompt`, `Confirm`, `Select`, `MultiSelect`,
`Table`, `Spinner`, `ProgressBar`, `Panel`/`Box`, `Color/Style`. → emits an arg parser
+ interactive prompt flow in pblang.

Each palette item is a **template** that expands to idiomatic pblang and registers its
editable properties and events with the inspector.

---

## 6. How it compares

| Tool | Builds | What pblang Studio takes from it |
|------|--------|----------------------------------|
| **Delphi** | Desktop GUI | Form designer + object inspector + event handlers; **readable generated code** (its strongest idea) |
| **Qt Designer** | Desktop GUI | Widget palette, layouts/constraints, signals→slots wiring |
| **WordPress** | Websites | Block/section composition, themes, plugins, content-first authoring |
| **Joomla** | Web/CMS | Structured templates, modules, menus, extensions |
| **Android Studio** | Mobile/app | Visual layout editor *beside* a real code editor + build/run/debug |
| **VS Code** | Code | First-class code editing, diagnostics, extensions — the code half of Studio |

pblang Studio's distinct angle: **one language and one codebase across desktop GUI,
web, and CLI**, with **honest, round-trippable generated pblang** (no lock-in), and an
optional **pbAI** assistant (next section).

---

## 7. Interactivity & the event model

"Interactive" means events are bound visually and run real pblang:

1. Select an element on the canvas.
2. In the inspector's **Events** tab, pick an event (`on_click`, `on_change`,
   `on_submit`, `on_select`, `on_timer`, …).
3. Studio generates a handler stub and links it:

   ```
   // generated by pblang Studio — button "save"
   fn on_save_click(ctx) {
       // your logic here
   }
   ```
4. Fill in the body in the code editor; the binding persists across design edits.

For data-driven UIs, bind a widget's value to a model field; Studio wires the
read/update so toggling a checkbox or typing in a field updates state and re-renders.

---

## 8. pbAI assist (**started** — the editor side ships today)

Because pblang is getting its own model family (**pbAI** — see `docs/pbAI.md`), the
designer can offer AI help that stays inside the tool:

- **"Describe the screen"** → pbAI drafts a layout you can refine by hand. *(planned — designer side)*
- **"Wire this button to save the form to SQLite"** → pbAI fills the handler body. *(planned — designer side)*
- **Explain / fix** a diagnostic on the generated code. *(today — see below)*

pbAI drives the same **pb-mcp-server** tools the rest of the toolchain uses, so its
suggestions are grounded in your real project.

**Shipped today (the editor side, §84):** the bootstrap IDE (`editors/ide/index.html`)
now has a **`🤖 AI` panel** (`Ctrl+I`) wired to the **pb-mcp-server**. It shows live
gateway status, lets you pick the AI tool (`ai_ollama_generate` for a local LLM,
`ai_chain_of_thought`, …), and offers quick actions over the current code — **Explain**,
**Find bugs**, **Improve**, **Add comments**, **Explain diagnostics** — plus a free-form
prompt and *Insert into editor*. This is the same in-tool, project-grounded assistant the
designer will reuse; the canvas-aware actions ("describe the screen", "wire this button")
are the remaining designer-side work. See `docs/IDE_MANUAL.md` §2a.

---

## 9. Architecture (planned)

```
┌───────────────────────── pblang IDE (browser) ─────────────────────────┐
│  Palette │            Canvas (drag/drop)            │  Inspector        │
│  Tree    │  ── live preview of GUI / web / CLI ──   │  props · events   │
├─────────────────────────────────────────────────────────────────────────┤
│  Code editor (VS Code-like) ⇄ two-way sync ⇄ Designer                    │
└───────────────────────────────┬─────────────────────────────────────────┘
                                 │  HTTP/WS (pblang serve — today)
                                 ▼
        ┌──────────────────────────────────────────────────────┐
        │  Backend: parse/codegen via stdlib/compiler           │
        │  .pbui (design metadata) ⇄ app.sapp (pblang code)     │
        │  run/check/format (today) · preview render            │
        └───────────────────────────┬──────────────────────────┘
                                     ▼  build targets (today)
        native (graphics) · web (http_server) · CLI · WASM · C
```

- **Front end** bootstraps from the existing browser IDE (`editors/ide/index.html`) and
  migrates to being **written in pblang** itself (pblang's web/GUI stack), so Studio is a
  self-hosted pblang application that builds itself.
- **Back end** extends `pblang serve` (already exposes `/run`, `/check`, `/format`,
  `/ws` — see `docs/IDE_MANUAL.md` §8) with designer endpoints (parse-to-tree,
  codegen, preview render).
- **Output** rides pblang's existing multi-backend codegen (interpreter, WASM, C,
  native) so a Studio project can ship as a desktop binary, a web service, or a CLI —
  and, longer term, run on a pblang-built base (see `docs/OS_Development.md`).

---

## 10. Roadmap (planned)

> **Status (2026-05-31, §88): D0 STARTED.** The bootstrap IDE now has a **`🎨 Designer`**
> view (`Ctrl+D`): Palette · Canvas · Inspector, with a *kind* selector. **Web** (D0):
> click components onto a live HTML canvas (incl. a **2D Canvas**, a **3D Scene** preview,
> and a **Custom "any element"** for the 2D/3D/n-dimensional goal) and **codegen → pblang
> (`html.sapp`)**. Two extra kinds shipped beyond the original three: **🔌 Circuit**
> (digital logic — drag gates, truth-table simulation, codegen to boolean pblang) and
> **⚛ Quantum** (gate grid, browser state-vector simulation, codegen to `quantum.sapp`).
> Separately, a **`🌐 Browser`** whose engine is written in pblang (`stdlib/browser.sapp`).
> Remaining D-phases (round-trip, events, desktop GUI, CLI, full 3D, self-hosted Studio)
> are below.

| Phase | Goal |
|-------|------|
| **D0 — Web canvas MVP** | **STARTED (§88)** — drag `html.sapp` blocks onto a canvas; codegen a pblang web app; live preview. (Web is easiest — the canvas *is* HTML.) Round-trip + `web_framework.sapp` routing are the next slice (D1). |
| **D1 — Round-trip** | **STARTED (§92)** — `stdlib/ui_parse.sapp` parses generated pblang web code back into the canvas via the **self-hosted parser** (the designer's `↩ From code` button + gateway `/parse-ui`). The `.pbui` metadata doc (canvas-only state) is the remaining piece. |
| **D2 — Inspector + events** | **DONE for web (§95)** — the inspector now has an **Events** section: pick an applicable event per element (`on_click`/`on_change`/`on_input`/`on_submit`), Studio auto-names a handler (e.g. button "Save" → `on_save_click`) and wires it. Codegen emits the binding as an html attribute (`map{ "onclick": "on_save_click(event)" }`) **plus** a `fn on_save_click(event){…}` stub; the round-trip (`ui_parse.sapp`) reads those attrs back into the node's `events`, so a code→canvas→code cycle preserves the wiring. Bindings persist in the `.pbui`-style localStorage doc. Desktop/CLI event targets land with D3/D4. |
| **D3 — Desktop GUI target** | **STARTED for web-parity widgets (§96)** — new `stdlib/gui.sapp` toolkit (widgets *drawn* from `graphics.sapp` primitives + the 5×7 bitmap font, mouse hit-tested, handlers as field closures) + a designer **🖥 Desktop** kind: widget palette, drag-to-move form-surface canvas, property/size inspector, D2-style event wiring → generates a runnable `gui.sapp` app (`form`/`add`/`run`). Widgets: label/button/textbox/checkbox/panel **+ radio/select(dropdown)/slider (§98)** — the standard form-control set, with the slider adding press-and-drag. **Round-trip (§97):** `stdlib/gui_parse.sapp` parses generated `gui.sapp` back into the form + widgets via the self-hosted parser (gateway `/parse-gui`, the **↩ From code** button) — the desktop kind is now two-way. **Data-binding (§99):** the form has a `model` (source of truth); widgets `bind` a value to a model field (inspector Model editor + per-widget bind), generating `gui.set_model` + `gui.bind(...)` and round-tripping both — toggling/typing updates the model, a handler writing the model re-displays. Next: popup dropdown, layout containers (nesting), radio-group binding. |
| **D4 — CLI/TUI target** | Visual command/argument/prompt builder emitting a pblang CLI. |
| **D5 — Themes/plugins + pbAI** | Reusable components, themes, a plugin slot (WordPress/Joomla flavor), and pbAI-assisted authoring. |

Each phase is gated the pblang way — locked by tests and verified green — and tracked
in `docs/CURRENT_STATUS.md` as it ships.

---

## 11. Relationship to existing pieces

- **`editors/ide/index.html` + `pblang serve`** — the host the designer extends
  (`docs/IDE_MANUAL.md`).
- **`stdlib/graphics.sapp` / `pblang-graphics`** — the desktop-GUI runtime.
- **`stdlib/web_framework.sapp` / `html.sapp` / `http_server.sapp`** — the web runtime.
- **`stdlib/compiler/` (self-hosted)** — parse↔codegen for two-way sync.
- **Multi-backend codegen** — ship Studio projects as native / web / WASM / C / CLI.
- **`docs/pbAI.md`** — the optional in-designer AI assistant.

---

*pblang Studio: drag interactive elements to build desktop GUI, web, and CLI apps —
or write the code by hand — in one language, with readable, round-trippable pblang as
the single source of truth.*


i want also for both for web apps and gui apps to have 2d and 3d elements and give the programmer the ability to design any element if he/she wants, for n-dimensions
