# Native single-line text editing

The PBLang `gui.textbox` control, including Modern UI `input`, supports:

- Left/Right and Home/End cursor movement, including repeated keys.
- Insertion at the cursor, Backspace before it and Delete after it.
- Click-to-position using the native bitmap renderer's character advance.
- A steady visible caret and horizontal scrolling while focused.
- Shift+Left/Right/Home/End selection, including repeated arrow keys.
- Ctrl+A or Command+A to select all, then typing, Delete or Backspace to
  replace/remove the selection. Plain Left/Right collapse it to either edge.
- A clipped highlight for the visible selected text. Ordinary clicks clear
  selection and place the caret. Left-button dragging selects text; Shift-click
  extends from the existing anchor within the already focused field.

Pointer capture stays with the field where selection started; crossing another
control never activates it. Movement beyond either horizontal edge scrolls in
bounded scalar steps per movement event, not on a timer. A stationary pointer
does not continue scrolling. Left release finalizes new coordinates and ends
capture; a right release does not cancel an active left gesture. Same-position
release never shifts a click because of scrolling. Capture cancels on editing
keys, focus navigation, changed text, invalid pointer events, relayout, widget
removal/hide/disable and close. This is in-window event capture, not an OS-wide
mouse grab or a guarantee about release events lost outside the window.

This replaces the old append-only behavior. To explicitly edit at the end after
clicking elsewhere in a nonempty field, press End. The API, document formats and
existing character limits are unchanged. Enabled/read-only restrictions still
apply. Cursor offsets use Unicode scalar indexes, not UTF-8 byte offsets.
They are not grapheme-cluster, shaped-text or IME cursor guarantees.

`caret`, `selection_anchor`, `text_offset` and `cursor_value` are transient widget state. A changed
external value resets the cursor to the new end, and malformed/stale offsets
are normalized before editing/rendering. These fields do not enter model data
or saved documents. Same-value replacement need not reset the position.

Other Ctrl/Command key combinations are ignored during text editing,
rather than inserting their letter or deleting ordinary text. This does not
implement clipboard copy/paste, word movement or local text Undo.
Replacement counts the resulting characters after removing the selected range;
a rejected insertion keeps the original text and selection. Releasing one of
two held Shift keys no longer clears Shift while the other remains down.
The multiline `textarea` remains
append-only in this slice. Full OS text-input/IME and shaped RTL handling remain
separate work; the current renderer still has bitmap-font limitations.

## Focused evidence

`gui_text_cursor_test.sapp` first failed against the append-only implementation,
then passed with cursor movement, insertion/deletion, Unicode scalars, limits,
read-only/disabled fields, repeat, click placement, programmatic replacement,
modifier suppression and an in-bounds drawn caret/scrolling checks. The existing
binding test now requests End explicitly before deleting the last character.

PBOffice's `sheets_text_edit_test.sapp` uses only event-dispatched synthetic
input: mid-cell correction, Apply, Undo/Redo, Revert drafts, page reuse and path
field editing without file I/O. No user document, real GUI, browser or emulator
was used. These are not physical-platform or full usability claims.

`gui_text_selection_test.sapp` first reproduced missing replacement behavior
and the two-Shift release bug, then passed with the implementation. It covers
both selection directions, collapse, whole-field replacement at capacity,
Unicode scalar boundaries, malformed/stale anchors, no-op unsupported chords,
read-only/disabled/hidden fields, model isolation and clipped rendering.
PBOffice's editing regression also covers selected replacements through
Revert, Apply, Undo/Redo and session-only path editing, without file I/O.

`gui_text_drag_test.sapp` reproduced missing drag replacement before the fix.
It covers forward/reverse release, Shift-click, secondary release, HiDPI
coordinates, edge scrolling, cancellation and same-name widget replacement.
The PBOffice editing test additionally applies and undoes/redoes a mouse edit.

For an explicit visual demonstration, see
[product previews](UI_PRODUCT_PREVIEWS.md). Web and Android use their respective
renderer controls; these native pointer tests do not establish platform parity.
