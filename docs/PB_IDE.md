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
        │  stdlib/pbai.sapp           local pblang assistant    │
        │  stdlib/graphics.sapp       window / rect / present   │
        └───────────────────────────────────────────────────────┘
```

## Current native panels

- **Code** — editor, syntax highlighting, selection, undo/redo, find, save, run, check, output pane.
- **Terminal** — runs real PC commands through `proc_spawn_wait` when launched with `--allow-exec` or `--allow-all`.
- **Design** — native visual designer with label/button/textbox/checkbox/panel palette and `->CODE` generation into the editor.
- **AI** — local pbAI assistant with quick actions: explain, bugs, improve, comment.
- **Browser** — pblang-native reader view using `stdlib/browser.sapp` to fetch pages, decode entities, strip scripts/styles, extract links, and parse forms.
- **Help/Manual** — shortcut overlay and documentation-oriented panel.

---

## Web-IDE parity checklist

| Area | Native status |
|---|---|
| Top toolbar/actions | Implemented: run, check, save, clear, restore, panels. Format/auto-fix/examples/open still need native implementations. |
| Code editor | Implemented: typing, movement, selection, find, undo/redo, syntax coloring, output pane. |
| Terminal | Implemented with persistent cwd and scrollback. |
| Designer | Implemented with palette, canvas, drag/move, delete, edit text, and code generation. |
| AI helper | Implemented with local pbAI; no network/Ollama required. |
| Browser reader | Improved: entity decode, whitespace collapse, script/style removal, adjacent-link spacing, absolute links, forms parser. |
| Browser forms UI | Parser exists; editable form cards and submit UI are next. |
| File picker | Pending; current fallback uses the configured buffer path. |
| Visual polish | Ongoing: match web colors/layout while staying native and bitmap-font based. |

---

## Screenshot-driven fixes tracked

The screenshots showed three main native IDE problems:

1. **Greek/web text mojibake or unreadable entity text.** Fixed in `stdlib/browser.sapp` by improving entity decoding, title decoding, whitespace collapse, and reader parsing. The ASCII bitmap font still transliterates/folds non-ASCII at draw time until a full Unicode text renderer exists.
2. **Browser panel looked unlike the web reader.** Improved by separating reader text, links, and parsed forms at the engine level. Forms UI is now the next UI task rather than a missing parser task.
3. **Native IDE parity was unclear.** This document now separates implemented, partial, and pending web-parity items.

Regression coverage: `tests/semantics/browser_native_ide_test.sapp` locks reader parsing, title/entity decode, link resolution, form extraction, and UTF-8 form URL encoding.

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
4. Replace pending placeholders: format, auto-fix, examples picker, open dialog, browser form cards.
5. Retire `index.html` only when the native IDE reaches real workflow parity.
