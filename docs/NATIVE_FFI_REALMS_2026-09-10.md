# Native FFI ownership correction — 2026-09-10

The previous process-global FFI table allowed an independently created VM to
call or close a library loaded by another VM if it obtained the numeric handle.
The table now belongs to an execution realm. Root VMs and spawned VMs receive
fresh realms; nested import initialization shares the caller's realm so
exported handles remain usable after an import returns.

## Implemented behavior

- At most 128 open libraries per realm, shared across imports and cached
  polyglot libraries. Capacity is checked before loading constructors or
  starting compilation, and checked again on insertion.
- Opaque nonzero OS-random tokens fit exactly in the PBLang Number range
  `1..=2^53-1`. Fractional, non-finite, negative, zero and oversized values fail.
  Ownership lookup is the authorization boundary; randomness adds opacity.
- Unknown, foreign and already closed handles fail, including repeated close.
- Calls retain an `Rc<Library>` outside the registry borrow. Symbol lookup errors
  preserve the handle; closing a handle cannot unload an active call's code.
  The last realm owner releases all remaining libraries.
- The sandboxed C embedding constructor disables `spawn`; the ordinary
  compatibility constructor is unchanged.

The native exception is limited to these security corrections. New product
functionality remains implemented in PBLang.

## Validation on Linux GNU

- Six new native regression tests: independent realms, failed symbol lookup
  and resource drop, malformed numbers, capacity before loader/compiler effects,
  nested imports, and parent-handle rejection inside a spawned VM.
- Full VM and C embedding package tests: 23 passed; four pre-existing
  documentation examples remain ignored.
- Existing root sandbox suite: 12 passed.
- Eight PBLang suites passed with a freshly compiled debug runtime:
  `ffi_tuple_contract`, `linux_dirfs_ffi_denial`, `secure_file_capability`,
  `pbide_source_check`, `publisher_trust_store`, `release_publisher_attestation`,
  `publisher_artifact_admission`, and `publisher_release_floor`.
  The admission fixture used its required bounded 32 MiB file ceiling.
- Clippy completed successfully for both native packages and their test targets.
  Existing dependency/style warnings remain. Formatting passed for FFI/handle
  modules; the seven formatting differences in the VM/C API owners were verified
  identical to the committed baseline. No unrelated formatting was applied.
- A separate read-only security review approved the ownership/lifetime design.

These tests do not establish Windows/macOS execution coverage. The final
constructor simplification was covered by a repeat of the complete package
suite; the root sandbox and PBLang integration results exercised the same
behavior before that equivalent import-constructor simplification.

## Remaining limits

This is not a hostile-code FFI sandbox. Caller-declared ABI types and numeric
pointers can still cause undefined behavior; native constructors and functions
run in-process. Mutable library paths are not immutable identity admission.
`VmLimits.max_memory_bytes` does not hard-account intrinsic VM allocations, and
no hard in-process native wall deadline is provided. Child/channel budgets and
network/SQLite handle ownership remain separate work.

Failed imports can retain unreachable libraries until root-realm drop, within
the shared cap. OS entropy failure occurs after library loading, so constructors
may already have executed on that host-failure path. Release trust, runtime
signature-tuple enforcement and overall production readiness remain false.
