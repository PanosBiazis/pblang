# PBIDE bounded draft editor

Launch the PBLang Web editor from a trusted checkout:

```sh
target/release/pblang run editors/ide/pbide_text_web_main.sapp --allow-all
```

Open its printed loopback URL. Stop the launcher when finished; closing a browser
tab does not stop the server. This pilot uses the bounded Linux transport.

Four drafts have separate Undo/Redo histories. New draft and selection first
record the visible text. Discard explicitly removes the active draft; the last
draft cannot be discarded. Draft IDs are local fixed labels, not filenames.
Use Download .sapp or copy your work before closing: there is no server persistence, and browser
Back entries can retain older text even after Discard or Clear history.

Each source fits 4096 UTF-8 bytes. All history frames across all drafts share
8192 bytes, with at most nine snapshots per draft. Edits can evict that draft's
oldest snapshots; combined overflow rejects the operation without silently
pruning other drafts. Selection uses explicit Unicode scalar offset fields.

Check source runs the self-hosted linter on only the active draft, without
resolving its imports or executing it. Incomplete radix literals and exponents
produce [numeric diagnostics](SELFHOST_NUMERIC_LITERALS.md); preview preserves
the original text as an escaped plain fallback. Source, tokens, nesting and rendered
diagnostics have independent bounds. This does not hard-preempt computation.

The flat draft codec admits at most 17664 bytes and eleven fields. Only fixed
IDs 1..4 are valid. Nested history is validated using the existing flat history
codec, with a shared aggregate source budget. Eight-field forms require the
standalone active history to match the workspace's active history. This is
consistency validation of forgeable browser state, not authenticated storage.
Legacy six- and seven-field requests remain supported.

HTTP admits exact loopback Host and same-origin POSTs, rejects duplicate/unknown
fields and malformed encodings, and escapes source, history, workspace and
diagnostics. The no-script CSP remains. Body and response limits are 65536 and
131072 bytes respectively, including bounded transport enforcement; four requests
can be in flight. Synchronous handlers do not have a hard CPU/heap deadline.

Tests cover independent histories, new/select/discard, no final-tab discard,
canonical codec roundtrip, malformed IDs/separators, byte limits, history
splicing, legacy compatibility, escaping, non-echoing rejection, four-tab
worst-case form/page sizes and actual loopback exchange. A browser verified
retained drafts, independent Undo and Check on the active draft.

## Source preview and download

Highlight source records the submitted draft through the existing history path
and displays a lexical preview below the editor. Keywords, strings, numbers and
comments have fixed colors. The preview uses the compiler tokenizer's physical
Unicode scalar positions and escapes slices of the original source, preserving
literal spelling, comments, whitespace and escapes. It never renders decoded
token values or evaluates source. It is refreshed on submission, not as you type,
and is separate from Check source.

Preview source is capped at 4096 UTF-8 bytes, tokens at 512 including EOF, and
rendered markup at 32768 bytes. Invalid positions or a token/output limit produce
a complete escaped plain-text preview. Input beyond the source limit is rejected.
The fixed no-script CSP remains unchanged.

Download .sapp sends the visible LF-normalized source as a plain UTF-8 attachment
named Draft1.sapp through Draft4.sapp, according to the active draft. Source never
enters response headers. The server validates selection, source and existing
workspace/history consistency but does not commit the submitted edit into
history. Download therefore remains usable when a new history snapshot would
exceed the shared byte limit. There is no host file write, filename/path input,
server persistence or acknowledgment of a saved browser file. The browser controls
where or whether the attachment is saved.

Focused regressions cover exact Unicode/escape round trips, incomplete literals,
HTML injection text, token and output fallback, full-budget export, canonical
filenames, malformed/spliced state and combined worst-case page size. Seven real
loopback exchanges cover edits, history, drafts, preview, attachment and origin/host
rejection. Browser interaction verified visible colors, literal text and draft
switching; browser download-manager completion was not asserted.

## File import

Import file first records visible edits through the normal history path and
opens a separate no-script file chooser. Choose a UTF-8 file, then Open as new
draft. Return to editor restores the captured workspace without adding a draft.
At four drafts the import button is disabled. A successful upload creates one
new draft with one history frame; existing drafts and their histories remain.

The raw file limit is 4096 UTF-8 bytes, checked before removing one leading BOM
and converting CRLF to LF. Bare CR, NUL and invalid UTF-8 are rejected. A selected
empty file is accepted. Filename and MIME metadata are validated and discarded;
no path is used, no file is written by the server, and source is never executed.
The draft has no continuing connection to the original file. Use Download .sapp
to export later edits.

The dedicated multipart parser accepts exactly two ordered parts: workspace,
then upload. It rejects duplicate/unknown headers, fields and disposition
parameters, malformed framing and any extra boundary-looking line. The supported
Content-Type has a single quoted or unquoted ASCII alphanumeric/underscore/hyphen
boundary of 1..70 characters. File Content-Type is optional and parameter-free.
Each part has at most two header lines and 512 header bytes. The upload endpoint
rejects a Content-Length above 24576 in preflight, before waiting for the body,
and verifies the actual byte length again. Existing loopback Host/Origin and
no-script response protections apply.

A valid workspace is copied before draft creation. File or shared-history quota
failure returns the unchanged canonical workspace on a retry page and discards
the rejected upload. Malformed multipart or workspace receives a generic error;
use browser Back to return to the chooser in that case. Imported source counts
toward the existing 8192-byte aggregate history budget; other drafts are never
pruned to make room. Browser state remains forgeable and is not persistent storage.

Three import suites cover parsing, state transitions and five actual HTTP
exchanges, including invalid UTF-8 and header-only oversize rejection before the
handler. An embedded Chromium browser verified a real file chooser/upload,
BOM/CRLF normalization, literal HTML text, Unicode, preservation of unsubmitted
edits, cancel and oversize failure recovery. Other browser engines and operating
systems have not been validated for this multipart subset.

Automatic caret binding, live highlighting inside the editable
field, Format/Run, Designer and bidirectional source synchronization remain
unimplemented in this pilot. Runtime hard resource enforcement and broader
browser/OS coverage remain separate work.
