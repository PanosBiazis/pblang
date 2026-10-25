# PBIDE form preflight

Native PBIDE: open a UTF-8 text `.dfm` buffer and press F6. The checked buffer is
not executed or saved. Other suffixes retain their existing source-check route.
The selected-buffer adapter accepts at most 65,536 UTF-8 bytes.

Web editor pilot: run `pblang run editors/ide/pbide_text_web_main.sapp --allow-all`
locally, paste a text DFM into the current draft and press **Check DFM form**. The
pilot's existing 4096-byte text and per-page history budgets remain unchanged. The
file picker also accepts `.dfm`; filenames are not retained. Existing undo/redo and
draft behavior is preserved. Stop the local listener with Ctrl+C.

Both use `ui_form_check.sapp`, shared with the CLI and PB-MCP. Reports show whether
the structural subset can render and how many properties/events remain unmapped.
Imported handlers remain inactive. A positive report never claims full application
conversion. Binary DFM and unsupported controls fail explicitly. No language-wide
compiler limit is added. No native Designer or imported Pascal execution is enabled.

The checker report is value-free; the editor itself necessarily displays your
source and transports drafts/history in its submitted page. This does not provide
account isolation or erase browser history. The existing loopback editor must
not be published as an authenticated application. This change does not retrofit
all form-preview policy defaults into the older PBIDE web pilot.
