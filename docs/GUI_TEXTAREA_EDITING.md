# Multiline textarea editing

`gui.textarea` uses the same Unicode-scalar caret and selection editing as
`gui.textbox`. Existing constructors and model bindings are unchanged. Initial
or externally replaced values put the caret at the end, retaining append behavior.

- Left/Right move by one scalar, including across a newline. Without Shift,
  an existing selection collapses to the corresponding edge.
- Home/End move to the beginning/end of the current logical line, including
  empty and trailing lines. Shift extends selection.
- Backspace/Delete remove the selection or the adjacent scalar/newline.
- Enter inserts a newline at the caret, replacing any selection.
- Ctrl+A or Meta+A selects the entire value. ASCII A/a/KeyA and the ordered
  Greek layout keysym names Greek_alpha/Greek_ALPHA are recognized. This also
  applies to textbox. Other layout shortcut identities are not implied.
- Committed text replaces the selected range atomically; the capacity check
  excludes removed scalars. Existing default limits remain 65,536 scalars for
  textarea and 4,096 for textbox, and 256 scalars per committed-text event.
  Invalid/control-containing explicit text never falls back to a physical key.
- Readonly, hidden and disabled widgets reject editing. Cursor/selection and
  viewport fields are transient widget state, excluded from bound model values.

A focused editable textarea draws a steady caret and selection with a viewport
that follows it horizontally and vertically. Logical lines do not wrap. Resize
recomputes the visible capacity on redraw. The editable interior uses the existing
pixel-clipped bitmap canvas (including when the form requests automatic fonts);
unsupported bitmap glyphs retain the existing placeholder. Unfocused and readonly
areas keep the existing text-lines rendering. Zero-size interiors draw no caret.

This bounded keyboard slice does not implement Up/Down, word or grapheme motion,
IME/preedit, clipboard, wrapping, mouse placement/drag selection or scrolling.
Clicking an area focuses it; keyboard navigation edits inside it. Combining marks
and emoji sequences consist of multiple independently editable scalars. The shared
GUI default event loop still uses its existing native input backend; consumers
requiring ordered Greek commits must explicitly use the documented ordered input
path. This change cannot recover events missing from the native backend.

## Validation

`tests/semantics/gui_textarea_edit_test.sapp` checks editing, scalar boundaries,
selection capacity, model updates, protected fields and recorded drawing geometry.
Run it with the candidate stdlib resolved in a local validation project. A retained
runtime using an old installed stdlib does not exercise these changes.

`tests/tooling/native_textarea_input.py` requires an isolated Xvfb display and the
existing `xdotool`, `setxkbmap` and ImageMagick `import` tools. It types into an
initially empty real window, using stable Greek and US layouts, and asserts the
intermediate `αβ\nγXδ` and final `Ω` values plus exact saved/reopened UTF-8 bytes.
It uses `XMODIFIERS=@im=none` because the isolated display has no IBus service.
Key delays are 1 ms; waits between layout changes and screenshots are not a claim
about arbitrary throughput. The result proves this Linux/X11 fixture path, not
product installation, other native platforms, complete IME support, or performance.
