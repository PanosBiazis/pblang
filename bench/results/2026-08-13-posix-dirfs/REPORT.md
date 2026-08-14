# Portable POSIX Directory-Handle Benchmark

Date: 2026-08-13

## Scope

This benchmark measures the new PBLang-only Linux/macOS directory-handle core.
Every caller path is resolved component-by-component with `openat`,
`O_DIRECTORY`, and `O_NOFOLLOW`. Bounded data moves from the held source file
descriptor to an anonymous `tmpfile` snapshot and is loaded through the
platform descriptor view; no caller-controlled pathname is reopened.

## Environment

- Live host: Linux 7.0.0-28-generic x86_64
- CPU: Intel Core i7-1195G7, 4 cores / 8 threads
- PBLang SHA-256: `3f5c6d8847a750803f6a053087f5904c3358b114207dcc8169c6a1fc2512348e`
- Warm-up: 10 prefix reads
- Retained samples: 100 per operation

## Results

| Operation | Median | p95 | Max | Stability |
|---|---:|---:|---:|---|
| Descriptor prefix, up to 4 KiB | 0.0753 ms | 0.1190 ms | 0.1616 ms | timer-unstable, 23.97% CV |
| Descriptor range, 256 bytes | 0.0660 ms | 0.0771 ms | 0.1201 ms | noisy, 11.43% CV |

The operations are below the timer's reliable range; the primary gate is the
verified descriptor-bound, anonymous-snapshot, no-path-fallback contract.

## Platform Evidence

- Linux: live conformance for bounded prefix/range, atomic replace, exclusive
  create, file-only remove, descriptor close, and no pathname fallback.
- macOS: implementation and ABI profile are present. Darwin constants are
  pinned to Apple XNU `fcntl.h`; a physical macOS run remains required before
  claiming live macOS release verification.
- Windows: fails closed. A real root-relative Win32/Nt handle backend still
  requires a PBLang FFI/runtime primitive capable of the relevant native call
  shapes; lexical `CreateFileW` checks are intentionally not used.

The external workspace filesystem returned `EPERM` for symlink fixture
creation, so this run could not repeat the final/intermediate symlink live
lane. Existing `linux_dirfs_test.sapp` and `secure_file_test.sapp` retain that
coverage where the filesystem permits symlinks.

## Reproduce

```sh
cd products/pblang
target/release/pblang run tests/semantics/posix_dirfs_test.sapp --allow-all
target/release/pblang run bench/posix_dirfs_bench.sapp --allow-all
```
