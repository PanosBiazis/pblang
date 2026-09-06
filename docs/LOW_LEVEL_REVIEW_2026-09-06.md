# Bounded low-level review — 2026-09-06

Source-only defensive review of the retained native FFI dispatch/policy and
PBCK load/deserialization paths. No unsafe foreign call, exploit, native build
or device test was executed. Findings remain open; documentation changes are
not runtime remediation. JIT, allocator internals, dependency advisories and
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

`src/main.rs` reads PBCK using whole-file `std::fs::read` before parsing;
`crates/pblang-ir/src/lib.rs::Chunk` deserialization does not require consuming
all input. CLI instruction/memory budgets also default to unset on the reviewed
path. Untrusted bytecode therefore needs bounded admission and execution; this
review found a DoS/canonicality risk, not demonstrated native out-of-bounds
corruption in the checked-reader bytecode path.

Required fix: enforce byte limits before allocation, reject trailing bytes and
noncanonical encodings, validate structural references/control flow, and apply
explicit untrusted-execution budgets. Regression fixtures must never run a
hostile program in the agent/user process.

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

Current project instructions forbid new Rust source. These native fixes require
an explicit change to that constraint or a genuinely implemented PBLang native
boundary—not a library claim or a version bump. Hard heap/preemption, isolation,
revocation freshness and physical platform evidence remain release requirements.
See the final compatibility gate in `ROADMAP.md` before any 1.0.0 release.
