# Bounded low-level review — 2026-09-06

Initial source-only defensive review of the retained native FFI dispatch/policy
and PBCK load/deserialization paths, followed by the scoped PBCK remediation
below. No unsafe foreign call, exploit or device test was executed. FFI findings
remain open. JIT, allocator internals, dependency advisories and
all platform implementations were not exhaustively reviewed.

## High: caller-controlled ABI within an FFI grant

`FfiPolicy` in `crates/pblang-vm/src/vm.rs` authorizes libraries and symbols,
not exact signature tuples. `vm/ffi.rs` raw calls use `extern C fn(f64...)`;
typed calls still accept the signature from the script. A mismatched declaration
or arity can cause host undefined behavior. Preconditions are an enabled FFI
capability and an allowed library/symbol (or allow-all); this is not a claim
that default-denied FFI is reachable from the sandbox.

Required fix: enforce a host-owned exact library identity, symbol, calling mode,
argument/return types and platform tuple; close the raw bypass. A PBLang metadata
contract cannot enforce native machine calling conventions.

## High: ABI widths, numeric pointers and unbounded string returns

`vm/ffi.rs` collapses i32/i64/bool/ptr into integer dispatch keys and f32/f64
into floating dispatch keys. `vm/ffi_dispatch.rs` uses i64/f64 trampolines;
numeric casts lack complete finite/integral/range validation. Pointer results
can lose bits through f64, and `c_str_to_value` uses `CStr::from_ptr` without a
length bound. Incorrect declarations or invalid foreign pointers can produce
undefined behavior or memory faults. Accepted type labels are not proof of ABI
support. The former correct-width assertion in `FFI_contract.md` is corrected.

Required fix: reject unsupported shapes until exact trampolines exist, validate
numeric ranges, use opaque pointer handles, and define bounded buffer length,
ownership and lifetime contracts. Verify later with isolated native fixtures.

## Medium: PBCK allocation and canonicality

Originally `src/main.rs` read PBCK using whole-file `std::fs::read` before
parsing, and native deserialization accepted trailing bytes/noncanonical flags.
The follow-up replaces that read with `pbck_read_bounded`: at most 64 MiB of
payload, an 8 KiB scratch buffer, and one excess byte consumed only to detect
overflow. Growth requests are fallible and geometrically bounded; file metadata
is not trusted. The slice deserializer independently checks the same byte cap,
requires full consumption, and accepts exactly 0/1 at all six flag sites.

Serializer-produced PBCK v1 layouts are unchanged. Previously accepted trailing
bytes/noncanonical flags and artifacts above 64 MiB are intentionally rejected.
The writer remains unbounded and can still produce an oversized artifact that
the reader refuses; checked serialization/publication is a follow-up.

CLI instruction/memory budgets still default to unset on the reviewed path.
Decoded allocation amplification, allocator overhead, blocking-stream deadlines,
namespace/snapshot consistency and structural bytecode verification are not
solved by the input byte cap. Untrusted bytecode needs bounded execution; this
review found a DoS/canonicality risk, not demonstrated native out-of-bounds
corruption in the checked-reader bytecode path.

Remaining: validate structural references/control flow and apply explicit
untrusted-execution budgets. Regression fixtures never execute hostile programs.

## PBLang-only byte-array admission

`host_pbck_inspect.sapp` and `host_pbck_decode.sapp` now share a complete byte-array
validation pass before parsing. This rejects non-numbers, NaN/infinity,
fractions, negative values and values above 255, including bytes in payloads
the inspector skips (numeric constants/source spans). The pass is O(n), retains
no copied payload, and follows the existing 64 MiB length gate. Decoder `compact`
must be a boolean; absent/nil options retain normal decoding. Valid arrays and
both compact/noncompact output schemas remain compatible.

This is not an immutable concurrent-array snapshot. Their `decode_file` and
`inspect_file` wrappers still call `fs_read_bytes` before admission, so they do
not inherit the native CLI's bounded file-read guarantee.

Focused regression evidence: native PBCK tests first reproduced trailing-byte
and noncanonical-flag acceptance, then passed after remediation. The new
`host_pbck_byte_input_test.sapp` first reproduced skipped-payload non-byte
acceptance, then passed with decoder/inspector/PBCS hardening regressions.
CLI fixtures now use unique auto-cleaned directories and compare raw output
bytes, not lossy UTF-8 conversions. Validation on Linux x86-64: 11 focused IR
tests, 3 freshly built native CLI integration tests and 4 PBLang regressions
passed. The four PBLang tests and three source checks also passed against the
curated commit snapshot, independent of unrelated working-tree changes. Debug
and optimized native builds completed. No full suite or physical-platform
verification was performed. Native/PBLang lint completed without errors, with
style/unused-binding warnings retained. Graphify update reached its 20-second
deadline; the graph update is not claimed complete.

The 3 CLI integration tests above ran in the debug test profile. A separate
optimized CLI test build hit 60-second and then 90-second limits while compiling
test dependencies/the binary; no optimized CLI integration-test result is
claimed. The normal optimized binary build had completed, and all four PBLang
regressions passed using that binary. Both timed-out builds were terminated;
timeouts were not retried into a reported test success.

## Follow-up: PBLang default-value recursion

Both PBLang PBCK readers checked depth in `_chunk`, but function-valued defaults
recurse through `_value` before any body chunk is reached. A deeply nested
encoded prefix could therefore grow the parser's call stack past the intended
64-level bound before a later truncation/body check rejected it. This is an
availability/correctness finding, not demonstrated native memory corruption.

