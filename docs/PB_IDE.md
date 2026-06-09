# pblang-native IDE — browser-free, for total security

> **Goal.** Make the IDE completely written in pblang — no HTML, CSS, or JS — while matching the web IDE's workflow as closely as possible.

The native IDE is the self-hosted Studio direction of the project: editor, terminal, designer, AI helper, and browser are pblang code running on pblang's own runtime. The web IDE remains useful as a bootstrap/reference UI, but the native IDE is the long-term security-first target.

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
        ├───────────────────────────────────────────────────────┤
        │  stdlib/editor_model.sapp   buffer + cursor + edits   │
        │  stdlib/bitmap_font.sapp    5×7 glyphs → pixels       │
        │  stdlib/browser.sapp        fetch + reader parser     │
        │  stdlib/gui.sapp            designer widget model     │
        │  stdlib/design_engine.sapp  responsive 2D/3D layout   │
        │  stdlib/pbai.sapp           local pblang assistant    │
        │  stdlib/graphics.sapp       window / rect / present   │
        └───────────────────────────────────────────────────────┘
```

## Current native panels

- **Code** — editor, syntax highlighting, selection, undo/redo, find, save, run, check, output pane.
- **Terminal** — runs real PC commands through `proc_spawn_wait` when launched with `--allow-exec` or `--allow-all`.
- **Design** — native visual designer with label/button/textbox/checkbox/panel palette, 3D cube/ball design items, responsive layout cycling, and `->CODE` generation into the editor.
- **AI** — local pbAI assistant with quick actions: explain, bugs, improve, quality, comment. The assistant now exposes structured analysis metadata: intent, score vector, code-present flag, quality score, risk summary, and raw scan metrics.
- **Browser** — pblang-native reader view using `stdlib/browser.sapp` to fetch pages, decode entities, strip scripts/styles, extract links, parse forms, clean noisy reader lines, deduplicate repeated navigation text, and list discovered forms.
- **Help/Manual** — shortcut overlay and documentation-oriented panel.

---

## Web-IDE parity checklist

| Area | Native status |
|---|---|
| Top toolbar/actions | Implemented: run, check, format, auto-fix, rotating examples, save, open-path prompt, style cycling, and pinned native panel tabs. |
| Code editor | Implemented: typing, movement, selection, find, undo/redo, syntax coloring, output pane. |
| Terminal | Implemented with persistent cwd and scrollback. |
| Designer | Implemented with palette, canvas, drag/move, delete, edit text, responsive layout cycling, 3D cube/ball items, and code generation. |
| AI helper | Implemented with local pbAI; no network/Ollama required. Adds EXPLAIN/BUGS/IMPROVE/QUALITY/COMMENT quick actions plus quality/risk metadata. |
| Browser reader | Improved: entity decode, whitespace collapse, script/style removal, adjacent-link spacing, absolute links, noisy-line filtering, duplicate-line suppression, links view, and forms parser. |
| Browser forms UI | Basic forms list implemented; editable form cards and submit UI are next. |
| File picker | Pending; current fallback uses the configured buffer path. |
| Visual polish | Improved: modern brand pill, active-tab underline, pinned always-visible panel tabs, no hidden-counter overlap, Pixel/Smooth/Contrast style cycling, cleaner diagnostics/history cards, and web-inspired dark palette while staying native and bitmap-font based. |

---

## Screenshot-driven fixes tracked

The screenshots showed three main native IDE problems:

1. **Greek/web text mojibake or unreadable entity text.** Fixed in `stdlib/browser.sapp` by improving entity decoding, title decoding, whitespace collapse, and reader parsing. The ASCII bitmap font still transliterates/folds non-ASCII at draw time until a full Unicode text renderer exists.
2. **Browser panel looked unlike the web reader.** Improved by separating reader text, links, and parsed forms at the engine level. Forms UI is now the next UI task rather than a missing parser task.
3. **Native IDE parity was unclear.** This document now separates implemented, partial, and pending web-parity items.
4. **AI panel felt shallow.** Improved in `stdlib/pbai.sapp` with structured `analyze()` metadata, quality scoring, sensitive-capability risk summaries, more pblang-specific knowledge (typeclasses, logic, reactive streams, security), and a native IDE `QUALITY` quick action.
5. **Code side pane had placeholders.** The diagnostics/history boxes now show actionable check guidance and the latest action instead of "pending" text.
6. **Toolbar parity gaps.** Native Format now uses `stdlib/compiler/formatter.sapp`; Auto-fix applies deterministic pblang fixes; Examples cycles built-in starter programs; Open reloads the current buffer path.
7. **Panel tabs were unreachable in the new toolbar.** Fixed by splitting the toolbar into left-side actions and right-pinned panel tabs (`Code`, `Term`, `AI`, `Designer`, `Browser`), plus keyboard fallbacks (`F2`/`F3`/`F4`/`F7`/`F8`).
8. **Some embedded examples did not run through F5.** Replaced fragile snippets with self-hosted-runner-compatible examples and added `tests/semantics/pbide_examples_test.sapp`.
9. **Newer screenshots showed `+N` overlap near Open and weak style choice.** Removed the hidden-action counter, shortened core toolbar actions, and added a visible `Style` cycle (`Pixel` → `Smooth` → `Contrast`). `PBIDE_SCALE=1` remains the launch-time escape hatch for smaller, less-pixelated text.
10. **Open looked broken before the buffer file existed.** `Open` now starts an in-IDE path prompt (`Enter` opens, `Esc` cancels, `Ctrl+V` pastes the IDE clipboard). Save defaults to a repo/session-local `.pbide_buffer.sapp` and falls back there if the previous path fails.
11. **Fractional line numbers appeared after mouse clicks.** Mouse-to-buffer math and `editor_model.set_cursor` now floor fractional coordinates before clamping.
12. **Designer needed a real layout foundation.** Added `stdlib/design_engine.sapp`: backend-neutral 2D/3D item model, snapping, anchors, responsive grid/stack layout, hit-testing, and isometric 3D projection. The native Designer now has `CUBE3D`/`BALL3D` palette items and a `LAYOUT` cycle for free/responsive/grid/stack positioning.
13. **Designer layout crashed on normal GUI widgets.** Fixed `design_engine.sapp` to treat `min_w`/`min_h` as optional, so `gui.sapp` widgets and design-engine 3D items can be reflowed together.

Regression coverage: `tests/semantics/browser_native_ide_test.sapp` locks reader parsing, title/entity decode, link resolution, form extraction, and UTF-8 form URL encoding. `tests/semantics/pbide_examples_test.sapp` runs the embedded IDE examples through the same self-hosted frontend used by F5. `tests/semantics/design_engine_test.sapp` covers responsive columns, grid layout, anchors, snapping, hit-testing, and 3D projection.

---

## Honest gaps

- **Full Unicode rendering** is not done. The current native font is ASCII-first and folds/transliterates many non-ASCII characters for readability.
- **Browser layout** is a reader view, not a full HTML/CSS layout engine.
- **Interactive verification** still needs human testing across display backends, keyboard layouts, DPI settings, and focus behavior.
- **Performance** can improve with a host `gfx_draw_text` primitive or row-run batching.

## Migration plan

1. Keep the web IDE as the reference workflow.
2. Bring native panels to web-parity feature by feature.
3. Add regression fixtures for every screenshot-level bug.
4. Replace remaining pending actions: full file picker dialog, searchable examples picker, editable browser form cards.
5. Retire `index.html` only when the native IDE reaches real workflow parity.
