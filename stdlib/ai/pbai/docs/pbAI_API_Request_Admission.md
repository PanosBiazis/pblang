# One-shot request-file admission

The local CLI bridge admits one request before importing the model facade,
tokenizer or brain. This is a file boundary, not an HTTP endpoint or a general
sandbox for the selected action. No new permission grants or global installation
are supplied by the bridge.

## Input and limits

| Input | Contract |
|---|---|
| Filename omitted or empty | Use an empty object immediately, without importing or probing the filesystem backend. Existing no-file status usage remains available. |
| Request bytes | At most 983040 (960 KiB), including whitespace and UTF-8 encoding. One bounded backend observation loads at most 983041 bytes and rejects overflow. |
| JSON containers | At most 32 levels, counting the root object as level 1. A byte scan checks nesting before JSON parsing; escaped quotes and braces inside strings do not increase depth. |
| Encoding | Exact valid UTF-8. Decode/re-encode must preserve every input byte; malformed, overlong, truncated and surrogate byte sequences are rejected. JSON grammar rejects a leading BOM. |
| JSON root | One object. Empty files, scalar/array/null roots, malformed syntax, non-finite overflow numbers and trailing data are rejected. Existing duplicate-key last-wins behavior and per-action field handling remain unchanged. |
| Path | At most 4096 UTF-8 bytes, with components at most 255 bytes; no NUL, backslash, traversal, empty interior component or trailing slash. A single leading `./` is normalized. |
| Filesystem | Current Linux descriptor backend with existing filesystem, FFI and execution authority. No symlink path component or final symlink; only descriptor-verified, seekable regular files. No ordinary unbounded path-read fallback. |

The underlying prefix backend reserves the leaf name `root`, and its relative
root opener also reserves the parent `root`. Therefore `root`, `dir/root`,
`root/file.json` and `./root/file.json` are rejected by this profile. This is a
known path compatibility limit, not an instruction to rename or modify user data.
A missing file, unreadable file, directory, FIFO, socket, device, unavailable
backend, unsupported platform or denied capability produces a fixed rejection.
Other platforms are not silently given a weaker file-reading implementation.

The original file is never overwritten, chmodded or retained as a persistent
copy by admission. Its creator and access permissions are not authenticated;
operators should protect sensitive request files themselves. Read-only admission
does not change persistence or output policies of a successfully selected action.

## Output and ordering

Successful calls preserve `{ok:true,action,value}` and existing action behavior.
An admission failure prints one bounded JSON envelope and returns, preserving the
existing exit-zero JSON-error convention:

```json
{"ok":false,"action":"request","command":"request","error":"request must be a valid JSON object","value":null}
```

`action` and `command` use the constant `request` for admission errors, even if
the CLI action is unknown or very long. The bridge never truncates, hashes or
reflects that untrusted action, request text, path, parser diagnostic, descriptor,
owner metadata or content fingerprint into this envelope. The error is chosen
from fixed internal messages. Payload fields called `format` or `error` remain
ordinary user data and cannot forge an internal admission result.

An omitted file preserves the empty-object request for **all** actions, including
explicit training actions; it is not a new authorization decision. With a supplied
rejected file, no facade import or action dispatch occurs. In particular, malformed
`learn` and `tool_call` requests do not proceed with empty defaults.

Standalone use, from its project root, retains the existing command shape:

```text
pblang run tools/pbai_api.sapp -- status
pblang run tools/pbai_api.sapp -- tokenize ./request.json
```

The second command requires an already-authorized Linux descriptor profile; a
runtime that denies FFI or execution receives the fixed failure instead. The bridge
does not enable those capabilities. The first command's admission path does not
require them. Other capabilities required by a selected action remain its concern.

The transitional PBLang copy is run from the **PBLang project root**:

```text
pblang run stdlib/ai/pbai/tools/pbai_api.sapp -- status
pblang run stdlib/ai/pbai/tools/pbai_api.sapp -- tokenize ./request.json
```

Its bridge imports its existing transitional facade/tokenizer/brain explicitly
through the owning stdlib path. Running from the nested snapshot directory is not
a supported dependency-layout profile. This admission maintenance does not make
the snapshot a second source of truth or synchronize its model implementation.

## Cleanup and limits of the guarantee

The product helper checks the backend's cleanup-poison state before acquisition.
It catches a reader exception, attempts root close exactly once without retry,
requires a boolean successful close, and rechecks the poison state before using
bytes. Parsing happens after that close. Failed cleanup blocks admission.

The shared backend's individual internal close failures poison its status, but an
unexpected native exception may still leave an internal descriptor until this
one-shot process exits. This wrapper does not claim complete recovery from every
native fault, hard kill, OOM, hung I/O or process crash. It does not prove physical
memory erasure or immediate RAM reclamation.

File type/size observation and copying use a held descriptor. Concurrent mutation
of the same inode is not an atomic, authenticated snapshot: the accepted object is
the bounded observation returned by the reader, not a guarantee about subsequent
contents. There is no product-enforced wall-clock deadline for filesystem access or
the existing metadata subprocess, nor an exact heap ceiling. Test timeouts do not
establish those runtime guarantees. File bytes and JSON depth are bounded; accepted
model actions retain their separate resource policies.

## Qualification

`tests/semantics/pbai_api_request_test.sapp` checks pure parsing and omitted input.
The owning `tests/tooling/pbai_api_admission_check.py` runs the actual bridge, private
filesystem cases, before/after baseline probes, and injected dependency failures.
It checks source and staged source-hidden execution with network denied, isolated
`PBAI_STATE_ROOT`, private fixtures and bounded child processes. Fault mocks check
product ownership decisions; they do not establish native kernel cleanup.

For PBLang, the semantic file is under the transitional tree and the orchestration
harness is in the PBLang root's `tests/tooling/`. Qualification needs an explicit
`PBLANG_BIN`, `--product` and `--baseline-ref`, and should run serially under the
coordinator's existing workload lock. Results and failed attempts belong in the
scoped evidence packet; no whole-product readiness is implied by this suite.
