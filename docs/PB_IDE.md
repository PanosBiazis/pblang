# pblang-native IDE — browser-free, for total security

> **Goal (yours).** Make the IDE **completely written in pblang — no HTML, CSS, or JS** —
> for the scope of **total security**. This is the self-hosted Studio the project already
> commits to (`docs/Visual_Designer.md`): the editor and designer are a *pblang
> application* on pblang's own runtime, not an HTML/JS app that talks to pblang.

---

## Why — the security argument

The browser IDE (`editors/ide/index.html`) is the bootstrap, but as a security surface a
web IDE drags in **everything a browser is**:

| Web-stack layer | Attack surface it adds |
|-----------------|------------------------|
| HTML parser | malformed-markup / injection bugs |
| DOM + `innerHTML` | **XSS** (we already had to fix one — PrjAnalysis review) |
| CSS engine | layout/parser exploits, data exfiltration tricks |
| JS engine | the largest attack surface in computing (JIT bugs, prototype pollution) |
| `fetch`/iframes | SSRF, content-injection, sandbox escapes |

A pblang-native IDE **deletes all of those layers**. The whole IDE is: a native window
(`gfxcreatewindow`), a pixel buffer, key events (`gfxpollevents`), and pblang code —
running under pblang's **capability model** (`docs/Capability_model.md`, `docs/Safety.md`).
There is **no markup to inject, no script engine to exploit, no DOM**. "Total security"
here means *removing* the dangerous layers, not hardening them. It also dogfoods the
language (the IDE is built with the thing it edits) — the same "all the way down" property
as self-hosting the compiler.

---

## Architecture (all pblang)

```
        ┌─────────────── editors/ide/pbide.sapp ───────────────┐
        │  window + event loop (graphics.sapp → minifb/SDL2)    │
        │   • keydown → editor ops      • F5 → run via frontend │
        ├───────────────────────────────────────────────────────┤
        │  editor_model.sapp   buffer + cursor + edits  (tested) │
        │  bitmap_font.sapp    5×7 glyphs → pixels      (tested) │
        │  graphics.sapp       Window / point / rect / present   │
        └───────────────────────────────────────────────────────┘
```

- **`stdlib/editor_model.sapp`** — the editor core: a line buffer + cursor and the edit
  ops (`insert_text`, `newline`, `backspace`, cursor movement, clamping). Pure pblang, no
  UI dependency → **fully tested in the VM** (`tests/semantics/editor_model_test.sapp`).
- **`stdlib/bitmap_font.sapp`** — there is *no text-rendering builtin*, so the IDE draws
  its own glyphs: a 5×7 font where `draw_text` emits pixels through a `setpx(x,y,color)`
  callback (drives a real window, or a recording surface for tests). **Tested in the VM**
  (`tests/semantics/bitmap_font_test.sapp`) by verifying a glyph's exact pixels.
- **`editors/ide/pbide.sapp`** — the window binding: creates the window, draws the buffer
  (line numbers + code + cursor + status bar) with the font, maps key events to editor
  ops, and runs the buffer through the self-hosted frontend on F5. **Verified to run** —
  it opens the window and executes the full draw/event loop without error.

---

## Status & honest gaps

- **Done + tested (no display needed):** editor model, keymap, syntax-highlighting
  token flow, and font renderer are pure pblang and covered by VM tests.
- **Runs:** `pblang --ide` launches `editors/ide/pbide.sapp` with `--allow-all`
  by default. The resolver checks `PBLANG_IDE`, the current directory,
  executable-relative install paths, the build-time repo path, and standard
  share installs (`/usr/local/share/pblang`, `/usr/share/pblang`).
- **Current native panels:** code editor, terminal, visual designer, pbAI chat,
  and pblang browser. The browser/AI views hard-wrap long lines, and the bitmap
  font folds common Unicode punctuation plus Greek letters to readable ASCII.
- **Honest gaps:**
  - **Interactive verification** still needs human window testing for real
    keyboard layouts, focus behavior, and display backends.
  - **Font fidelity** is still ASCII-first. Non-ASCII is made readable through
    folding/transliteration, not full Unicode glyph rendering.
  - **Performance:** text is drawn through the bitmap-font path. A host
    `gfx_draw_text` primitive or row-run batching would help very large files.

## Migration plan

1. **Done:** editor model + font renderer (pblang, tested) + a running native window.
2. **Done:** keymap/modifiers, scrolling viewport, selection, output pane, syntax highlighting.
3. **Done:** native terminal, designer, pbAI, and browser panels.
4. Next: add visual regression fixtures for the screenshot issues and improve font/text performance.
5. Retire `index.html` once the native IDE reaches parity — the IDE is then 100% pblang,
   with no web attack surface.
