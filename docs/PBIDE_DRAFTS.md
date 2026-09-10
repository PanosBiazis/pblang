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
Copy your work before closing: there is no server persistence, and browser
Back entries can retain older text even after Discard or Clear history.

Each source fits 4096 UTF-8 bytes. All history frames across all drafts share
8192 bytes, with at most nine snapshots per draft. Edits can evict that draft's
oldest snapshots; combined overflow rejects the operation without silently
pruning other drafts. Selection uses explicit Unicode scalar offset fields.

Check source runs the self-hosted linter on only the active draft, without
resolving its imports or executing it. Source, tokens, nesting and rendered
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
98304 bytes respectively, including bounded transport enforcement; four requests
can be in flight. Synchronous handlers do not have a hard CPU/heap deadline.

Tests cover independent histories, new/select/discard, no final-tab discard,
canonical codec roundtrip, malformed IDs/separators, byte limits, history
splicing, legacy compatibility, escaping, non-echoing rejection, four-tab
worst-case form/page sizes and actual loopback exchange. A browser verified
retained drafts, independent Undo and Check on the active draft.

File import/export, automatic caret binding, highlighting, Format/Run, Designer
and bidirectional source synchronization remain unimplemented in this pilot.
