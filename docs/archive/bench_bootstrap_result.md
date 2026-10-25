## Comparison

| # | Build | Executable (bytes) | Executable (human) | Peak RSS (KiB) |
|---|-------|-------------------:|-------------------:|---------------:|
| 0 | Rust: `cargo` toolchain (interpret) | 33170360 | 31.6 MiB | 8868 |
| 1 | pblang: `compile` native exe #1 | 25256 | 24.7 KiB | 1936 |
| 2 | pblang: `compile` native exe #2 | 25256 | 24.7 KiB | 1936 |
| 3 | pblang: `compile` native exe #3 | 25256 | 24.7 KiB | 1940 |

- **Determinism:** all three pblang builds produced byte-identical executables (25256 bytes).
- **On-disk:** the pblang-compiled native exe is ~1313× smaller than the Rust toolchain binary.
- **In RAM:** the pblang-compiled native exe peaks at ~4× less RSS than the interpreter.

## Native backend probe (same program, each `--backend`)

| Backend flag | Path actually used | Exe (bytes) | Peak RSS (KiB) |
|--------------|--------------------|------------:|---------------:|
| ctranspiler | C99 | 25256 | 1924 |
| cranelift | C99 (fell back) | 25256 | 1904 |
| llvm | C99 (fell back) | 25256 | 2044 |

> A "fell back" row means that native backend errored on this program/toolchain and the
> compiler transparently used the C99 path instead — worth knowing when a backend is advertised.
