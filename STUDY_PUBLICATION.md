# Study new-copy publication contract

`study_files.save_copy` validates/encodes the canonical deck and calls `study_publication.save`. This adapter is bounded to 262144 UTF-8 bytes and `.pbdeck` paths of at most 4096 bytes. It reuses `linux_dirfs.sealed_child_bytes`, `copy_held_range_new_atomic` and `close`; no new FFI, syscalls, Rust, shell commands, mutable stage path or path-based removal is added.

The existing root walker holds a no-follow output parent. The payload is sealed in a memfd; the existing backend copies it into an anonymous O_TMPFILE inode, verifies mode/size and synchronizes it, then publishes that exact inode without replacing an existing name. A failure cannot publish this attempt's partial payload or leave a named staging file. Complete output can exist after an ambiguous helper result or a post-publication error; no automatic retry or deletion follows.

## Result evidence

- `publication_attempted:false`, `written:false`: the copy/publication backend was not called.
- `publication_attempted:true`, `output_may_exist:true`: the backend ran; errors without positive evidence omit `written` rather than claim absence.
- `publication_confirmed:true`, `written:true`: the backend confirmed publication. This evidence is preserved if durability or subsequent wrapper cleanup fails.
- `ok:true` requires backend success and wrapper handle closure. `durability_verified` means backend fsync acknowledgment, not independent power-loss testing. `atomic_visibility:true` is returned only with positive publication evidence.
- `partial_output_possible:false` describes this anonymous operation, not unrelated actors modifying the same namespace or other exporters.
- `owned_handles_closed` is the result of one root close operation, which consumes its registered sealed child too. No numeric descriptor is retried. Internal backend allocation/exception cleanup remains a separate runtime/FFI boundary; this adapter does not certify it.

Errors are generic and content/path-free; raw backend errors, descriptors and fingerprints are not returned. Aggregates/operation metadata still use `safe_to_log:false`. Mode is 0600 on successful publication.

## Availability and compatibility

This deliberately tightens deck-save availability to the existing Linux x86-64 sealed-memfd/O_TMPFILE/exact-inode helper implementation, with `/proc` and local execution capability. Unsupported operating systems, filesystems or capabilities fail closed without falling back to direct final-path writes. This does not upgrade the separate Web/Android exporter's partial-output contract. Canonical deck format, card history and in-memory models are unchanged.

## Checks

Run the semantic test with a fresh existing directory:

```
pblang run tests/semantics/study_publication_test.sapp --allow-all -- /absolute/fresh-scratch
```

`study_publication_probe.sapp` is a tiny-payload fixture for an external Linux resource-limit harness. A limit can deny the publication helper after the anonymous inode is prepared. The product-level `pbdeckmix/tests/publication_failures.py` runs a real RLIMIT_FSIZE payload failure and actual output permission failures through PBStudy acceptance, PBDeckMix mix and handoff, preserves existing file/hardlink/symlink/directory collisions, checks no named residue and races two writers. Physical Windows/macOS/mobile, abrupt termination and injected low-level close/fsync failure are not claimed by these tests.
