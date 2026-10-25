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
are separate. Known acquisition errors now also populate the primary scope
error fields as described below. `with_file` is unchanged.

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
body error, throw and invalid return. The historical cycle21 before/after comparison records 68 cases
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


## Known primary error through cleanup (2026-10-09, cycle24)

`with_files` returns a known rejected load as `{ok:false,error:fixed_message}`
inside its existing resource scope. This preserves that message in `error` and
`body_result`, sets `original_error_available:true` and `body_status:returned`,
and still skips the application callback and attempts prior closes in reverse
order. The acquisition diagnostic, index, stage, counts and cleanup entries are
unchanged. This is a deliberate correction of those four diagnostic fields;
it does not claim access to arbitrary caught exceptions. The earlier cycle21
measurement above remains an immutable historical result.

A primary error and a cleanup error can both be present. The known acquisition
message wins the primary slot while a throwing/nonboolean close stays in
`cleanup_errors`. A valid idempotent false from an already closed bundled handle
is accepted. No retry is added; a close that throws before release leaves cleanup
unconfirmed, even though a test can later perform its own diagnostic close.

Run `python3 tests/tooling/pbai_checkpoint_primary.py --product pblang` from PBLang
or `--product pbai` from standalone with the complete trusted `PBLANG_BIN` launcher.
Linux/Python/bubblewrap and the existing private-file exec plus exact libc/11-symbol
profile are required. Real synthetic files exercise missing/invalid encoding after
an acquired handle, normal/already-closed/malformed/throwing closers, recovery,
first-file rejection, explicit body error with early close, opaque body exception
and successful body. Original saved methods are observed before test cleanup;
source-hidden copies and isolated `PBAI_STATE_ROOT` are removed after execution.

`--phase baseline --baseline-owner FILE` compares an explicitly restored original
scope. `--provider-stdlib DIR --expected-owner-sha256 HASH` qualifies a pinned
provider copy. Neural-handle observation wrappers are test-only. Installed-copy
qualification is separate from shared launcher activation or unmodified production
acceptance. See [scoped evidence](evidence/pbai_checkpoint_primary-2026-10-09.json).
