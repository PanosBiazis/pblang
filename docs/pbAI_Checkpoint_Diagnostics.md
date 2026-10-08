# Checkpoint acquisition diagnostics

The shared owners add `student_decode_result(text)` and `load_file_result(path)`.
Both return exactly `{ok:true,value:...}` or `{ok:false,error:...}` for known
outcomes. Decoding returns a detached validated artifact; loading transfers one
live handle which the caller must close. Existing `student_decode` and
`load_file` keep their throwing contracts and fixed messages. Unexpected host
exceptions can still throw. There is no retry, fallback or extra capability.

The strict decoder retains its 4096-byte, depth-4, 64-item, 128-line limits and
rejects duplicate keys. File loading retains the private Linux storage backend.
The old save/load facade binding remains a two-operation provider contract.

`with_files` uses the new load result before entering the runtime's opaque catch.
Its additive `acquisition_error` is nil or `{index:0..7,error:fixed_message}`.
Known rejection messages distinguish invalid encoding, invalid checkpoint,
invalid path, unsupported platform, and bounded private-storage read rejection.
The latter does not reveal missing-versus-permission details. No source path,
backend payload or checkpoint content is included. Body errors and cleanup errors
are separate; the old report fields remain unchanged. `with_file` is unchanged.

## Verification

Run `tests/semantics/pbai_checkpoint_result_test.sapp` with network, HTTP, exec,
FFI, spawn, graphics and SQLite denied. It covers negative encoding/artifact/path
cases, successful detached decoding and legacy throws.

On Linux with Python 3 and bubblewrap, set `PBLANG_BIN` to a trusted runtime and
run `python3 tests/tooling/pbai_checkpoint_diagnostics.py --product pblang`
(or `--product pbai` from the standalone repository, alongside its pblang sibling).
The gate copies actual source, hides original trees, denies network, creates only
synthetic private files and removes them at completion. It observes real handles
without replacing their numerical methods. File operations require the existing
exec category and an exact libc library with 11 symbol grants; exec is not an
executable allowlist. This qualification is Linux-specific.

Each product exercises 34 direct/facade cases: missing, malformed encoding,
invalid artifact and duplicate keys at positions 0, 1 and 7; groups of 1 and 8;
body error, throw and invalid return. The before/after comparison records 68 cases
and 188 closed handles per phase, with 0 then 48 known diagnostics, identical old
report fields and file bytes, and equal standalone/embedded observations. This is
explicit lifecycle evidence, not physical memory reclamation or a speed claim.
`--phase baseline` is for a deliberately restored pre-change source fixture; it
is not a runtime compatibility mode. The existing batch fault gate also adds 32
known-acquisition cases per product, including failing cleanup. Its original
130-case output marker describes the original subset; total assertions cover
162 cases per product.

## Example

Create `student.json` with the existing file example in a fresh private directory.
Run `examples/pbai_student_checkpoint_diagnostics.sapp` (embedded) or
`demo/pbai_student_checkpoint_diagnostics.sapp` (standalone), explicitly setting
`PBAI_CHECKPOINT_TEST_DIR` and `PBAI_CHECKPOINT_CANDIDATE` to `missing.json`,
`corrupt.json` or `student.json`. The corrupt fixture contains `{}`. The example
reads only and reports the known cause and cleanup count; it never activates a
model. Source-qualified checks do not establish installed-release acceptance.

No AGI/ASI, trained specialist, universal device support, authenticated model,
physical zeroization, hard-termination cleanup or new performance claim is made.