`_value` now checks depth on entry, before reading another tag, in both modules.
The existing `depth > MAX_DEPTH` boundary is preserved. The decoder also checks
default/type counts against remaining bytes before their loops, matching the
inspector's preflight. Bytecode format and returned report schemas are unchanged.

`host_pbck_default_depth_test.sapp` verifies complete 64-function default chains,
the retained table-count semantics, both decoder layouts and rejection at 65.
`tests/tooling/host_pbck_default_errors.sh` checks nine exact exit/diagnostic cases
against a pinned local release executable, with bounded child time/memory/output
and private temporary logs. The 66-level truncated prefix produced the wrong
late error before the fix; it now produces the nesting-limit error. These are
small parsing-only fixtures; no guest program or host-crashing input is executed.
The source test is portable; the supervisor requires Bash and GNU timeout and
has been exercised on Linux only.

Validation: run `bash tests/tooling/host_pbck_default_errors.sh` for the nine
diagnostic cases. The `host_pbck_default_depth`, `host_pbck_byte_input`,
`host_pbck_decode` and `host_pbck_inspect` semantic tests all passed, both in the
working tree and in a curated commit snapshot. Three source checks found no
errors/warnings; native lint retained existing style/unused-binding warnings.
Package build and portable-lock publication passed. No full suite or native
rebuild was needed. This follow-up's Graphify update also hit the 20-second
deadline, so graph freshness remains unverified.

At this stage chunk `constants` counters did not count default-value nodes;
the next follow-up adds a separate value-count budget without reinterpreting
those report fields. The inaccurate every-allocation-is-budgeted comment was removed. Unbounded
PBLang file reads, snapshot consistency, execution budgets and the linker's
different depth-accounting model are not changed by this slice. No Rust source
or language catch semantics were modified.

## Follow-up: aggregate decoded-value budget

Both PBLang readers now have a default/hard ceiling of 1,000,000 decoded values.
Each scalar constant, function constant, function default and nested chunk
constant consumes one unit before its tag/payload is read. Constant-table and
default counts receive an additional remaining-budget preflight before loops.
The per-value check remains authoritative because a function can contain more
values than its immediate table counts reveal. Counters are private and reset
per read, not per chunk or function.

`decode_bytes_with_options(bytes, options)` now accepts `max_values`. Inspection
adds `inspect_bytes_with_options(bytes, options)` while preserving the exact
one-argument `inspect_bytes(bytes)` wrapper. Nil/absent options or an absent key
use the hard ceiling. An explicit `max_values` must be an integer in
`0..1000000`; explicit nil, booleans, strings, NaN/infinities, fractions and
out-of-range values are rejected without echoing the input. Unknown option keys
remain ignored for compatibility. `Inspect.MAX_TOTAL_VALUES` and the shared
`value_limit_from_options` validator keep the two paths aligned.

Example (one nil constant; no instructions, spans or source metadata):

```pblang
let Decode = import("stdlib:host_pbck_decode.sapp");
let Inspect = import("stdlib:host_pbck_inspect.sapp");
let bytes = [80,66,67,75,1,0,0,0,0,1,0,0,0,2,0,0,0,0,0];
let decoded = Decode.decode_bytes_with_options(bytes, map{ max_values: 1, compact: true });
let requirements = Inspect.inspect_bytes_with_options(bytes, map{ max_values: 1 });
```

The public `constants` field still counts aggregate constant-table entries,
not defaults. No budget fields were added to output schemas. Format/layout is
unchanged; previously admitted artifacts exceeding the new combined value
ceiling are intentionally rejected. File wrappers and the byte wrappers use
the fixed default; tighter limits are explicitly selected through these options
APIs, not automatically forwarded from separate VM/linker options.

`host_pbck_value_budget_test.sapp` covers exact small limits, nested defaults,
nested tables, siblings, scalar tags, invalid configuration and exact wrapper
result parity. It first demonstrated ignored `max_values`, then passed after
the fix. Tests use tiny artifacts rather than allocating a million-node tree.
Five selected semantic tests (value budget, default depth, byte input, decoder,
inspector) and the nine existing exact-diagnostic subprocess cases passed in
the working tree and curated commit snapshot. Package build and portable lock
publication passed. Three source checks found zero errors/warnings; lint kept
existing style/unused-binding warnings. No full suite or native rebuild was run.
Graphify update reached its 20-second limit again and is not marked complete.

This limits decoded value count, not bytes of heap: variable-sized payloads,
map/object overhead, instructions/captures, input-array allocation, unbounded
`fs_read_bytes`, native PBCK decoding and execution budgets remain separate.
It is not an immutable snapshot, sandbox, publisher check or 1.0 readiness claim.

## Publisher boundary: measurement is not authorization

`publisher_artifact_admission` and `publisher_trust_store` accept caller-selected
trust-store/floor roots. Existing regressions show self-selected trust and an
alternate replay-history root. They already keep operator origin, authenticity,
rollback enforcement and execution authorization false; current callers are
tests/docs, not an installer/executor bypass demonstrated by this review.

No extra always-false gate or forgeable PBLang policy map was added. Real closure
requires an effect boundary consuming an operator-owned, non-forgeable bootstrap
capability and the same held artifact descriptor. Preserve `prepare().ok` as
experimental operation success, never as installation/launch permission.

## Implementation constraint and release gate

The user explicitly authorized targeted retained-native fixes for this review;
new product functionality remains PBLang-first. The self-hosted compiler/VM and
native emitters coexist with the retained `src/main.rs` / native VM path used
by `pblang run file.pbck`; self-hosting does not imply every installed executable
uses the same implementation. Hard heap/preemption, isolation,
revocation freshness and physical platform evidence remain release requirements.
See the final compatibility gate in `ROADMAP.md` before any 1.0.0 release.
